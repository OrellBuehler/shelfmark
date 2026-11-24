<script lang="ts">
	import { untrack } from "svelte";
	import { createVirtualizer } from "@tanstack/svelte-virtual";
	import ChevronRight from "@lucide/svelte/icons/chevron-right";
	import Search from "@lucide/svelte/icons/search";
	import X from "@lucide/svelte/icons/x";
	import type { CategoryItem } from "#lib/api";
	import { languageName } from "#lib/format";
	import { lib } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";
	import Stars from "./Stars.svelte";

	const LABELS: Record<string, string> = {
		authors: "Authors",
		series: "Series",
		tags: "Tags",
		publisher: "Publishers",
		publishers: "Publishers",
		languages: "Languages",
		formats: "Formats",
		rating: "Rating",
		identifiers: "Identifiers"
	};
	const ROW = 30;

	type Row =
		| { kind: "head"; key: string; label: string; count: number; open: boolean }
		| { kind: "item"; key: string; item: CategoryItem };

	let filter = $state("");
	let el = $state<HTMLDivElement | null>(null);

	function isOpen(key: string, filtering: boolean) {
		return filtering || lib.collapsed[key] === false;
	}

	const rows = $derived.by(() => {
		const f = filter.trim().toLowerCase();
		const out: Row[] = [];
		for (const cat of lib.categories) {
			const items = f ? cat.items.filter((i) => i.name.toLowerCase().includes(f)) : cat.items;
			if (f && !items.length) continue;
			if (!cat.items.length) continue;
			const open = isOpen(cat.key, !!f);
			out.push({ kind: "head", key: cat.key, label: LABELS[cat.key] ?? cat.key, count: items.length, open });
			if (open) for (const item of items) out.push({ kind: "item", key: cat.key, item });
		}
		return out;
	});

	const virtualizer = createVirtualizer<HTMLDivElement, HTMLDivElement>({
		count: 0,
		getScrollElement: () => el,
		estimateSize: () => ROW,
		overscan: 12
	});

	$effect(() => {
		const count = rows.length;
		const scroller = el;
		untrack(() => {
			if (!scroller) return;
			$virtualizer.setOptions({
				count,
				getScrollElement: () => scroller,
				getItemKey: (i) => {
					const r = rows[i];
					return r.kind === "head" ? `h:${r.key}` : `i:${r.key}:${r.item.search}`;
				}
			});
			$virtualizer.measure();
		});
	});

	function toggle(key: string) {
		lib.collapsed = { ...lib.collapsed, [key]: lib.collapsed[key] === false };
	}

	function display(key: string, item: CategoryItem) {
		return key === "languages" ? languageName(item.name) : item.name;
	}

	function ratingValue(item: CategoryItem) {
		const m = /^(\d)/.exec(item.name);
		return m ? Number(m[1]) * 2 : null;
	}
</script>

<div class="flex h-full min-h-0 flex-col bg-sidebar text-sidebar-foreground">
	<div class="p-2 pb-1">
		<div class="relative">
			<Search class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
			<input
				bind:value={filter}
				placeholder="Filter categories"
				aria-label="Filter categories"
				class="h-7 w-full rounded-md border border-transparent bg-background/60 pr-6 pl-7 text-xs outline-none select-text placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/40"
				onkeydown={(e) => {
					if (e.key === "Escape") {
						filter = "";
						e.stopPropagation();
					}
				}}
			/>
			{#if filter}
				<button
					type="button"
					aria-label="Clear filter"
					class="absolute top-1/2 right-1 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:text-foreground"
					onclick={() => (filter = "")}
				>
					<X class="size-3" />
				</button>
			{/if}
		</div>
	</div>
	<div bind:this={el} class="thin-scroll min-h-0 flex-1 overflow-y-auto px-1.5 pb-2">
		{#if !lib.categories.length && !lib.loaded}
			<div class="space-y-2 p-2">
				{#each Array(8) as _, i (i)}
					<div class="h-4 animate-pulse rounded bg-muted" style="width:{50 + ((i * 37) % 45)}%"></div>
				{/each}
			</div>
		{:else if !rows.length}
			<p class="px-3 py-6 text-center text-xs text-muted-foreground">
				{filter || lib.query ? "No matching items" : "Nothing to browse yet"}
			</p>
		{:else}
			<div class="relative w-full" style="height:{$virtualizer.getTotalSize()}px">
				{#each $virtualizer.getVirtualItems() as vi (vi.key)}
					{@const row = rows[vi.index]}
					{#if row}
						<div class="absolute top-0 left-0 w-full" style="height:{ROW}px;transform:translateY({vi.start}px)">
							{#if row.kind === "head"}
								<button
									type="button"
									class="mt-1 flex h-[26px] w-full items-center gap-1 rounded-md px-1.5 text-[11px] font-semibold tracking-wider text-muted-foreground uppercase outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
									aria-expanded={row.open}
									onclick={() => toggle(row.key)}
								>
									<ChevronRight class={cn("size-3.5 transition-transform", row.open && "rotate-90")} />
									<span class="flex-1 text-left">{row.label}</span>
									<span class="font-normal tabular-nums opacity-70">{row.count.toLocaleString()}</span>
								</button>
							{:else}
								{@const item = row.item}
								{@const active = lib.isActive(item)}
								<button
									type="button"
									title={display(row.key, item)}
									aria-pressed={active}
									class={cn(
										"flex h-[26px] w-full items-center gap-2 rounded-md pr-2 pl-6 text-left text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring",
										active ? "bg-brand/25 font-medium text-foreground dark:bg-brand/20" : "hover:bg-accent"
									)}
									onclick={(e) => lib.toggleFilter(item, e.ctrlKey || e.metaKey)}
								>
									{#if row.key === "rating" && ratingValue(item)}
										<Stars value={ratingValue(item)} size={11} class="flex-1" />
									{:else}
										<span class="min-w-0 flex-1 truncate">{display(row.key, item)}</span>
									{/if}
									<span class="text-xs text-muted-foreground tabular-nums">{item.count.toLocaleString()}</span>
								</button>
							{/if}
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>
</div>
