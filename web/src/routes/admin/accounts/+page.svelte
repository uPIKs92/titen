<script lang="ts">
	import PageHeader from '$lib/components/PageHeader.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { listAccounts, createAccount, deleteAccount, refreshToken, getOAuthConfig, getThreadsProfile, createAuthorizeUrl, updateAccount } from '$lib/api';
	import type { OAuthConfig } from '$lib/api';
	import MetaSetupGuide from '$lib/components/MetaSetupGuide.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Table from '$lib/components/ui/table';
	import Skeleton from '$lib/components/ui/skeleton/skeleton.svelte';
	import { formatDate as formatDateTz } from '$lib/tz';
	import { toast } from '$lib/toast.svelte';
	import type { Account, ThreadsProfile } from '$lib/types';

	let accounts = $state<Account[]>([]);
	let loading = $state(true);
	let loaded = $state(false);
	let showAddModal = $state(false);
	let showGuide = $state(false);
	let deletingId = $state<string | null>(null);
	let deleteStage = $state<1 | 2>(1);
	let deleteConfirmText = $state('');
	let deleteUsername = $state('');
	let refreshingId = $state<string | null>(null);
	let submitting = $state(false);
	let profiles = $state<Record<string, ThreadsProfile | null>>({});
	let profilesLoading = $state<Set<string>>(new Set());

	let formUserId = $state('');
	let formUsername = $state('');
	let formAccessToken = $state('');
	let formExpiresAt = $state(new Date(Date.now() + 60 * 24 * 60 * 60 * 1000).toISOString().slice(0, 16));
	let formAppSecret = $state('');

	function formatDate(iso: string | null): string {
		if (!iso) return '\u2014';
		return formatDateTz(iso);
	}

	function needsReconnect(account: Account): boolean {
		if (!account.is_active || account.token_status === 'expired') return true;
		if (!account.expires_at) return false;
		const t = new Date(account.expires_at.includes('T') ? account.expires_at : account.expires_at.replace(' ', 'T') + 'Z').getTime();
		return Number.isFinite(t) && t < Date.now();
	}

	function statusFromAccount(account: Account): string {
		if (account.is_active) {
			return 'active';
		}
		if (account.token_status === 'expired') {
			return 'expired';
		}
		return 'suspended';
	}

	async function loadAccounts() {
		loading = true;
		try {
			accounts = await listAccounts();
			// Load profiles concurrently in small batches to avoid overwhelming
			// the backend or hitting Threads API rate limits (5 at a time).
			const ids = accounts.map((a) => a.id);
			profilesLoading = new Set(ids);
			const BATCH = 5;
			for (let i = 0; i < ids.length; i += BATCH) {
				const batch = ids.slice(i, i + BATCH);
				const results = await Promise.allSettled(
					batch.map((id) => getThreadsProfile(id)),
				);
				batch.forEach((id, j) => {
					const r = results[j];
					profiles = {
						...profiles,
						[id]: r.status === 'fulfilled' ? r.value : null,
					};
					const next = new Set(profilesLoading);
					next.delete(id);
					profilesLoading = next;
				});
			}
		} catch (e: any) {
			toast(e.message || 'Failed to load accounts', 'error');
		} finally {
			loading = false;
			loaded = true;
		}
	}

	async function handleRefreshToken(id: string) {
		refreshingId = id;
		try {
			const res = await refreshToken(id);
			toast('Token refreshed', 'success');
			accounts = accounts.map((a) =>
				a.id === id ? { ...a, expires_at: res.expires_at } : a,
			);
		} catch (e: any) {
			toast(e.message || 'Failed to refresh token', 'error');
		} finally {
			refreshingId = null;
		}
	}

	async function handleDelete() {
		if (!deletingId) return;
		if (deleteStage === 2 && deleteConfirmText.trim() !== deleteUsername) {
			toast(`Type "${deleteUsername}" exactly to confirm`, 'error');
			return;
		}
		if (deleteStage === 1) {
			deleteStage = 2;
			return;
		}
		try {
			await deleteAccount(deletingId);
			toast('Account deleted', 'success');
			accounts = accounts.filter((a) => a.id !== deletingId);
		} catch (e: any) {
			toast(e.message || 'Failed to delete account', 'error');
		} finally {
			deletingId = null;
			deleteStage = 1;
			deleteConfirmText = '';
		}
	}

	function resetForm() {
		formUserId = '';
		formUsername = '';
		formAccessToken = '';
		formExpiresAt = '';
		formAppSecret = '';
	}

	async function handleAddAccount() {
		if (!formAccessToken) {
			toast('Access token is required', 'error');
			return;
		}
		submitting = true;
		try {
			const created = await createAccount({
				...(formUserId && { user_id: formUserId }),
				...(formUsername && { username: formUsername }),
				access_token: formAccessToken,
				expires_at: new Date(formExpiresAt).toISOString(),
				...(formAppSecret && { app_secret: formAppSecret }),
			});
			toast('Account added', 'success');
			accounts = [...accounts, created];
			showAddModal = false;
			resetForm();
		} catch (e: any) {
			toast(e.message || 'Failed to add account', 'error');
		} finally {
			submitting = false;
		}
	}

	// ── Per-account Threads app credentials + OAuth connect ──
	// Every account is connected through its own Threads app (App ID + Secret).
	let oauthCfg = $state<OAuthConfig | null>(null);
	let connecting = $state(false);
	let copied = $state(false);

	// Add-account form (credentials for the NEW account).
	let newAppId = $state('');
	let newAppSecret = $state('');
	let copyFromId = $state(''); // existing account whose saved app to reuse

	// Edit-credentials dialog (an EXISTING account).
	let editingAccount = $state<Account | null>(null);
	let editAppId = $state('');
	let editAppSecret = $state('');
	let savingApp = $state(false);

	// Server-derived URI when available; otherwise the origin the user is on.
	const redirectUri = $derived(
		oauthCfg?.redirect_uri || (typeof window !== 'undefined' ? `${window.location.origin}/auth/callback` : ''),
	);
	const accountsWithApp = $derived(accounts.filter((a) => a.app_id && a.app_secret_set));
	const copySource = $derived(accounts.find((a) => a.id === copyFromId) ?? null);
	const canConnectNew = $derived(
		!!newAppId.trim() && (!!newAppSecret || (!!copySource && copySource.app_id === newAppId.trim())),
	);

	async function loadOAuthConfig() {
		try {
			oauthCfg = await getOAuthConfig();
		} catch (e: any) {
			toast(e.message || 'Failed to load OAuth settings', 'error');
		}
	}

	function openGuide(fromAdd = false) {
		if (fromAdd) showAddModal = false;
		showGuide = true;
		if (!oauthCfg) loadOAuthConfig();
	}

	function openAddModal() {
		showGuide = false;
		newAppId = '';
		newAppSecret = '';
		copyFromId = '';
		showAddModal = true;
		loadOAuthConfig();
	}

	function pickCopyFrom(id: string) {
		copyFromId = id;
		const src = accounts.find((a) => a.id === id);
		newAppId = src?.app_id ?? '';
		newAppSecret = '';
	}

	function openEditApp(account: Account) {
		editingAccount = account;
		editAppId = account.app_id ?? '';
		editAppSecret = '';
	}

	async function saveAccountApp() {
		if (!editingAccount) return;
		if (!editAppId.trim()) {
			toast('App ID is required', 'error');
			return;
		}
		if (!editAppSecret && !editingAccount.app_secret_set) {
			toast('App Secret is required', 'error');
			return;
		}
		savingApp = true;
		try {
			const updated: any = await updateAccount(editingAccount.id, {
				app_id: editAppId.trim(),
				...(editAppSecret && { app_secret: editAppSecret }),
			});
			if (updated?.error) throw new Error(updated.error);
			accounts = accounts.map((a) =>
				a.id === editingAccount!.id
					? { ...a, app_id: updated.app_id ?? editAppId.trim(), app_secret_set: updated.app_secret_set ?? true }
					: a,
			);
			toast(`Threads app saved for @${editingAccount.username}`, 'success');
			editingAccount = null;
			editAppSecret = '';
		} catch (e: any) {
			toast(e.message || 'Failed to save Threads app', 'error');
		} finally {
			savingApp = false;
		}
	}

	async function copyRedirectUri() {
		try {
			await navigator.clipboard.writeText(redirectUri);
			copied = true;
			setTimeout(() => (copied = false), 1500);
		} catch {
			toast('Copy failed — select the URI and copy manually', 'error');
		}
	}

	async function startOAuth(params: { app_id?: string; app_secret?: string; account_id?: string }) {
		connecting = true;
		try {
			if (!oauthCfg) await loadOAuthConfig();
			const { authorize_url } = await createAuthorizeUrl({ redirect_uri: redirectUri, ...params });
			window.location.href = authorize_url;
		} catch (e: any) {
			toast(e.message || 'Failed to start Threads connection', 'error');
			connecting = false;
		}
	}

	function handleConnectNew() {
		return startOAuth({
			app_id: newAppId.trim(),
			...(newAppSecret && { app_secret: newAppSecret }),
			...(copyFromId && { account_id: copyFromId }),
		});
	}

	// Reconnect uses THIS account's saved Threads app; the exchange upserts by
	// user_id so the account is renewed in place.
	function handleReconnect(account: Account) {
		if (!account.app_id || !account.app_secret_set) {
			toast('Set this account\'s Threads App ID and Secret first', 'error');
			openEditApp(account);
			return;
		}
		return startOAuth({ account_id: account.id });
	}

	// Fetch once on mount — avoid infinite $effect re-runs
	$effect(() => {
		if (!loaded) {
			loadAccounts();
			loadOAuthConfig();
		}
	});
</script>

<PageHeader title="Accounts">
	{#snippet action()}
		<div class="row-gap-xs">
			<Button variant="outline" onclick={() => openGuide()} aria-label="Meta setup guide" title="Where do I find the App ID, secret and redirect URI?">
				<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
				Setup guide
			</Button>
			<Button variant="default" onclick={openAddModal}>Add Account</Button>
		</div>
	{/snippet}
</PageHeader>

<div class="data-table-wrap plk-card">
	{#if loading}
		<Table.Root>
			<Table.Header><Table.Row><Table.Head>Username</Table.Head><Table.Head>Status</Table.Head><Table.Head class="hidden md:table-cell">Token</Table.Head><Table.Head class="hidden md:table-cell">Created</Table.Head><Table.Head>Actions</Table.Head></Table.Row></Table.Header>
			<Table.Body>
				{#each Array(3) as _}
					<Table.Row>
						<Table.Cell><Skeleton class="h-4 w-full" /></Table.Cell>
						<Table.Cell><Skeleton class="h-4 w-full" /></Table.Cell>
						<Table.Cell><Skeleton class="h-4 w-full" /></Table.Cell>
						<Table.Cell class="hidden md:table-cell"><Skeleton class="h-4 w-full" /></Table.Cell>
						<Table.Cell class="hidden md:table-cell"><Skeleton class="h-4 w-full" /></Table.Cell>
						<Table.Cell><Skeleton class="h-4 w-full" /></Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	{:else if accounts.length === 0}
		<div class="plk-empty">
			<p class="plk-empty-title">No accounts yet</p>
			<p class="plk-empty-text">Add a Threads account to get started.</p>
		</div>
	{:else}
		<Table.Root>
			<Table.Header>
				<Table.Row>
					<Table.Head>Account</Table.Head>
					<Table.Head>Followers</Table.Head>
					<Table.Head>Status</Table.Head>
					<Table.Head>Threads app</Table.Head>
					<Table.Head class="hidden md:table-cell">Token</Table.Head>
					<Table.Head class="hidden md:table-cell">Created</Table.Head>
					<Table.Head>Actions</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each accounts as account (account.id)}
					<Table.Row>
						<Table.Cell>
							<div class="account-cell">
								{#if profiles[account.id]?.threads_profile_picture_url}
									<img
										src={profiles[account.id]!.threads_profile_picture_url!}
										alt={account.username}
										class="account-avatar"
										width="40"
										height="40"
									/>
								{:else if profilesLoading.has(account.id)}
									<Skeleton class="account-avatar" />
								{:else}
									<div class="account-avatar account-avatar-placeholder">
										{account.username.charAt(0).toUpperCase()}
									</div>
								{/if}
								<div>
									<div style="font-weight:500;">
										{profiles[account.id]?.name ?? account.username}
										{#if profiles[account.id]?.username && profiles[account.id]?.username !== account.username}
											<span style="font-size:var(--text-xs);color:var(--muted-foreground);">@{profiles[account.id]!.username}</span>
										{/if}
									</div>
									{#if profiles[account.id]?.threads_biography}
										<div class="account-bio">{profiles[account.id]!.threads_biography}</div>
									{:else if account.user_id}
										<div class="text-xs-muted">ID: {account.user_id.length > 30 ? account.user_id.slice(0, 12) + '\u2026' : account.user_id}</div>
									{/if}
								</div>
							</div>
						</Table.Cell>
						<Table.Cell>
							{#if profilesLoading.has(account.id)}
								<Skeleton class="h-5 w-12" />
							{:else if profiles[account.id]?.followers_count != null}
								<span class="tabular-nums font-medium">{profiles[account.id]!.followers_count!.toLocaleString()}</span>
							{:else}
								<span class="text-xs-muted">\u2014</span>
							{/if}
						</Table.Cell>
						<Table.Cell><StatusBadge status={statusFromAccount(account)} /></Table.Cell>
						<Table.Cell>
							<div class="app-cell">
								{#if account.app_id}
									<code class="app-id" data-testid="account-app-id">{account.app_id}</code>
									<span class="text-xs-muted">{account.app_secret_set ? 'secret saved' : 'secret missing'}</span>
								{:else}
									<span class="text-xs-muted">Not set</span>
								{/if}
								<Button variant="outline" size="sm" onclick={() => openEditApp(account)}>{account.app_id ? 'Edit' : 'Set up'}</Button>
							</div>
						</Table.Cell>
						<Table.Cell class="hidden md:table-cell">
						<span class="tabular-nums text-sm">
							{#if account.expires_at}
								Expires {formatDate(account.expires_at)}
							{:else}
								—
							{/if}
						</span>
					</Table.Cell>
						<Table.Cell class="hidden md:table-cell"><span class="tabular-nums">{formatDate(account.created_at)}</span></Table.Cell>
						<Table.Cell>
							<div class="row-gap-xs">
								<Button
									variant="outline"
									size="sm"
									disabled={refreshingId === account.id}
									onclick={() => handleRefreshToken(account.id)}
								>
									{refreshingId === account.id ? '…' : 'Refresh'}
								</Button>
								{#if needsReconnect(account)}
									<Button variant="default" size="sm" disabled={connecting} onclick={() => handleReconnect(account)}>Reconnect</Button>
								{/if}
								<Button variant="ghost" size="sm" onclick={() => { deletingId = account.id; deleteStage = 1; deleteConfirmText = ''; deleteUsername = account.username; }}>
									<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="icon-sm-danger">
														<path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/>
									</svg>
								</Button>
							</div>
						</Table.Cell>
						</Table.Row>
						{/each}
						</Table.Body>
						</Table.Root>
						{/if}
						</div>

						<!-- Add Account Modal -->
{#if showAddModal}
	<Dialog.Root open onOpenChange={(o) => { if (!o) showAddModal = false; }}>
		<Dialog.Content class="confirm-dialog" style="max-width:32rem;" aria-describedby={undefined}>
			<Dialog.Title class="text-base font-semibold mb-xs">Add Account</Dialog.Title>

			<p class="text-hint mb-sm">Connect through Threads — no token to copy from the Meta developer page.</p>

			<!-- Step 1: this account's Threads app -->
			<section class="connect-step" aria-labelledby="step-creds">
				<h3 id="step-creds" class="connect-step-title"><span class="step-num">1</span> This account's Threads app</h3>
				{#if accountsWithApp.length > 0}
					<div class="plk-field mb-sm">
						<label class="plk-label" for="copy_from">Reuse an existing account's app <span class="text-hint">(optional)</span></label>
						<select id="copy_from" class="form-select" value={copyFromId} onchange={(e) => pickCopyFrom((e.currentTarget as HTMLSelectElement).value)} disabled={connecting}>
							<option value="">Use a different app…</option>
							{#each accountsWithApp as a (a.id)}
								<option value={a.id}>@{a.username} — {a.app_id}</option>
							{/each}
						</select>
					</div>
				{/if}
				<div class="plk-field mb-sm">
					<label class="plk-label" for="new_app_id">Threads App ID</label>
					<Input id="new_app_id" type="text" bind:value={newAppId} placeholder="Threads app ID (not the main Meta app ID)" disabled={connecting} oninput={() => { if (copySource && copySource.app_id !== newAppId.trim()) copyFromId = ''; }} />
				</div>
				<div class="plk-field mb-sm">
					<label class="plk-label" for="new_app_secret">Threads App Secret</label>
					<Input id="new_app_secret" type="password" bind:value={newAppSecret} autocomplete="off" placeholder={copySource ? `Using the saved secret from @${copySource.username}` : 'Threads app secret'} disabled={connecting} />
				</div>
				<p class="text-hint">Meta dashboard → Use cases → Access the Threads API → Customize → Settings. <button type="button" class="guide-link" onclick={() => openGuide(true)}>Open the setup guide</button></p>
			</section>

			<!-- Step 2: redirect URI to whitelist -->
			<section class="connect-step" aria-labelledby="step-redirect">
				<h3 id="step-redirect" class="connect-step-title"><span class="step-num">2</span> Allow this redirect URI in Meta</h3>
				<p class="text-hint mb-xs">Add it to <strong>Redirect Callback URLs</strong> on the same Meta Settings page. <button type="button" class="guide-link" onclick={() => openGuide(true)}>Where exactly?</button></p>
				<div class="redirect-row">
					<code class="redirect-uri" data-testid="redirect-uri">{redirectUri}</code>
					<Button type="button" variant="outline" size="sm" onclick={copyRedirectUri}>{copied ? 'Copied' : 'Copy'}</Button>
				</div>
			</section>

			<!-- Step 3: connect -->
			<section class="connect-step" aria-labelledby="step-connect">
				<h3 id="step-connect" class="connect-step-title"><span class="step-num">3</span> Authorize</h3>
				<button
					type="button"
					class="oauth-connect-btn"
					disabled={connecting || !canConnectNew}
					onclick={handleConnectNew}
				>
					<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
						<path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 3c1.66 0 3 1.34 3 3s-1.34 3-3 3-3-1.34-3-3 1.34-3 3-3zm0 14.2c-2.5 0-4.71-1.28-6-3.22.03-1.99 4-3.08 6-3.08 1.99 0 5.97 1.09 6 3.08-1.29 1.94-3.5 3.22-6 3.22z"/>
					</svg>
					{connecting ? 'Redirecting to Threads…' : 'Connect with Threads'}
				</button>
				{#if !canConnectNew}<p class="text-hint mt-xs">Enter the App ID and Secret in step 1 to enable this.</p>{/if}
			</section>

			<details class="manual-add">
				<summary>Advanced: add with an access token instead</summary>
				<form onsubmit={(e) => { e.preventDefault(); handleAddAccount(); }}>
					<div class="plk-field mb-sm">
						<label class="plk-label" for="access_token">Access Token</label>
						<Input id="access_token" type="password" bind:value={formAccessToken} placeholder="Long-lived access token" disabled={submitting} />
					</div>
					<div class="plk-field mb-sm">
						<label class="plk-label" for="username">Username <span class="text-hint">(optional)</span></label>
						<Input id="username" type="text" bind:value={formUsername} placeholder="e.g. @username" disabled={submitting} />
					</div>
					<div class="plk-field mb-sm">
						<label class="plk-label" for="user_id">User ID <span class="text-hint">(optional)</span></label>
						<Input id="user_id" type="text" bind:value={formUserId} placeholder="e.g. 1234567890" disabled={submitting} />
					</div>
					<div class="plk-field mb-sm">
						<label class="plk-label" for="expires_at">Expires At <span class="text-hint">(optional)</span></label>
						<Input id="expires_at" type="datetime-local" bind:value={formExpiresAt} disabled={submitting} />
					</div>
					<div class="plk-field mb-sm">
						<label class="plk-label" for="app_secret">App Secret <span class="text-hint">(only to upgrade a short-lived token)</span></label>
						<Input id="app_secret" type="password" bind:value={formAppSecret} placeholder="Threads App Secret" disabled={submitting} />
					</div>
					<div class="confirm-actions">
						<Button type="submit" variant="outline" size="sm" disabled={submitting}>{submitting ? 'Adding…' : 'Add with token'}</Button>
					</div>
				</form>
			</details>

			<div class="confirm-actions">
				<Button type="button" variant="ghost" size="sm" onclick={() => (showAddModal = false)}>Close</Button>
			</div>
		</Dialog.Content>
	</Dialog.Root>
{/if}

{#if showGuide}
	<MetaSetupGuide {redirectUri} onclose={() => (showGuide = false)} onadd={openAddModal} />
{/if}

<!-- Edit an account's Threads app -->
{#if editingAccount}
	<Dialog.Root open onOpenChange={(o) => { if (!o) editingAccount = null; }}>
		<Dialog.Content class="confirm-dialog" style="max-width:28rem;" aria-describedby={undefined}>
			<Dialog.Title class="text-base font-semibold mb-xs">Threads app for @{editingAccount.username}</Dialog.Title>
			<p class="text-hint mb-sm">Used when this account is connected or reconnected. Each account can use a different Threads app. <button type="button" class="guide-link" onclick={() => { editingAccount = null; openGuide(); }}>Where do I find these?</button></p>
			<form onsubmit={(e) => { e.preventDefault(); saveAccountApp(); }}>
				<div class="plk-field mb-sm">
					<label class="plk-label" for="edit_app_id">Threads App ID</label>
					<Input id="edit_app_id" type="text" bind:value={editAppId} placeholder="Threads app ID" disabled={savingApp} />
				</div>
				<div class="plk-field mb-sm">
					<label class="plk-label" for="edit_app_secret">Threads App Secret</label>
					<Input id="edit_app_secret" type="password" bind:value={editAppSecret} autocomplete="off" placeholder={editingAccount.app_secret_set ? 'Leave blank to keep the saved secret' : 'Threads app secret'} disabled={savingApp} />
				</div>
				<div class="confirm-actions">
					<Button type="button" variant="ghost" size="sm" onclick={() => (editingAccount = null)} disabled={savingApp}>Cancel</Button>
					<Button type="submit" variant="default" size="sm" disabled={savingApp}>{savingApp ? 'Saving…' : 'Save'}</Button>
				</div>
			</form>
		</Dialog.Content>
	</Dialog.Root>
{/if}

<!-- Delete Confirmation (stage 2: type-to-confirm) -->
<ConfirmDialog
	open={deletingId !== null}
	title={deleteStage === 1 ? 'Delete Account' : 'Really delete EVERYTHING?'}
	message={deleteStage === 1
		? 'This will permanently remove this account and all its posts, schedules, media, mentions and analytics. This action cannot be undone.'
		: `Type the username "${deleteUsername}" below to confirm the permanent wipe of this account and ALL its data.`}
	confirmLabel={deleteStage === 1 ? 'Continue' : 'Delete permanently'}
	showInput={deleteStage === 2}
	bind:confirmText={deleteConfirmText}
	onconfirm={handleDelete}
	oncancel={() => { deletingId = null; deleteStage = 1; deleteConfirmText = ''; }}
/>

<style>
	.account-cell {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}
	.account-avatar {
		width: 40px;
		height: 40px;
		border-radius: 9999px;
		object-fit: cover;
		flex-shrink: 0;
	}
	.account-avatar-placeholder {
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in oklch, var(--primary) 8%, transparent);
		color: var(--primary-text);
		font-weight: 600;
		font-size: var(--text-sm);
	}
	.account-bio {
		font-size: var(--text-xs);
		color: var(--muted-foreground);
		max-width: 280px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.oauth-connect-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-sm);
		width: 100%;
		padding: var(--space-md) var(--space-lg);
		margin-bottom: var(--space-md);
		background: var(--primary);
		color: var(--primary-foreground);
		font-family: var(--plk-font-heading);
		font-weight: 600;
		font-size: var(--text-sm);
		border: none;
		border-radius: var(--plk-radius-md);
		cursor: pointer;
		transition: background-color var(--dur-short) var(--ease-out);
	}
	.oauth-connect-btn:hover {
		background: color-mix(in oklch, var(--primary) 88%, var(--foreground));
	}

	.guide-link { background: none; border: 0; padding: 0; color: var(--primary-text); cursor: pointer; text-decoration: underline; font: inherit; }
	.app-cell { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-xs); }
	.app-id { font-size: var(--text-xs); }
	.form-select { width: 100%; padding: var(--space-xs) var(--space-sm); background: var(--card); border: var(--rule-default); border-radius: var(--plk-radius-md); font-size: var(--text-sm); color: inherit; }
	.connect-step { margin-bottom: var(--space-md); }
	.connect-step-title { display: flex; align-items: center; gap: var(--space-xs); font-size: var(--text-sm); font-weight: 600; margin-bottom: var(--space-xs); }
	.step-num { display: inline-flex; align-items: center; justify-content: center; width: 1.25rem; height: 1.25rem; border-radius: 50%; background: var(--primary-text); color: var(--primary-foreground); font-size: var(--text-xs); }
	.redirect-row { display: flex; align-items: center; gap: var(--space-xs); }
	.redirect-uri { flex: 1; min-width: 0; overflow-wrap: anywhere; padding: var(--space-xs) var(--space-sm); background: var(--card); border: var(--rule-default); border-radius: var(--plk-radius-md); font-size: var(--text-xs); }
	.manual-add { margin-bottom: var(--space-md); font-size: var(--text-sm); }
	.manual-add summary { cursor: pointer; color: var(--muted-foreground); margin-bottom: var(--space-sm); }
</style>