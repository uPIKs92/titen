<script lang="ts">
	import PageHeader from '$lib/components/PageHeader.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import {
		listComments,
		fetchComments,
		listPosts,
		updateCommentReply,
		replyToComment,
	} from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import * as Select from '$lib/components/ui/select';
	import Skeleton from '$lib/components/ui/skeleton/skeleton.svelte';
	import Textarea from '$lib/components/ui/textarea/textarea.svelte';
	import type { Comment, Post } from '$lib/types';
	import { formatDateTime } from '$lib/tz';
	import { truncate } from '$lib/format';
	import { toast } from '$lib/toast.svelte';

	let comments = $state<Comment[]>([]);
	let posts = $state<Post[]>([]);
	let loading = $state(true);
	let loaded = $state(false);
	let commentsLoading = $state(false);
	let fetchLoading = $state(false);
	let selectedPostId = $state('');
	let replyStatusFilter = $state(''); // '' = all
	let sentimentFilter = $state(''); // '' = all
	let searchQuery = $state('');

	// Reply workflow state
	let replyingTo = $state<string | null>(null);
	let replyText = $state('');
	let replyLoading = $state(false);
	let actionLoading = $state<string | null>(null); // commentId being updated

	let filteredComments = $derived(
		comments.filter((c) => {
			if (replyStatusFilter && c.reply_status !== replyStatusFilter) return false;
			if (sentimentFilter && c.sentiment !== sentimentFilter) return false;
			if (searchQuery && !(c.text || '').toLowerCase().includes(searchQuery.toLowerCase()))
				return false;
			return true;
		}),
	);

	let sentimentSummary = $derived({
		positive: comments.filter((c) => c.sentiment === 'positive').length,
		negative: comments.filter((c) => c.sentiment === 'negative').length,
		neutral: comments.filter((c) => c.sentiment === 'neutral').length,
	});

	let replyStatusSummary = $derived({
		new: comments.filter((c) => c.reply_status === 'new').length,
		needs_reply: comments.filter((c) => c.reply_status === 'needs_reply').length,
		replied: comments.filter((c) => c.reply_status === 'replied').length,
		skipped: comments.filter((c) => c.reply_status === 'skipped').length,
	});

	const selectedPost = $derived(posts.find((p) => p.id === selectedPostId) ?? null);
	const filtersActive = $derived(!!(replyStatusFilter || sentimentFilter || searchQuery.trim()));

	function clearFilters() {
		replyStatusFilter = '';
		sentimentFilter = '';
		searchQuery = '';
	}

	function postLabel(p: Post): string {
		return truncate((p.caption || '').trim() || '(no caption)', 70);
	}
	function postMeta(p: Post): string {
		const when = p.published_at ?? p.created_at;
		return [when ? formatDateTime(when) : null, p.status].filter(Boolean).join(' · ');
	}

	// Toggle-chip definitions (value '' = all)
	const statusChips = $derived([
		{ value: '', label: 'All', count: comments.length },
		{ value: 'new', label: 'New', count: replyStatusSummary.new },
		{ value: 'needs_reply', label: 'Needs reply', count: replyStatusSummary.needs_reply },
		{ value: 'replied', label: 'Replied', count: replyStatusSummary.replied },
		{ value: 'skipped', label: 'Skipped', count: replyStatusSummary.skipped },
	]);
	const sentimentChips = $derived([
		{ value: '', label: 'All', count: comments.length },
		{ value: 'positive', label: 'Positive', count: sentimentSummary.positive },
		{ value: 'neutral', label: 'Neutral', count: sentimentSummary.neutral },
		{ value: 'negative', label: 'Negative', count: sentimentSummary.negative },
	]);

	function formatDate(iso: string): string {
		return formatDateTime(iso);
	}

	const REPLY_STATUS_COLORS: Record<string, string> = {
		new: 'badge--neutral',
		needs_reply: 'badge--warning',
		replied: 'badge--success',
		skipped: 'badge--muted',
	};

	async function loadPosts() {
		try {
			posts = await listPosts().catch(() => []);
			if (posts.length > 0 && !selectedPostId) {
				selectedPostId = posts[0].id;
			}
		} catch (e: any) {
			toast(e.message || 'Failed to load posts', 'error');
		}
	}

	async function loadComments() {
		if (!selectedPostId) return;
		commentsLoading = true;
		try {
			comments = await listComments(selectedPostId);
		} catch (e: any) {
			toast(e.message || 'Failed to load comments', 'error');
			comments = [];
		} finally {
			commentsLoading = false;
		}
	}

	async function handleFetch() {
		if (!selectedPostId) return;
		fetchLoading = true;
		try {
			comments = await fetchComments(selectedPostId);
			toast(`Fetched ${comments.length} comments`, 'success');
		} catch (e: any) {
			toast(e.message || 'Failed to fetch comments', 'error');
		} finally {
			fetchLoading = false;
		}
	}

	async function handleStatusChange(commentId: string, status: string) {
		actionLoading = commentId;
		try {
			const updated = await updateCommentReply(commentId, { reply_status: status });
			const idx = comments.findIndex((c) => c.id === commentId);
			if (idx >= 0) comments[idx] = updated;
			toast(`Marked as ${status}`, 'success');
		} catch (e: any) {
			toast(e.message || 'Failed to update status', 'error');
		} finally {
			actionLoading = null;
		}
	}

	function startReply(commentId: string) {
		replyingTo = commentId;
		const existing = comments.find((c) => c.id === commentId)?.reply_text;
		replyText = existing ?? '';
	}

	function cancelReply() {
		replyingTo = null;
		replyText = '';
	}

	async function handleReply(commentId: string) {
		if (!replyText.trim()) {
			toast('Reply text cannot be empty', 'error');
			return;
		}
		replyLoading = true;
		try {
			const result = await replyToComment(commentId, replyText.trim());
			const idx = comments.findIndex((c) => c.id === commentId);
			if (idx >= 0) comments[idx] = result.data;
			toast('Reply published to Threads', 'success');
			cancelReply();
		} catch (e: any) {
			toast(e.message || 'Failed to publish reply', 'error');
		} finally {
			replyLoading = false;
		}
	}

	$effect(() => {
		if (selectedPostId) loadComments();
	});

	async function init() {
		loading = true;
		await loadPosts();
		loading = false;
		loaded = true;
	}

	$effect(() => {
		if (!loaded) init();
	});
</script>

<PageHeader title="Comments" description="Comment inbox with reply workflow & sentiment analysis" />

{#if posts.length > 0}
	<section class="toolbar" aria-label="Comment filters">
		<div class="toolbar-post">
			<div class="field field--grow">
				<label class="field-label" for="comments-post">Post</label>
				<Select.Root type="single" bind:value={selectedPostId}>
					<Select.Trigger id="comments-post" class="w-full">
						<span class="trigger-text">
							{selectedPost ? postLabel(selectedPost) : 'Select a post…'}
						</span>
					</Select.Trigger>
					<Select.Content class="post-menu">
						{#each posts as post (post.id)}
							<Select.Item value={post.id} label={postLabel(post)}>
								<span class="post-opt">
									<span class="post-opt-title">{postLabel(post)}</span>
									<span class="post-opt-meta">{postMeta(post)}</span>
								</span>
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<Button variant="default" onclick={handleFetch} disabled={!selectedPostId || fetchLoading} class="fetch-btn">
				{fetchLoading ? 'Fetching…' : 'Fetch new comments'}
			</Button>
		</div>

		{#if comments.length > 0}
			<div class="toolbar-filters">
				<div class="field">
					<span class="field-label" id="lbl-status">Reply status</span>
					<div class="chips" role="group" aria-labelledby="lbl-status">
						{#each statusChips as c (c.value)}
							<button type="button" class="chip" class:chip--on={replyStatusFilter === c.value} aria-pressed={replyStatusFilter === c.value} onclick={() => (replyStatusFilter = c.value)}>
								{c.label}<span class="chip-count tabular-nums">{c.count}</span>
							</button>
						{/each}
					</div>
				</div>
				<div class="field">
					<span class="field-label" id="lbl-sent">Sentiment</span>
					<div class="chips" role="group" aria-labelledby="lbl-sent">
						{#each sentimentChips as c (c.value)}
							<button type="button" class="chip" class:chip--on={sentimentFilter === c.value} aria-pressed={sentimentFilter === c.value} onclick={() => (sentimentFilter = c.value)}>
								{c.label}<span class="chip-count tabular-nums">{c.count}</span>
							</button>
						{/each}
					</div>
				</div>
				<div class="field field--grow field--search">
					<label class="field-label" for="comments-search">Search</label>
					<Input id="comments-search" type="search" placeholder="Search comment text…" bind:value={searchQuery} />
				</div>
			</div>
		{/if}
	</section>
{/if}

{#if comments.length > 0 && !loading && !commentsLoading}
	<div class="results-line" role="status" aria-live="polite">
		<span data-testid="results-count">Showing {filteredComments.length} of {comments.length} comments</span>
		{#if filtersActive}
			<button type="button" class="guide-link" onclick={clearFilters}>Clear filters</button>
		{/if}
	</div>
{/if}

<div class="comments-list">
	{#if loading || commentsLoading}
		<div class="comment-card comment-card--skeleton">
			{#each Array(5) as _}
				<Skeleton class="h-16 w-full" />
			{/each}
		</div>
	{:else if comments.length === 0}
		<div class="empty-state">
			<p class="empty-state-title">No comments yet</p>
			<p class="empty-state-desc">Choose a post above, then click “Fetch new comments” to pull them in.</p>
		</div>
	{:else if filteredComments.length === 0}
		<div class="empty-state">
			<p class="empty-state-title">No comments match your filters</p>
			<p class="empty-state-desc">Try adjusting the filters above.</p>
		</div>
	{:else}
		{#each filteredComments as comment (comment.id)}
			<div class="comment-card" class:comment-card--replied={comment.reply_status === 'replied'}>
				<div class="comment-header">
					<div class="comment-author">
						<span class="comment-author-name">{comment.author_username ?? 'anonymous'}</span>
						<span class="comment-date">{formatDate(comment.fetched_at)}</span>
					</div>
					<div class="comment-badges">
						<StatusBadge status={comment.sentiment ?? 'neutral'} />
						<span class="badge {REPLY_STATUS_COLORS[comment.reply_status] ?? 'badge--neutral'}">
							{comment.reply_status.replace('_', ' ')}
						</span>
					</div>
				</div>

				<p class="comment-text">{comment.text}</p>

				{#if comment.reply_text}
					<div class="comment-reply-preview">
						<span class="reply-label">Reply:</span>
						<span class="reply-text">{truncate(comment.reply_text, 120)}</span>
					</div>
				{/if}

				{#if comment.sentiment_score !== null}
					<div class="comment-score">
						Score: {comment.sentiment_score.toFixed(2)}
					</div>
				{/if}

				<!-- Reply input (inline) -->
				{#if replyingTo === comment.id}
					<div class="reply-box">
						<Textarea
							placeholder="Type your reply..."
							bind:value={replyText}
							rows={2}
							class="textarea textarea--sm"
						/>
						<div class="reply-actions">
							<Button
								variant="default"
								size="sm"
								onclick={() => handleReply(comment.id)}
								disabled={replyLoading || !replyText.trim()}
							>
								{replyLoading ? 'Publishing...' : 'Publish Reply'}
							</Button>
							<Button variant="ghost" size="sm" onclick={cancelReply} disabled={replyLoading}>
								Cancel
							</Button>
						</div>
					</div>
				{:else}
					<!-- Action buttons -->
					<div class="comment-actions">
						{#if comment.reply_status !== 'replied' && comment.threads_comment_id}
							<Button
								variant="ghost"
								size="sm"
								onclick={() => startReply(comment.id)}
								disabled={actionLoading === comment.id}
							>
								Reply
							</Button>
						{/if}
						{#if comment.reply_status !== 'needs_reply'}
							<Button
								variant="ghost"
								size="sm"
								onclick={() => handleStatusChange(comment.id, 'needs_reply')}
								disabled={actionLoading === comment.id}
							>
								Mark Pending
							</Button>
						{/if}
						{#if comment.reply_status !== 'skipped'}
							<Button
								variant="ghost"
								size="sm"
								onclick={() => handleStatusChange(comment.id, 'skipped')}
								disabled={actionLoading === comment.id}
							>
								Skip
							</Button>
						{/if}
					</div>
				{/if}
			</div>
		{/each}
	{/if}
</div>

<style>
	.toolbar {
		display: flex;
		flex-direction: column;
		gap: var(--space-md);
		padding: var(--space-md);
		margin-bottom: var(--space-md);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		background: var(--color-surface);
	}

	.toolbar-post {
		display: flex;
		align-items: flex-end;
		gap: var(--space-sm);
	}

	.toolbar-filters {
		display: flex;
		flex-wrap: wrap;
		align-items: flex-end;
		gap: var(--space-md) var(--space-lg);
		padding-top: var(--space-md);
		border-top: 1px solid var(--color-border);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: var(--space-2xs);
		min-width: 0;
	}

	.field--grow {
		flex: 1 1 14rem;
	}

	.field--search {
		flex-basis: 16rem;
	}

	.field-label {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		color: var(--color-muted);
		text-transform: uppercase;
		letter-spacing: 0.08em;
	}

	.trigger-text {
		display: block;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: left;
	}

	:global(.toolbar-post [data-slot='select-trigger']) {
		min-width: 0;
	}

	:global(.fetch-btn) {
		flex: 0 0 auto;
	}

	.post-opt {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		max-width: min(34rem, 80vw);
	}

	.post-opt-title {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.post-opt-meta {
		font-size: var(--text-xs);
		color: var(--color-muted);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2xs);
	}

	.chip {
		display: inline-flex;
		align-items: center;
		gap: var(--space-xs);
		min-height: 2rem;
		padding: 0.15rem 0.7rem;
		border: var(--rule-default);
		border-radius: 999px;
		background: transparent;
		color: var(--color-muted);
		font-size: var(--text-sm);
		cursor: pointer;
		transition: border-color 0.15s ease, color 0.15s ease, background 0.15s ease;
	}

	.chip:hover {
		color: inherit;
	}

	.chip--on {
		color: inherit;
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
	}

	.chip-count {
		font-size: var(--text-xs);
		color: var(--color-muted);
	}

	.results-line {
		display: flex;
		align-items: center;
		gap: var(--space-sm);
		margin-bottom: var(--space-sm);
		font-size: var(--text-sm);
		color: var(--color-muted);
	}

	.guide-link {
		background: none;
		border: none;
		padding: 0;
		color: var(--color-accent);
		text-decoration: underline;
		cursor: pointer;
		font-size: inherit;
	}

	@media (max-width: 40rem) {
		.toolbar {
			padding: var(--space-sm);
		}

		.toolbar-post {
			flex-direction: column;
			align-items: stretch;
		}

		.toolbar-filters {
			flex-direction: column;
			align-items: stretch;
		}

		/* flex-basis would otherwise become a height in a column layout */
		.field--grow,
		.field--search {
			flex: 0 0 auto;
		}
	}

	.comments-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-sm);
	}

	.comment-card {
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		padding: var(--space-md);
		background: var(--color-surface);
		transition: border-color 0.15s ease;
	}

	.comment-card--replied {
		opacity: 0.7;
	}

	.comment-card--skeleton {
		gap: var(--space-sm);
	}

	.comment-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		margin-bottom: var(--space-xs);
	}

	.comment-author {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.comment-author-name {
		font-weight: 600;
		font-size: var(--text-sm);
	}

	.comment-date {
		font-size: var(--text-xs);
		color: var(--color-muted);
	}

	.comment-badges {
		display: flex;
		gap: var(--space-xs);
	}

	.comment-text {
		font-size: var(--text-sm);
		line-height: 1.5;
		margin: var(--space-xs) 0;
	}

	.comment-score {
		font-size: var(--text-xs);
		color: var(--color-muted);
	}

	.comment-reply-preview {
		display: flex;
		gap: var(--space-xs);
		padding: var(--space-xs) var(--space-sm);
		margin-top: var(--space-xs);
		background: var(--color-bg);
		border-radius: var(--radius-sm);
		font-size: var(--text-sm);
	}

	.reply-label {
		font-weight: 600;
		color: var(--color-muted);
	}

	.reply-text {
		color: var(--color-text);
	}

	.comment-actions {
		display: flex;
		gap: var(--space-xs);
		margin-top: var(--space-sm);
	}

	.reply-box {
		margin-top: var(--space-sm);
	}

	.reply-actions {
		display: flex;
		gap: var(--space-xs);
		margin-top: var(--space-xs);
	}

	.badge--warning {
		background: var(--color-warning-bg);
		color: var(--color-warning-text);
	}

	.badge--muted {
		background: var(--color-muted-bg);
		color: var(--color-muted);
	}
</style>
