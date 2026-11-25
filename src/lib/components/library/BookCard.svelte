<script lang="ts">
	import { isAudioEntry, type BookSummary } from "#lib/api";
	import { formatDuration } from "#lib/format";
	import { lib } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";
	import AudioBadge from "./AudioBadge.svelte";
	import Cover from "./Cover.svelte";

	let {
		book,
		index,
		width,
		showIndex
	}: { book: BookSummary; index: number; width: number; showIndex: boolean } = $props();

	const selected = $derived(lib.selected.has(book.id));
	const progress = $derived(lib.progress[book.id] ?? 0);
	const shown = $derived(book.formats.slice(0, 3));
	const audioEntry = $derived(isAudioEntry(book));

	function onclick(e: MouseEvent) {
		lib.selectIndex(index, { shift: e.shiftKey, toggle: e.ctrlKey || e.metaKey });
	}
</script>

<button
	type="button"
	tabindex="-1"
	data-book-id={book.id}
	role="option"
	aria-selected={selected}
	class={cn(
		"group/card flex w-full flex-col rounded-lg p-1.5 text-left outline-none transition-colors",
		selected ? "bg-brand/20 dark:bg-brand/15" : "hover:bg-accent/70"
	)}
	{onclick}
	ondblclick={() => lib.openEntry(book)}
>
	<div
		class={cn(
			"relative rounded-[4px] shadow-sm transition-shadow duration-150 group-hover/card:shadow-md",
			selected && "ring-2 ring-brand ring-offset-2 ring-offset-background"
		)}
	>
		<Cover {book} width={Math.ceil(width * 2)} />
		{#if showIndex && book.series}
			<span
				class="absolute top-1.5 left-1.5 rounded-md bg-black/65 px-1.5 py-0.5 text-[10px] leading-none font-semibold text-white tabular-nums backdrop-blur-sm"
			>
				#{book.seriesIndex}
			</span>
		{/if}
		{#if book.audio}
			<AudioBadge {book} overlay class="pointer-events-none absolute top-1.5 right-1.5" />
		{/if}
		<div class="pointer-events-none absolute inset-x-1.5 bottom-1.5 flex flex-wrap gap-1">
			{#each shown as f (f)}
				<span
					class="rounded bg-black/60 px-1 py-px text-[9px] leading-tight font-semibold tracking-wide text-white/95 uppercase backdrop-blur-sm"
					>{f}</span
				>
			{/each}
			{#if book.formats.length > shown.length}
				<span class="rounded bg-black/60 px-1 py-px text-[9px] leading-tight font-semibold text-white/95 backdrop-blur-sm"
					>+{book.formats.length - shown.length}</span
				>
			{/if}
			{#if audioEntry && book.audio}
				<span
					class="rounded bg-black/60 px-1 py-px text-[9px] leading-tight font-semibold tracking-wide text-white/95 uppercase backdrop-blur-sm"
					>{book.audio.format}</span
				>
				<span class="rounded bg-black/60 px-1 py-px text-[9px] leading-tight font-semibold text-white/95 backdrop-blur-sm"
					>{formatDuration(book.audio.durationMs)}</span
				>
			{/if}
		</div>
		{#if progress > 0}
			<div class="absolute inset-x-0 bottom-0 h-1 overflow-hidden rounded-b-[3px] bg-black/30">
				<div class="h-full bg-brand" style="width:{Math.round(progress * 100)}%"></div>
			</div>
		{/if}
	</div>
	<div class="mt-2 min-w-0 px-0.5">
		<div class="truncate text-[13px] leading-tight font-medium" title={book.title}>{book.title}</div>
		<div class="mt-0.5 truncate text-xs text-muted-foreground">{book.authors.join(", ") || "Unknown"}</div>
	</div>
</button>
