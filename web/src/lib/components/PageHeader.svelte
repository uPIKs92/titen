<script lang="ts">
	interface Props {
		title: string;
		description?: string;
		action?: import('svelte').Snippet;
	}
	let { title, description, action }: Props = $props();
</script>

<div class="plk-page-header">
	<div class="plk-page-header-text">
		<h1 class="plk-page-title">{title}</h1>
		{#if description}
			<p class="plk-page-subtitle">{description}</p>
		{/if}
	</div>
	{#if action}
		<div class="plk-page-actions">
			{@render action()}
		</div>
	{/if}
</div>
