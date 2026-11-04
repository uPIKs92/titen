<script lang="ts">
import LayoutGrid from '@lucide/svelte/icons/layout-grid';
import Users from '@lucide/svelte/icons/users';
import PenSquare from '@lucide/svelte/icons/pen-square';
import CalendarClock from '@lucide/svelte/icons/calendar-clock';
import CalendarDays from '@lucide/svelte/icons/calendar-days';
import MessageSquare from '@lucide/svelte/icons/message-square';
import ChartLine from '@lucide/svelte/icons/chart-line';
import ImageIcon from '@lucide/svelte/icons/image';
import SettingsIcon from '@lucide/svelte/icons/settings';
import CheckCircle2 from '@lucide/svelte/icons/check-circle-2';
import XCircle from '@lucide/svelte/icons/x-circle';
import Info from '@lucide/svelte/icons/info';
import AtSign from '@lucide/svelte/icons/at-sign';
import Clock from '@lucide/svelte/icons/clock';
import LogOut from '@lucide/svelte/icons/log-out';
	import { getToasts } from '$lib/toast.svelte';
	import { page } from '$app/state';
	import { verifySession, logout, hasSessionHint, SessionUnreachableError } from '$lib/api';
	import { fetchTimezone, getTimezone, DEFAULT_TIMEZONE } from '$lib/tz';
	import { goto } from '$app/navigation';

	let { children }: { children: import('svelte').Snippet } = $props();

	// Auth guard: verify session cookie is valid, redirect to login if not
	let authed = $state(false);
	let appVersion = $state('');
	let tzLabel = $state('');
	let now = $state(new Date());

	// Live clock in the configured (server) timezone; falls back to the browser's.
	const clockTz = $derived.by(() => {
		const tz = tzLabel || DEFAULT_TIMEZONE;
		try {
			new Intl.DateTimeFormat('en-GB', { timeZone: tz });
			return tz;
		} catch {
			return DEFAULT_TIMEZONE;
		}
	});
	const clockText = $derived(
		new Intl.DateTimeFormat('en-GB', {
			timeZone: clockTz,
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit',
			hour12: false,
		}).format(now)
	);

	$effect(() => {
		const id = setInterval(() => (now = new Date()), 1000);
		return () => clearInterval(id);
	});

	// Session verification. With a recent-sign-in hint the shell renders at once
	// and is verified in the background; a slow/unreachable server shows a
	// retry banner instead of blocking or bouncing to the login page.
	let optimistic = $state(false);
	let checking = $state(true);
	let unreachable = $state('');
	let started = false;

	async function verify() {
		checking = true;
		unreachable = '';
		try {
			const session = await verifySession();
			if (session.authenticated) {
				authed = true;
				appVersion = session.version ?? '';
				tzLabel = (await fetchTimezone().catch(() => '')) || getTimezone() || '';
			} else {
				optimistic = false;
				const currentPath = page.url.pathname + page.url.search;
				goto(`/login?redirect=${encodeURIComponent(currentPath)}`);
			}
		} catch (e) {
			unreachable = e instanceof SessionUnreachableError ? e.message : 'Could not reach the server';
		} finally {
			checking = false;
		}
	}

	$effect(() => {
		if (started) return;
		started = true;
		optimistic = hasSessionHint();
		verify();
	});

	const showShell = $derived(authed || optimistic);

	async function handleLogout() {
		await logout();
		goto('/login');
	}

	const navItems = [
		{ href: '/admin/dashboard', label: 'Dashboard', icon: LayoutGrid },
		{ href: '/admin/accounts', label: 'Accounts', icon: Users },
		{ href: '/admin/posts', label: 'Posts', icon: PenSquare },
		{ href: '/admin/schedules', label: 'Schedules', icon: CalendarClock },
	{ href: '/admin/calendar', label: 'Calendar', icon: CalendarDays },
		{ href: '/admin/comments', label: 'Comments', icon: MessageSquare },
		{ href: '/admin/mentions', label: 'Mentions', icon: AtSign },
		{ href: '/admin/analytics', label: 'Analytics', icon: ChartLine },
		{ href: '/admin/media', label: 'Media', icon: ImageIcon },
		{ href: '/admin/settings', label: 'Settings', icon: SettingsIcon },
	] as const;

	function isActive(href: string): boolean {
		return page.url.pathname.startsWith(href);
	}
</script>

<svelte:head>
	<title>Titen Admin</title>
</svelte:head>

<a class="plk-skip-link" href="#main">Skip to content</a>

<div class="plk-shell">
	<aside class="plk-sidebar" aria-label="Admin navigation">
		<div class="plk-sidebar-header">
			<a class="plk-sidebar-brand" href="/admin/dashboard">
				<span class="plk-sidebar-brand-mark" aria-hidden="true">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 3l8 4.5v9L12 21l-4-2.2" /><path d="M4 16.5v-9L12 3" /></svg>
				</span>
				<span class="plk-sidebar-brand-text">
					<span class="plk-eyebrow" style="font-size: 0.625rem;">Threads</span>
					<span style="font-weight: 600; white-space: nowrap;">Titen</span>
				</span>
			</a>
		</div>
		<nav class="plk-sidebar-content">
			<div class="plk-sidebar-group">
				<div class="plk-sidebar-group-label">Workspace</div>
				<ul class="plk-sidebar-menu">
					{#each navItems as item}
						<li>
							<a
								class="plk-sidebar-link"
								href={item.href}
								aria-current={isActive(item.href) ? 'page' : undefined}
								data-plk-tooltip={item.label}
								data-plk-tooltip-side="right"
								data-plk-tooltip-collapsed
							>
								<item.icon aria-hidden="true" />
								<span class="plk-sidebar-text">{item.label}</span>
							</a>
						</li>
					{/each}
				</ul>
			</div>
		</nav>
		<div class="plk-sidebar-footer">
			<button class="plk-sidebar-link" type="button" onclick={handleLogout}>
				<LogOut aria-hidden="true" />
				<span class="plk-sidebar-text">Sign out</span>
			</button>
		</div>
	</aside>
	<div class="plk-sidebar-backdrop"></div>

	<div class="plk-inset">
		<header class="plk-topbar">
			<button class="plk-btn plk-btn--ghost plk-btn--icon plk-btn--sm" type="button" data-plk-sidebar-toggle aria-label="Toggle sidebar">
				<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M9 4v16" /></svg>
			</button>
			<hr class="plk-separator" aria-orientation="vertical" />
			<span class="plk-topbar-title">{navItems.find((n) => isActive(n.href))?.label ?? 'Admin'}</span>
			<div class="plk-topbar-end">
				<span class="plk-badge plk-badge--outline tz-clock" title="Times shown in your browser's timezone">
					<Clock aria-hidden="true" />
					<time datetime={now.toISOString()}>{clockText}</time>
					<span class="tz-name">{clockTz}</span>
				</span>
				<button class="plk-btn plk-btn--ghost plk-btn--icon plk-btn--sm" type="button" data-plk-theme-toggle aria-label="Toggle dark mode">
					<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" /></svg>
				</button>
			</div>
		</header>

		<main class="plk-main" id="main">
			<div class="admin-content">
				{#if showShell}
					{#if unreachable}
						<div class="plk-alert plk-alert--warning conn-banner" role="alert" data-testid="session-banner">
							<span>Can’t confirm your session — {unreachable}. Showing the last known view.</span>
							<button type="button" class="plk-btn plk-btn--outline plk-btn--sm" onclick={verify} disabled={checking}>{checking ? 'Retrying…' : 'Retry'}</button>
						</div>
					{/if}
					{@render children()}
				{:else if unreachable}
					<div class="auth-loading auth-loading--error" role="alert" data-testid="session-error">
						<p>Can’t reach the server — {unreachable}.</p>
						<button type="button" class="plk-btn plk-btn--outline plk-btn--sm" onclick={verify} disabled={checking}>{checking ? 'Retrying…' : 'Retry'}</button>
					</div>
				{:else}
					<div class="auth-loading" role="status">Verifying session…</div>
				{/if}
			</div>
		</main>

		<footer class="plk-statusbar admin-footer">
			<span class="plk-statusbar-item">Titen{#if appVersion}&nbsp;<span class="admin-version">v{appVersion}</span>{/if}</span>
		</footer>
	</div>
</div>

<!-- Toast container — polite live region; errors assert as role=alert -->
<div class="toast-container" role="status" aria-live="polite">
	{#each getToasts().toasts as t (t.id)}
		<div class="toast toast--{t.type}" role={t.type === 'error' ? 'alert' : undefined}>
			{#if t.type === 'success'}
				<CheckCircle2 class="toast-icon" aria-hidden="true" />
			{:else if t.type === 'error'}
				<XCircle class="toast-icon" aria-hidden="true" />
			{:else}
				<Info class="toast-icon" aria-hidden="true" />
			{/if}
			{t.message}
		</div>
	{/each}
</div>

<style>
	.tz-clock {
		gap: 0.375rem;
		font-family: var(--plk-font-mono);
		font-variant-numeric: tabular-nums;
	}

	.tz-clock :global(svg) {
		width: 0.875rem;
		height: 0.875rem;
	}

	.tz-name {
		color: var(--muted-foreground);
	}

	.admin-footer {
		margin-top: auto;
	}

	.admin-version {
		font-family: var(--plk-font-mono);
	}

	@media (max-width: 30rem) {
		.tz-name {
			display: none;
		}
	}

	.admin-content {
		display: flex;
		flex-direction: column;
		gap: var(--space-lg);
		width: 100%;
		min-width: 0;
		max-width: 90rem;
	}

	.auth-loading--error { flex-direction: column; gap: var(--space-sm); text-align: center; }
	.conn-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-sm);
		flex-wrap: wrap;
		margin-bottom: var(--space-md);
		font-size: var(--text-sm);
	}

	.auth-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 50vh;
		color: var(--muted-foreground);
		font-size: var(--text-sm);
	}
</style>
