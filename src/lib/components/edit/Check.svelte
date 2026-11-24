<script lang="ts">
	import CheckIcon from "@lucide/svelte/icons/check";
	import { cn } from "$lib/utils.js";

	let {
		checked = $bindable(false),
		disabled = false,
		id,
		class: className
	}: { checked?: boolean; disabled?: boolean; id?: string; class?: string } = $props();
</script>

<button
	type="button"
	role="checkbox"
	{id}
	{disabled}
	aria-checked={checked}
	class={cn(
		"border-input dark:bg-input/30 focus-visible:border-ring focus-visible:ring-ring/50 aria-checked:bg-primary dark:aria-checked:bg-primary aria-checked:border-primary aria-checked:text-primary-foreground grid size-4 shrink-0 place-items-center rounded-[4px] border shadow-xs outline-none transition-colors focus-visible:ring-3 disabled:cursor-not-allowed disabled:opacity-50",
		className
	)}
	onclick={() => (checked = !checked)}
>
	{#if checked}
		<CheckIcon class="size-3.5" />
	{/if}
</button>
