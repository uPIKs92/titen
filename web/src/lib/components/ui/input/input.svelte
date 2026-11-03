<script lang="ts">
	import { cn, type WithElementRef, type WithoutChildren } from "$lib/utils.js";
	import type { HTMLInputAttributes } from "svelte/elements";

	let {
		ref = $bindable(null),
		value = $bindable(),
		class: className,
		"data-slot": dataSlot = "input",
		...restProps
	}: WithoutChildren<WithElementRef<HTMLInputAttributes>> = $props();
</script>

<input
	bind:this={ref}
	data-slot={dataSlot}
	class={cn(
		"plk-input",
		className
	)}
	bind:value
	{...restProps} />
