<script lang="ts">
	import Search from "@lucide/svelte/icons/search";
	import X from "@lucide/svelte/icons/x";
	import { tick } from "svelte";
	import { cn } from "$lib/utils.js";
	import type { Reader } from "./reader.svelte";

	let { reader }: { reader: Reader } = $props();

	let input = $state<HTMLInputElement>();
	const s = $derived(reader.search);

	$effect(() => {
		if (reader.panel !== "search") return;
		void tick().then(() => setTimeout(() => input?.focus(), 80));
	});

	const offsets = $derived.by(() => {
		let n = 0;
		return s.groups.map((g) => {
			const start = n;
			n += g.hits.length;
			return start;
		});
	});

	async function pick(i: number) {
		reader.panel = null;
		await reader.goToHit(i);
	}
</script>

<div class="flex h-full min-h-0 flex-col">
	<form
		class="px-3 pt-3 pb-2"
		onsubmit={(e) => {
			e.preventDefault();
			void reader.runSearch();
		}}
	>
		<div
			class="border-input bg-background focus-within:ring-ring/40 focus-within:border-ring flex h-9 items-center gap-2 rounded-md border px-2.5 focus-within:ring-3"
		>
			<Search class="text-muted-foreground size-4 shrink-0" />
			<input
				bind:this={input}
				value={s.query}
				oninput={(e) => reader.queueSearch(e.currentTarget.value)}
				type="search"
				placeholder="Search in this book"
				aria-label="Search in this book"
				autocomplete="off"
				spellcheck="false"
				class="placeholder:text-muted-foreground min-w-0 flex-1 bg-transparent text-sm outline-none [&::-webkit-search-cancel-button]:hidden"
			/>
			{#if s.query}
				<button
					type="button"
					class="text-muted-foreground hover:text-foreground rounded p-0.5"
					aria-label="Clear search"
					onclick={() => {
						reader.clearSearch();
						input?.focus();
					}}
				>
					<X class="size-4" />
				</button>
			{/if}
		</div>
		<div class="text-muted-foreground mt-2 h-4 text-xs">
			{#if s.running}
				Searching… {Math.round(s.progress * 100)}%
			{:else if s.done}
				{#if s.total === 0}
					No results for “{s.query.trim()}”
				{:else}
					{s.total}{s.total >= 500 ? "+" : ""} result{s.total === 1 ? "" : "s"} in {s.groups.length}
					section{s.groups.length === 1 ? "" : "s"}
				{/if}
			{/if}
		</div>
	</form>
	<div class="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
		{#if !s.query && !s.groups.length}
			<p class="text-muted-foreground px-3 py-10 text-center text-sm">
				Search the full text of this book.<br />Matches are highlighted on the page.
			</p>
		{/if}
		{#each s.groups as group, gi (gi)}
			<section class="mt-2">
				<h3 class="text-muted-foreground truncate px-2 py-1.5 text-xs font-semibold tracking-wide uppercase">
					{group.label || "Untitled section"}
				</h3>
				<ul>
					{#each group.hits as hit, hi (hit.cfi)}
						{@const n = offsets[gi] + hi}
						<li>
							<button
								type="button"
								onclick={() => pick(n)}
								class={cn(
									"hover:bg-muted/70 w-full rounded-md px-2 py-2 text-left text-sm leading-snug",
									s.active === n && "bg-accent"
								)}
							>
								<span class="text-muted-foreground">{hit.excerpt.pre}</span><mark
									class="rounded-sm bg-amber-400/40 px-0.5 text-inherit">{hit.excerpt.match}</mark
								><span class="text-muted-foreground">{hit.excerpt.post}</span>
							</button>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	</div>
</div>
