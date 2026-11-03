<script lang="ts">
	import PageHeader from '$lib/components/PageHeader.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
	import PostDetail from '$lib/components/PostDetail.svelte';
	import DataTable from '$lib/components/DataTable.svelte';
	import { listPosts, deletePost, listAccounts, createPost, ApiError } from '$lib/api';
	import * as Dialog from '$lib/components/ui/dialog';
	import Textarea from '$lib/components/ui/textarea/textarea.svelte';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import * as Select from '$lib/components/ui/select';
	import { formatDateShort } from '$lib/tz';
	import { toast } from '$lib/toast.svelte';
	import type { Post, Account } from '$lib/types';

	let loading = $state(true);
	let loaded = $state(false);
	let posts = $state<Post[]>([]);
	let accounts = $state<Account[]>([]);
	let filterAccount = $state('');
	let filterStatus = $state('');
	let confirmDelete = $state<{ open: boolean; post: Post | null }>({ open: false, post: null });

	// New post dialog
	const CAPTION_MAX = 499;
	let showNew = $state(false);
	let publishing = $state(false);
	let newAccountId = $state('');
	let newType = $state<'TEXT' | 'IMAGE' | 'VIDEO'>('TEXT');
	let newCaption = $state('');
	let newMediaUrl = $state('');

	const needsMedia = $derived(newType !== 'TEXT');
	const canPublish = $derived(
		!!newAccountId &&
			newCaption.length <= CAPTION_MAX &&
			(needsMedia ? newMediaUrl.trim().length > 0 : newCaption.trim().length > 0)
	);

	function openNew() {
		newAccountId = filterAccount || accounts[0]?.id || '';
		newType = 'TEXT';
		newCaption = '';
		newMediaUrl = '';
		showNew = true;
	}

	async function submitNew() {
		if (!canPublish || publishing) return;
		publishing = true;
		try {
			const url = newMediaUrl.trim();
			const post = await createPost({
				account_id: newAccountId,
				media_type: newType,
				caption: newCaption.trim() || undefined,
				image_url: newType === 'IMAGE' ? url : undefined,
				video_url: newType === 'VIDEO' ? url : undefined,
			});
			toast('Post published', 'success');
			posts = [post, ...posts];
			showNew = false;
		} catch (e) {
			let msg = 'Publish failed';
			if (e instanceof ApiError) {
				try { msg = `Publish failed: ${JSON.parse(e.body).error ?? e.statusText}`; } catch { msg = `Publish failed: ${e.statusText || e.status}`; }
			}
			toast(msg, 'error');
		} finally {
			publishing = false;
		}
	}

	// Detail modal
	let detailPost = $state<Post | null>(null);

	function openDetail(post: Post) {
		// Enrich with account info for permalink
		const enriched = { ...post, account: accounts.find(a => a.id === post.account_id) };
		detailPost = enriched;
	}
	function closeDetail() {
		detailPost = null;
	}

	const filtered = $derived.by(() => {
		let result = posts;
		if (filterAccount) result = result.filter((p) => p.account_id === filterAccount);
		if (filterStatus) result = result.filter((p) => p.status === filterStatus);
		return result;
	});

	function formatDate(iso: string | null): string {
		if (!iso) return '\u2014';
		return formatDateShort(iso);
	}

	async function loadPosts() {
		loading = true;
		try {
			const [p, a] = await Promise.all([
				listPosts().catch(() => []),
				listAccounts().catch(() => []),
			]);
			posts = p;
			accounts = a;
		} catch (e: any) {
			toast('Failed to load posts', 'error');
		} finally {
			loading = false;
			loaded = true;
		}
	}

	function getAccountUsername(accountId: string): string {
		return accounts.find((a) => a.id === accountId)?.username ?? accountId.slice(0, 8);
	}

	async function handleDelete() {
		if (!confirmDelete.post) return;
		try {
			await deletePost(confirmDelete.post.id);
			toast('Post deleted', 'success');
			posts = posts.filter((p) => p.id !== confirmDelete.post!.id);
		} catch {
			toast('Failed to delete post', 'error');
		} finally {
			confirmDelete = { open: false, post: null };
		}
	}

	$effect(() => {
		if (!loaded) loadPosts();
	});

	const columns = [
		{ key: 'caption', label: 'Content', class: 'truncate truncate-mw-40' },
		{ key: 'account', label: 'Account' },
		{ key: 'media_type', label: 'Type', hideOnMobile: true },
		{ key: 'status', label: 'Status' },
		{ key: 'published_at', label: 'Published', hideOnMobile: true },
	];
</script>

<PageHeader title="Posts" description="Manage and monitor your Threads content.">
	{#snippet action()}
		<Button variant="default" onclick={openNew} disabled={accounts.length === 0} title={accounts.length === 0 ? 'Add an account first' : undefined}>New Post</Button>
	{/snippet}
</PageHeader>

<div class="page-toolbar">
	<Select.Root type="single" bind:value={filterAccount}>
		<Select.Trigger>
			{filterAccount ? `@${accounts.find((a) => a.id === filterAccount)?.username ?? ''}` : 'All Accounts'}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="" label="All Accounts">All Accounts</Select.Item>
			{#each accounts as account (account.id)}
				<Select.Item value={account.id} label={`@${account.username}`}>
					@{account.username}
				</Select.Item>
			{/each}
		</Select.Content>
	</Select.Root>
	<Select.Root type="single" bind:value={filterStatus}>
		<Select.Trigger>
			{filterStatus === '' ? 'All Status' : filterStatus.charAt(0).toUpperCase() + filterStatus.slice(1)}
		</Select.Trigger>
		<Select.Content>
			<Select.Item value="" label="All Status">All Status</Select.Item>
			<Select.Item value="draft" label="Draft">Draft</Select.Item>
			<Select.Item value="published" label="Published">Published</Select.Item>
			<Select.Item value="failed" label="Failed">Failed</Select.Item>
			<Select.Item value="deleted" label="Deleted">Deleted</Select.Item>
		</Select.Content>
	</Select.Root>
</div>

{#if showNew}
	<Dialog.Root open onOpenChange={(o) => { if (!o && !publishing) showNew = false; }}>
		<Dialog.Content class="confirm-dialog" style="max-width:32rem;" aria-describedby={undefined}>
			<Dialog.Title class="text-base font-semibold mb-xs">New Post</Dialog.Title>
			<p class="text-hint mb-sm">Publishes to Threads immediately. To post later, use Schedules.</p>
			<form onsubmit={(e) => { e.preventDefault(); submitNew(); }}>
				<div class="plk-field mb-sm">
					<label class="plk-label" for="np_account">Account</label>
					<select id="np_account" class="form-select" bind:value={newAccountId} disabled={publishing}>
						{#each accounts as a (a.id)}
							<option value={a.id}>@{a.username}</option>
						{/each}
					</select>
				</div>
				<div class="plk-field mb-sm">
					<label class="plk-label" for="np_type">Type</label>
					<select id="np_type" class="form-select" bind:value={newType} disabled={publishing}>
						<option value="TEXT">Text</option>
						<option value="IMAGE">Image</option>
						<option value="VIDEO">Video</option>
					</select>
				</div>
				{#if needsMedia}
					<div class="plk-field mb-sm">
						<label class="plk-label" for="np_media">{newType === 'IMAGE' ? 'Image' : 'Video'} URL</label>
						<Input id="np_media" type="url" bind:value={newMediaUrl} placeholder="https://… (publicly reachable)" disabled={publishing} />
					</div>
				{/if}
				<div class="plk-field mb-sm">
					<label class="plk-label" for="np_caption">Caption</label>
					<Textarea id="np_caption" rows={5} bind:value={newCaption} disabled={publishing} />
					<p class="text-hint" style:color={newCaption.length > CAPTION_MAX ? 'var(--destructive)' : undefined}>{newCaption.length}/{CAPTION_MAX}</p>
				</div>
				<div class="confirm-actions">
					<Button type="button" variant="ghost" size="sm" onclick={() => (showNew = false)} disabled={publishing}>Cancel</Button>
					<Button type="submit" size="sm" disabled={!canPublish || publishing}>{publishing ? 'Publishing…' : 'Publish now'}</Button>
				</div>
			</form>
		</Dialog.Content>
	</Dialog.Root>
{/if}

<div class="data-table-wrap plk-card">
	<DataTable {columns} rows={filtered} {loading} emptyTitle="No posts yet" emptyDesc="Posts will appear here once you publish content." onrowclick={openDetail}>
		{#snippet cell(row: Post, key: string)}
			{#if key === 'caption'}
				{row.caption ? (row.caption.length > 40 ? row.caption.slice(0, 40) + '…' : row.caption) : '(no caption)'}
			{:else if key === 'account'}
				<span style="color:var(--muted-foreground);">@{getAccountUsername(row.account_id)}</span>
			{:else if key === 'status'}
				<StatusBadge status={row.status} />
			{:else if key === 'published_at'}
				<span class="tabular-nums">{formatDate(row.published_at)}</span>
			{:else}
				{row[key as keyof Post] ?? '—'}
			{/if}
		{/snippet}
		{#snippet actions(row: Post)}
			<div class="row-gap-xs">
				<Button variant="outline" size="sm" onclick={() => openDetail(row)}>Detail</Button>
				<Button variant="ghost" size="sm" onclick={() => (confirmDelete = { open: true, post: row })}>
					<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="icon-sm-danger">
							<path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="M19 6l-1 14H6L5 6"/>
					</svg>
				</Button>
			</div>
		{/snippet}
	</DataTable>
</div>

<!-- Post Detail Modal -->
{#if detailPost}
	{@const postWithAccount = { ...detailPost, account: accounts.find(a => a.id === detailPost?.account_id) }}
	<PostDetail post={postWithAccount} onClose={closeDetail} />
{/if}

<ConfirmDialog
	open={confirmDelete.open}
	title="Delete Post"
	message="This will permanently remove this post. This action cannot be undone."
	confirmLabel="Delete"
	variant="danger"
	onconfirm={handleDelete}
	oncancel={() => (confirmDelete = { open: false, post: null })}
/>

<svelte:window onkeydown={(e) => {
	if (e.key === 'Escape' && detailPost) closeDetail();
}} />
