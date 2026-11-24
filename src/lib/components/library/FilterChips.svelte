<script lang="ts">
	import X from "@lucide/svelte/icons/x";
	import { lib } from "$lib/stores/library.svelte";
</script>

{#if lib.filters.length}
	<div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b bg-background px-3 py-2">
		{#each lib.filters as f (f.search)}
			<span
				class="inline-flex h-6 items-center gap-1 rounded-full bg-brand/20 pr-1 pl-2.5 text-xs font-medium text-foreground ring-1 ring-brand/40 ring-inset"
			>
				{f.label}
				<button
					type="button"
					aria-label={`Remove filter ${f.label}`}
					class="rounded-full p-0.5 outline-none hover:bg-foreground/10 focus-visible:ring-2 focus-visible:ring-ring"
					onclick={() => lib.removeFilter(f.search)}
				>
					<X class="size-3" />
				</button>
			</span>
		{/each}
		{#if lib.filters.length > 1}
			<button
				type="button"
				class="rounded px-1.5 text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
				onclick={() => lib.clearAll()}
			>
				Clear all
			</button>
		{/if}
	</div>
{/if}
