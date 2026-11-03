<script lang="ts">
	import '../app.css';
	import { onMount } from 'svelte';
	import { afterNavigate } from '$app/navigation';
	import { fetchTimezone } from '$lib/tz';

	let { children }: { children: import('svelte').Snippet } = $props();

	// Prefetch timezone from backend on app load
	let plk: typeof import('$lib/plk/behaviors.js') | undefined;
	onMount(() => {
		fetchTimezone();
		// Theme-kit behaviors (theme toggle, sidebar, tooltips). Manual init so
		// SvelteKit client navigation can re-scan the DOM.
		(window as unknown as { PLK_MANUAL?: boolean }).PLK_MANUAL = true;
		import('$lib/plk/behaviors.js').then((m) => {
			plk = m;
			plk.init(document.body);
		});
	});
	afterNavigate(() => plk?.init(document.body));
</script>

<svelte:head>
	<title>Titen — Self-hosted Threads Management</title>
</svelte:head>

{@render children()}
