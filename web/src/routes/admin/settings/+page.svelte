<script lang="ts">
	import PageHeader from '$lib/components/PageHeader.svelte';
	import { getHealth, getSettings, updateSettings, ApiError } from '$lib/api';
	import { toast } from '$lib/toast.svelte';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import { Switch } from '$lib/components/ui/switch';
	import * as Tabs from '$lib/components/ui/tabs';
	import { Field, FieldContent, FieldDescription, FieldLabel } from '$lib/components/ui/field';
	import McpSettings from '$lib/components/McpSettings.svelte';
	import type { HealthResponse } from '$lib/types';

	// ── State ──
	let activeTab = $state<'general' | 'agents' | 'danger'>('general');
	let saving = $state(false);
	let loading = $state(false);

	// General settings
	let instanceName = $state('');
	let autoFetchComments = $state(true);
	let commentFetchInterval = $state('30');
	let scheduleLookaheadHours = $state('24');

	// API keys (masked)

	// Track whether user typed a new secret

	// Danger zone
	let confirmPurgeText = $state('');
	let confirmDeleteText = $state('');

	// Health
	let health = $state<HealthResponse | null>(null);
	let healthLoading = $state(false);

	// Lifecycle guard
	let loaded = $state(false);

	// ── Lifecycle ──
	$effect(() => {
		if (!loaded) {
			loaded = true;
			loadSettings();
		}
	});

	async function loadSettings() {
		loading = true;
		try {
			const s = await getSettings();
			instanceName = s.instance_name ?? '';
			autoFetchComments = s.auto_fetch_comments ?? true;
			commentFetchInterval = s.comment_fetch_interval ?? '30';
			scheduleLookaheadHours = s.schedule_lookahead_hours ?? '24';
		} catch (e) {
			if (e instanceof ApiError) {
				toast(`Failed to load settings: ${e.status}`, 'error');
			} else {
				toast('Failed to load settings', 'error');
			}
		} finally {
			loading = false;
		}
	}

	async function saveGeneral() {
		saving = true;
		try {
			const s = await updateSettings({
				instance_name: instanceName,
				auto_fetch_comments: autoFetchComments,
				comment_fetch_interval: commentFetchInterval,
				schedule_lookahead_hours: scheduleLookaheadHours,
			});
			toast('General settings saved', 'success');
		} catch (e) {
			if (e instanceof ApiError) {
				toast(`Failed to save: ${e.status}`, 'error');
			} else {
				toast('Failed to save settings', 'error');
			}
		} finally {
			saving = false;
		}
	}

	async function refreshHealth() {
		healthLoading = true;
		try {
			health = await getHealth();
		} catch (e) {
			if (e instanceof ApiError) {
				toast(`Health check failed: ${e.status}`, 'error');
			} else {
				toast('Health check failed', 'error');
			}
			health = null;
		} finally {
			healthLoading = false;
		}
	}


	async function purgeFailedPosts() {
		if (confirmPurgeText !== 'PURGE') return;
		try {
			const res = await fetch('/api/posts/purge-failed', { method: 'POST' });
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const data = await res.json();
			toast(`Purged ${data.deleted ?? 0} failed posts`, 'success');
			confirmPurgeText = '';
		} catch {
			toast('Purge failed — endpoint may not be available yet', 'error');
		}
	}

	async function deleteAllSchedules() {
		if (confirmDeleteText !== 'DELETE ALL') return;
		try {
			const res = await fetch('/api/schedules', { method: 'DELETE' });
			if (!res.ok) throw new Error(`HTTP ${res.status}`);
			const data = await res.json();
			toast(`Deleted ${data.deleted ?? 0} schedules`, 'success');
			confirmDeleteText = '';
		} catch {
			toast('Delete failed — endpoint may not be available yet', 'error');
		}
	}
</script>

<PageHeader title="Settings" description="Instance configuration" />

<Tabs.Root
	value={activeTab}
	onValueChange={(v) => (activeTab = v as typeof activeTab)}
	
>
	<Tabs.List>
		<Tabs.Trigger value="general">General</Tabs.Trigger>
		<Tabs.Trigger value="agents">Agents (MCP)</Tabs.Trigger>
		<Tabs.Trigger value="danger">Danger Zone</Tabs.Trigger>
	</Tabs.List>

	<!-- ── General ── -->
	<Tabs.Content value="general">
		<section class="page-section">
			<h2 class="section-heading">Instance</h2>
			<div class="settings-card">
				<Field>
					<FieldLabel for="instance-name">Instance Name</FieldLabel>
					<Input
						id="instance-name"
						type="text"
						bind:value={instanceName}
						placeholder="My Titen Instance"
					/>
					<FieldDescription>Display name shown in the sidebar and page title</FieldDescription>
				</Field>
			</div>

		</section>

		<section class="page-section">
			<h2 class="section-heading">Automation</h2>
			<div class="settings-card">
				<div class="form-row">
					<Field>
						<FieldLabel for="comment-interval">Comment Fetch Interval</FieldLabel>
						<Input
							id="comment-interval"
							type="number"
							min="5"
							max="1440"
							bind:value={commentFetchInterval}
						/>
						<FieldDescription>Minutes between auto-fetch cycles (5–1440)</FieldDescription>
					</Field>
					<Field>
						<FieldLabel for="schedule-lookahead">Schedule Lookahead</FieldLabel>
						<Input
							id="schedule-lookahead"
							type="number"
							min="1"
							max="168"
							bind:value={scheduleLookaheadHours}
						/>
						<FieldDescription>Hours ahead to show upcoming schedules</FieldDescription>
					</Field>
				</div>

				<Field orientation="horizontal">
					<Switch id="auto-fetch-comments" bind:checked={autoFetchComments} />
					<FieldContent>
						<FieldLabel for="auto-fetch-comments">Auto-fetch comments for published posts</FieldLabel>
					</FieldContent>
				</Field>
			</div>

			<div class="settings-actions">
				<Button variant="default" onclick={saveGeneral} disabled={saving}>
					{saving ? 'Saving…' : 'Save Changes'}
				</Button>
			</div>
		</section>

		<!-- System Health -->
		<section class="page-section">
			<div class="settings-section-row">
				<h2 class="section-heading">System Health</h2>
				<Button variant="outline" size="sm" onclick={refreshHealth} disabled={healthLoading}>
					{healthLoading ? 'Checking…' : 'Refresh'}
				</Button>
			</div>
			{#if health}
				<div class="settings-card">
					<div class="health-grid">
						<div class="health-item">
							<span class="health-label">Status</span>
							<span class="plk-badge plk-badge--{health.status === 'ok' ? 'success' : 'destructive'}">{health.status}</span>
						</div>
						<div class="health-item">
							<span class="health-label">Version</span>
							<span class="health-value tabular-nums">{health.version}</span>
						</div>
						<div class="health-item">
							<span class="health-label">Database</span>
							<span class="health-value tabular-nums">{health.db}</span>
						</div>
					</div>
				</div>
			{:else if !healthLoading}
				<div class="settings-card">
					<p class="plk-field-hint">Click <strong>Refresh</strong> to check system health</p>
				</div>
			{/if}
		</section>
	</Tabs.Content>

	<!-- ── Agents (MCP) ── -->
	<Tabs.Content value="agents">
		<McpSettings />
	</Tabs.Content>

	<!-- ── Danger Zone ── -->
	<Tabs.Content value="danger">
		<section class="page-section">
			<div class="settings-card settings-card--danger">
				<h3 class="settings-danger-title">Purge Failed Posts</h3>
				<p class="settings-danger-desc">
					Permanently delete all posts with a <span class="plk-badge plk-badge--destructive">failed</span> status.
					This action cannot be undone.
				</p>
				<div class="danger-confirm">
					<Field>
						<FieldLabel for="confirm-purge">Type <code>PURGE</code> to confirm</FieldLabel>
						<Input
							id="confirm-purge"
							class="max-w-80"
							type="text"
							bind:value={confirmPurgeText}
							placeholder="PURGE"
						/>
					</Field>
				</div>
				<Button variant="destructive"
				onclick={purgeFailedPosts}
				disabled={confirmPurgeText !== 'PURGE'}
			>
					Purge Failed Posts
				</Button>
			</div>

			<div class="settings-card settings-card--danger">
				<h3 class="settings-danger-title">Delete All Schedules</h3>
				<p class="settings-danger-desc">
					Remove all scheduled posts including pending, processing, and failed entries.
					This action cannot be undone.
				</p>
				<div class="danger-confirm">
					<Field>
						<FieldLabel for="confirm-delete">Type <code>DELETE ALL</code> to confirm</FieldLabel>
						<Input
							id="confirm-delete"
							class="max-w-80"
							type="text"
							bind:value={confirmDeleteText}
							placeholder="DELETE ALL"
						/>
					</Field>
				</div>
				<Button variant="destructive"
				onclick={deleteAllSchedules}
				disabled={confirmDeleteText !== 'DELETE ALL'}
			>
					Delete All Schedules
				</Button>
			</div>
		</section>
	</Tabs.Content>
</Tabs.Root>

<style>
	/* ── Settings sections ── */


	.settings-section-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
		flex-wrap: wrap;
	}


	/* ── Settings card ── */
	.settings-card {
		background: var(--card);
		border: var(--rule-default);
		border-radius: var(--plk-radius-lg);
		padding: var(--space-lg);
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
	}

	.settings-card--info {
		border-color: var(--primary);
		background: color-mix(in oklch, var(--primary) 10%, var(--card));
	}

	.settings-info-text {
		font-size: var(--text-sm);
		color: var(--muted-foreground);
		line-height: 1.6;
	}

	.secret-status {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
	}

	.settings-card--danger {
		border-color: var(--destructive);
		border-width: 1.5px;
	}

	/* ── Form row (side-by-side fields) ── */
	.form-row {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
		gap: var(--space-md);
	}

	/* ── Input with reveal button ── */
	.input-reveal {
		display: flex;
		gap: var(--space-2xs);
		align-items: center;
	}

	/* ── Save actions ── */
	.settings-actions {
		display: flex;
		justify-content: flex-start;
		margin-top: var(--space-md);
	}

	/* ── Health grid ── */
	.health-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr));
		gap: var(--space-md);
	}

	.health-item {
		display: flex;
		flex-direction: column;
		gap: var(--space-3xs);
	}

	.health-label {
		font-family: var(--plk-font-mono);
		font-size: var(--text-xs);
		color: var(--muted-foreground);
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.health-value {
		font-family: var(--plk-font-heading);
		font-size: var(--text-md);
		font-weight: 600;
	}

	/* ── Danger zone ── */
	.settings-danger-title {
		font-size: var(--text-base);
		font-weight: 600;
		color: var(--destructive-ink);
	}

	.settings-danger-desc {
		font-size: var(--text-sm);
		color: var(--muted-foreground);
		line-height: 1.6;
		margin-bottom: var(--space-xs);
	}

	.danger-confirm {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
		margin-top: var(--space-xs);
	}

	/* ── Responsive ── */
	@media (max-width: 48rem) {
		.settings-card {
			padding: var(--space-md);
		}

		.form-row {
			grid-template-columns: 1fr;
		}
	}
</style>
