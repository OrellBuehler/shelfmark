<script lang="ts">
	import Star from "@lucide/svelte/icons/star";
	import { cn } from "#lib/utils";

	let {
		value,
		size = 14,
		onchange,
		class: className
	}: {
		value: number | null;
		size?: number;
		onchange?: (rating: number) => void;
		class?: string;
	} = $props();

	let hover = $state<number | null>(null);
	const stars = $derived((value ?? 0) / 2);
	const shown = $derived(hover ?? stars);
</script>

{#if onchange}
	<div
		class={cn("inline-flex items-center gap-0.5", className)}
		role="group"
		aria-label="Rating"
		onmouseleave={() => (hover = null)}
	>
		{#each [1, 2, 3, 4, 5] as n (n)}
			<button
				type="button"
				aria-pressed={stars === n}
				aria-label={`${n} ${n === 1 ? "star" : "stars"}`}
				class="rounded-sm p-0.5 outline-none focus-visible:ring-2 focus-visible:ring-ring"
				onmouseenter={() => (hover = n)}
				onclick={() => onchange(n === stars ? 0 : n * 2)}
			>
				<Star
					style="width:{size}px;height:{size}px"
					class={cn(
						"transition-colors",
						n <= shown ? "fill-brand text-brand" : "text-muted-foreground/40"
					)}
				/>
			</button>
		{/each}
	</div>
{:else}
	<span class={cn("inline-flex items-center gap-px", className)} role="img" aria-label={`${stars} of 5 stars`}>
		{#each [1, 2, 3, 4, 5] as n (n)}
			<Star
				style="width:{size}px;height:{size}px"
				class={n <= stars ? "fill-brand text-brand" : "text-muted-foreground/30"}
			/>
		{/each}
	</span>
{/if}
