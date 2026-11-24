<script lang="ts">
	import ArrowLeft from "@lucide/svelte/icons/arrow-left";
	import Bookmark from "@lucide/svelte/icons/bookmark";
	import Maximize from "@lucide/svelte/icons/maximize";
	import Minimize from "@lucide/svelte/icons/minimize";
	import NotebookPen from "@lucide/svelte/icons/notebook-pen";
	import Search from "@lucide/svelte/icons/search";
	import TableOfContents from "@lucide/svelte/icons/table-of-contents";
	import AppearanceMenu from "./AppearanceMenu.svelte";
	import type { Reader } from "./reader.svelte";
	import ToolButton from "./ToolButton.svelte";

	let { reader, onback }: { reader: Reader; onback: () => void } = $props();
</script>

<div class="flex h-12 items-center gap-1 px-2 sm:px-3">
	<ToolButton label="Back to library" onclick={onback}>
		<ArrowLeft />
	</ToolButton>
	<div class="mx-2 min-w-0 flex-1">
		<p class="truncate text-[13px] leading-tight font-medium">{reader.title}</p>
		{#if reader.author}
			<p class="text-muted-foreground truncate text-xs leading-tight">{reader.author}</p>
		{/if}
	</div>
	<div class="flex items-center gap-0.5">
		{#if reader.toc.length}
			<ToolButton label="Table of contents" shortcut="T" onclick={() => reader.openPanel("toc")}>
				<TableOfContents />
			</ToolButton>
		{/if}
		{#if reader.canSearch}
			<ToolButton label="Search in book" shortcut="Ctrl+F" onclick={() => reader.openPanel("search")}>
				<Search />
			</ToolButton>
		{/if}
		<ToolButton label="Highlights and bookmarks" onclick={() => reader.openPanel("notes")}>
			<NotebookPen />
		</ToolButton>
		<ToolButton
			label={reader.bookmarkHere ? "Remove bookmark" : "Bookmark this page"}
			shortcut="B"
			pressed={!!reader.bookmarkHere}
			onclick={() => reader.toggleBookmark()}
		>
			<Bookmark class={reader.bookmarkHere ? "fill-amber-500 text-amber-500" : ""} />
		</ToolButton>
		<AppearanceMenu {reader} />
		<ToolButton
			label="Fullscreen"
			shortcut="F"
			class="hidden sm:inline-flex"
			onclick={() => reader.toggleFullscreen()}
		>
			{#if reader.fullscreen}<Minimize />{:else}<Maximize />{/if}
		</ToolButton>
	</div>
</div>
