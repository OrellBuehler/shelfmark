<script lang="ts">
	import { untrack } from "svelte";
	import { createVirtualizer } from "@tanstack/svelte-virtual";
	import ArrowDown from "@lucide/svelte/icons/arrow-down";
	import ArrowUp from "@lucide/svelte/icons/arrow-up";
	import type { SortField } from "$lib/api";
	import { coverSrc } from "$lib/covers";
	import { formatDate, formatSize } from "$lib/format";
	import { lib } from "$lib/stores/library.svelte";
	import { cn } from "$lib/utils";
	import Stars from "./Stars.svelte";

	const HEADER = 32;
	const ROW = 44;

	interface Column {
		key: string;
		label: string;
		w: number;
		sort?: SortField;
		right?: boolean;
	}

	const columns: Column[] = [
		{ key: "cover", label: "", w: 44 },
		{ key: "title", label: "Title", w: 300, sort: "title" },
		{ key: "authors", label: "Authors", w: 190, sort: "author" },
		{ key: "series", label: "Series", w: 180, sort: "series" },
		{ key: "tags", label: "Tags", w: 190 },
		{ key: "rating", label: "Rating", w: 92, sort: "rating" },
		{ key: "published", label: "Published", w: 96, sort: "published" },
		{ key: "added", label: "Added", w: 108, sort: "added" },
		{ key: "size", label: "Size", w: 76, sort: "size", right: true },
		{ key: "formats", label: "Formats", w: 130 },
		{ key: "publisher", label: "Publisher", w: 160, sort: "publisher" }
	];

	const widths = $derived(columns.map((c) => lib.columnWidths[c.key] ?? c.w));
	const total = $derived(widths.reduce((a, b) => a + b, 0) + 24);
	const template = $derived(widths.map((w, i) => (columns[i].key === "title" ? `minmax(${w}px,1fr)` : `${w}px`)).join(" "));

	let el = $state<HTMLDivElement | null>(null);

	const virtualizer = createVirtualizer<HTMLDivElement, HTMLDivElement>({
		count: 0,
		getScrollElement: () => el,
		estimateSize: () => ROW,
		overscan: 8,
		scrollMargin: HEADER,
		paddingEnd: 12
	});

	$effect(() => {
		const count = lib.books.length;
		const scroller = el;
		untrack(() => {
			if (!scroller) return;
			$virtualizer.setOptions({ count, getScrollElement: () => scroller });
			$virtualizer.measure();
		});
	});

	$effect(() => {
		lib.cols = 1;
		const scroller = el;
		if (!scroller) return;
		scroller.scrollTop = lib.scrollTops.table;
		lib.scrollTo = (i) => $virtualizer.scrollToIndex(i, { align: "auto" });
		return () => {
			lib.scrollTo = null;
		};
	});

	function onscroll() {
		if (el) lib.scrollTops.table = el.scrollTop;
	}

	function resize(e: PointerEvent, col: Column, i: number) {
		e.preventDefault();
		e.stopPropagation();
		const handle = e.currentTarget as HTMLElement;
		handle.setPointerCapture(e.pointerId);
		const startX = e.clientX;
		const startW = widths[i];
		const move = (ev: PointerEvent) => {
			lib.columnWidths = { ...lib.columnWidths, [col.key]: Math.max(48, Math.round(startW + ev.clientX - startX)) };
		};
		const up = () => {
			handle.removeEventListener("pointermove", move);
			handle.removeEventListener("pointerup", up);
		};
		handle.addEventListener("pointermove", move);
		handle.addEventListener("pointerup", up);
	}

	function seriesLabel(s: string, i: number) {
		return `${s} · ${Number.isInteger(i) ? i : i.toFixed(1)}`;
	}
</script>

<div
	bind:this={el}
	{onscroll}
	role="listbox"
	tabindex="-1"
	aria-multiselectable="true"
	aria-label="Books"
	class="thin-scroll size-full overflow-auto outline-none"
>
	<div style="min-width:{total}px">
		<div
			class="sticky top-0 z-10 grid items-center border-b bg-background/95 px-3 text-xs font-medium text-muted-foreground backdrop-blur"
			style="height:{HEADER}px;grid-template-columns:{template}"
			role="row"
		>
			{#each columns as col, i (col.key)}
				<div
					class="relative flex h-full items-center px-2 {col.right ? 'justify-end' : ''}"
					role="columnheader"
					aria-sort={col.sort && lib.sortBy === col.sort ? (lib.descending ? "descending" : "ascending") : undefined}
				>
					{#if col.sort}
						{@const sort = col.sort}
						<button
							type="button"
							class={cn(
								"-mx-1.5 flex items-center gap-1 rounded px-1.5 py-0.5 outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
								lib.sortBy === sort && "text-foreground"
							)}
							onclick={() => lib.setSort(sort)}
						>
							{col.label}
							{#if lib.sortBy === sort}
								{#if lib.descending}<ArrowDown class="size-3" />{:else}<ArrowUp class="size-3" />{/if}
							{/if}
						</button>
					{:else}
						<span>{col.label}</span>
					{/if}
					{#if col.key !== "cover"}
						<div
							role="separator"
							aria-orientation="vertical"
							class="absolute top-1.5 right-0 bottom-1.5 w-2 cursor-col-resize after:absolute after:inset-y-0 after:right-0 after:w-px after:bg-border hover:after:w-0.5 hover:after:bg-brand"
							onpointerdown={(e) => resize(e, col, i)}
						></div>
					{/if}
				</div>
			{/each}
		</div>
		<div class="relative" style="height:{$virtualizer.getTotalSize()}px">
			{#each $virtualizer.getVirtualItems() as row (row.key)}
				{@const book = lib.books[row.index]}
				{#if book}
					{@const selected = lib.selected.has(book.id)}
					<button
						type="button"
						tabindex="-1"
						role="option"
						aria-selected={selected}
						data-book-id={book.id}
						class={cn(
							"absolute top-0 left-0 grid w-full items-center border-b border-border/50 px-3 text-left text-[13px] outline-none",
							selected ? "bg-brand/15 dark:bg-brand/15" : "hover:bg-accent/60"
						)}
						style="height:{ROW}px;transform:translateY({row.start - HEADER}px);grid-template-columns:{template}"
						onclick={(e) => lib.selectIndex(row.index, { shift: e.shiftKey, toggle: e.ctrlKey || e.metaKey })}
						ondblclick={() => lib.readBook(book)}
					>
						{#if selected}<span class="absolute inset-y-0 left-0 w-0.5 bg-brand"></span>{/if}
						<span class="px-2">
							{#if book.hasCover}
								<img
									src={coverSrc(book, 96)}
									alt=""
									loading="lazy"
									decoding="async"
									draggable="false"
									class="h-8 w-[22px] rounded-[2px] bg-muted object-cover ring-1 ring-black/10 dark:ring-white/10"
								/>
							{:else}
								<span class="block h-8 w-[22px] rounded-[2px] bg-muted ring-1 ring-black/10 dark:ring-white/10"></span>
							{/if}
						</span>
						<span class="truncate px-2 font-medium">{book.title}</span>
						<span class="truncate px-2">{book.authors.join(", ")}</span>
						<span class="truncate px-2 text-muted-foreground">{book.series ? seriesLabel(book.series, book.seriesIndex) : ""}</span>
						<span class="truncate px-2 text-muted-foreground">{book.tags.join(", ")}</span>
						<span class="px-2">{#if book.rating}<Stars value={book.rating} size={11} />{/if}</span>
						<span class="truncate px-2 text-muted-foreground tabular-nums">{formatDate(book.pubdate, "month")}</span>
						<span class="truncate px-2 text-muted-foreground tabular-nums">{formatDate(book.timestamp)}</span>
						<span class="truncate px-2 text-right text-muted-foreground tabular-nums">{formatSize(book.size)}</span>
						<span class="flex gap-1 overflow-hidden px-2">
							{#each book.formats as f (f)}
								<span class="rounded bg-secondary px-1 py-px text-[10px] font-semibold text-secondary-foreground uppercase">{f}</span>
							{/each}
						</span>
						<span class="truncate px-2 text-muted-foreground">{book.publisher ?? ""}</span>
					</button>
				{/if}
			{/each}
		</div>
	</div>
</div>
