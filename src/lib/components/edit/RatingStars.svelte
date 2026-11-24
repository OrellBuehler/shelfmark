<script lang="ts">
	import StarIcon from "@lucide/svelte/icons/star";
	import XIcon from "@lucide/svelte/icons/x";
	import { cn } from "#lib/utils.js";

	let {
		value = $bindable(0),
		disabled = false,
		id,
		class: className
	}: { value?: number; disabled?: boolean; id?: string; class?: string } = $props();

	let hover = $state<number | null>(null);
	const shown = $derived(hover ?? value);

	function set(v: number) {
		value = v === value ? 0 : v;
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === "ArrowRight" || e.key === "ArrowUp") value = Math.min(10, value + 1);
		else if (e.key === "ArrowLeft" || e.key === "ArrowDown") value = Math.max(0, value - 1);
		else if (e.key === "Backspace" || e.key === "Delete" || e.key === "0") value = 0;
		else if (/^[1-5]$/.test(e.key)) value = Number(e.key) * 2;
		else return;
		e.preventDefault();
	}
</script>

<div class={cn("flex items-center gap-2", className)}>
	<div
		{id}
		role="slider"
		tabindex={disabled ? -1 : 0}
		aria-label="Rating"
		aria-valuemin={0}
		aria-valuemax={10}
		aria-valuenow={value}
		aria-valuetext={value ? `${value / 2} of 5 stars` : "No rating"}
		class="focus-visible:ring-ring/50 flex rounded-md outline-none focus-visible:ring-3"
		onmouseleave={() => (hover = null)}
		{onkeydown}
	>
		{#each [1, 2, 3, 4, 5] as star (star)}
			{@const fill = Math.max(0, Math.min(2, shown - (star - 1) * 2))}
			<div class="relative size-7">
				<StarIcon class="text-muted-foreground/40 absolute inset-1 size-5" />
				<div class="absolute inset-0 overflow-hidden" style="width: {fill * 50}%">
					<StarIcon class="text-brand fill-brand absolute inset-1 size-5" />
				</div>
				{#if !disabled}
					<button
						type="button"
						tabindex="-1"
						aria-label="{star - 0.5} stars"
						class="absolute inset-y-0 left-0 w-1/2 cursor-pointer"
						onmouseenter={() => (hover = star * 2 - 1)}
						onclick={() => set(star * 2 - 1)}
					></button>
					<button
						type="button"
						tabindex="-1"
						aria-label="{star} stars"
						class="absolute inset-y-0 right-0 w-1/2 cursor-pointer"
						onmouseenter={() => (hover = star * 2)}
						onclick={() => set(star * 2)}
					></button>
				{/if}
			</div>
		{/each}
	</div>
	<span class="text-muted-foreground w-14 text-xs tabular-nums">
		{shown ? `${shown / 2} / 5` : "No rating"}
	</span>
	{#if value && !disabled}
		<button
			type="button"
			aria-label="Clear rating"
			class="text-muted-foreground hover:text-foreground -ml-1 grid size-5 place-items-center rounded"
			onclick={() => (value = 0)}
		>
			<XIcon class="size-3.5" />
		</button>
	{/if}
</div>
