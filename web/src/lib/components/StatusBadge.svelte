<script lang="ts">
	import type { Snippet } from "svelte";

	interface Props {
		status: string;
		children?: Snippet;
	}

	let { status, children }: Props = $props();

	type Tone = "success" | "warning" | "destructive" | "info" | "secondary";

	// Status → kit badge tone. Approved is soft success: it still awaits the scheduler.
	const TONES: Record<string, Tone> = {
		active: "success", ok: "success", valid: "success", published: "success",
		completed: "success", positive: "success", approved: "success",
		suspended: "warning", expired: "warning", pending: "warning",
		negative: "destructive", failed: "destructive",
		processing: "info",
		draft: "secondary", cancelled: "secondary", deleted: "secondary",
	};

	const tone = $derived(TONES[status] ?? "secondary");
</script>

<span class="plk-badge plk-badge--{tone} status-badge">
	{#if children}
		{@render children()}
	{:else}
		{status}
	{/if}
</span>

<style>
	/* Same width for every status in lists and tables, so rows align. */
	.status-badge {
		min-width: 5.5rem;
		justify-content: center;
		text-transform: capitalize;
	}
</style>
