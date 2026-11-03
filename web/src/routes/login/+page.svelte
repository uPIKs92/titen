<script lang="ts">
	import { goto } from '$app/navigation';
	import { loginWithApiKey } from '$lib/api';

	let apiKey = $state('');
	let error = $state('');
	let loading = $state(false);

	async function handleSubmit(e: Event) {
		e.preventDefault();
		error = '';
		loading = true;

		const trimmed = apiKey.trim();
		if (!trimmed) {
			error = 'API key is required.';
			loading = false;
			return;
		}

		try {
			const result = await loginWithApiKey(trimmed);
			if (!result.valid) {
				throw new Error('Invalid API key');
			}
			// Redirect to dashboard or original destination
			const params = new URLSearchParams(window.location.search);
			const redirect = params.get('redirect') || '/admin/dashboard';
			// Prevent open redirect — only allow relative paths within the app
			const safeRedirect = redirect.startsWith('/') && !redirect.startsWith('//') ? redirect : '/admin/dashboard';
			goto(safeRedirect);
		} catch {
			error = 'Invalid API key. Make sure it matches the TITEN_API_KEY on the server.';
		} finally {
			loading = false;
		}
	}
</script>

<svelte:head>
	<title>Sign in — Titen</title>
</svelte:head>

<div class="plk-backdrop" aria-hidden="true"></div>

<main class="plk-auth" id="auth">
	<div class="plk-auth-column">
		<div class="plk-auth-head">
			<p class="plk-tag">Titen</p>
			<h1 class="plk-auth-title">Sign in</h1>
			<p class="plk-auth-lede">
				Enter the API key configured on your Titen server
				(<code>TITEN_API_KEY</code> environment variable).
			</p>
		</div>

		<div class="plk-bezel">
			<div class="plk-bezel-core">
				<form class="plk-auth-body" onsubmit={handleSubmit} novalidate>
					{#if error}
						<div class="plk-alert plk-alert--destructive" role="alert">
							<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9" /><path d="M12 8v4M12 16h.01" /></svg>
							<div class="plk-alert-description">{error}</div>
						</div>
					{/if}
					<div class="plk-field">
						<label class="plk-label" for="apiKey">API Key</label>
						<input
							class="plk-input"
							id="apiKey"
							type="password"
							placeholder="titen_••••••••••••"
							bind:value={apiKey}
							autocomplete="off"
							autocapitalize="off"
							spellcheck="false"
							disabled={loading}
							aria-invalid={error ? 'true' : undefined}
						/>
						<p class="login-help">Required to access the admin dashboard.</p>
					</div>
					<button class="plk-btn plk-btn--lg plk-btn--pill plk-auth-submit" type="submit" disabled={loading}>
						{#if loading}
							Verifying…
						{:else}
							Sign in
							<span class="plk-btn-chip" aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M5 12h14M13 6l6 6-6 6" /></svg></span>
						{/if}
					</button>
				</form>
			</div>
		</div>

		<p class="plk-auth-foot">
			Don't have a key?
			<a href="https://github.com/codecoradev/titen#configuration" target="_blank" rel="noopener noreferrer">Read setup guide ↗</a>
		</p>
	</div>
</main>

<style>
	.login-help {
		margin: 0;
		font-size: var(--text-xs);
		color: var(--muted-foreground);
	}
</style>
