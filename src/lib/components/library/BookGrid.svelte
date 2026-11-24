<script lang="ts">
	import { untrack } from "svelte";
	import { createVirtualizer } from "@tanstack/svelte-virtual";
	import { lib } from "#lib/stores/library.svelte";
	import BookCard from "./BookCard.svelte";

	const PAD = 14;
	const GAP = 8;
	const ROW_GAP = 4;
	const META = 58;

	let el = $state<HTMLDivElement | null>(null);
	let width = $state(0);

	const cols = $derived(Math.max(1, Math.round((width - PAD * 2 + GAP) / (lib.coverSize + GAP))));
	const cardW = $derived((width - PAD * 2 - GAP * (cols - 1)) / cols);
	const rowH = $derived(Math.round((cardW - 12) * 1.5 + META) + ROW_GAP);
	const rowCount = $derived(Math.ceil(lib.books.length / cols));
	const showIndex = $derived(lib.sortBy === "series");

	const virtualizer = createVirtualizer<HTMLDivElement, HTMLDivElement>({
		count: 0,
		getScrollElement: () => el,
		estimateSize: () => 240,
		overscan: 3,
		paddingStart: 8,
		paddingEnd: 16
	});

	$effect(() => {
		const count = rowCount;
		const h = rowH;
		const scroller = el;
		untrack(() => {
			if (!scroller) return;
			$virtualizer.setOptions({ count, estimateSize: () => h, getScrollElement: () => scroller });
			$virtualizer.measure();
		});
	});

	$effect(() => {
		lib.cols = cols;
	});

	$effect(() => {
		const scroller = el;
		if (!scroller) return;
		scroller.scrollTop = lib.scrollTops.grid;
		lib.scrollTo = (i) => $virtualizer.scrollToIndex(Math.floor(i / lib.cols), { align: "auto" });
		return () => {
			lib.scrollTo = null;
		};
	});

	function onscroll() {
		if (el) lib.scrollTops.grid = el.scrollTop;
	}
</script>

<div
	bind:this={el}
	bind:clientWidth={width}
	{onscroll}
	role="listbox"
	aria-multiselectable="true"
	aria-label="Books"
	class="thin-scroll size-full overflow-y-auto overflow-x-hidden"
>
	<div class="relative w-full" style="height:{$virtualizer.getTotalSize()}px">
		{#each $virtualizer.getVirtualItems() as row (row.key)}
			<div
				class="absolute top-0 left-0 grid w-full"
				style="transform:translateY({row.start}px);height:{rowH - ROW_GAP}px;padding:0 {PAD}px;gap:{GAP}px;grid-template-columns:repeat({cols},minmax(0,1fr))"
			>
				{#each lib.books.slice(row.index * cols, row.index * cols + cols) as book, k (book.id)}
					<BookCard {book} index={row.index * cols + k} width={cardW} {showIndex} />
				{/each}
			</div>
		{/each}
	</div>
</div>
