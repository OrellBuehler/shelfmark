<script lang="ts">
	import { cn } from "$lib/utils.js";

	let {
		checked = $bindable(false),
		id,
		label,
		class: className
	}: { checked?: boolean; id?: string; label: string; class?: string } = $props();
</script>

<button
	type="button"
	role="switch"
	{id}
	aria-label={label}
	aria-checked={checked}
	class={cn(
		"bg-input aria-checked:bg-primary dark:aria-checked:bg-primary focus-visible:ring-ring/50 relative h-5 w-9 shrink-0 rounded-full outline-none transition-colors focus-visible:ring-3 dark:bg-input/80",
		className
	)}
	onclick={() => (checked = !checked)}
>
	<span
		class="bg-background dark:bg-foreground dark:in-aria-checked:bg-primary-foreground pointer-events-none absolute top-0.5 left-0.5 size-4 rounded-full shadow-sm transition-transform in-aria-checked:translate-x-4"
	></span>
</button>
