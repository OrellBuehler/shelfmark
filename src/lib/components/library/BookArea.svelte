<script lang="ts">
	import BookOpen from "@lucide/svelte/icons/book-open";
	import BookPlus from "@lucide/svelte/icons/book-plus";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import Pencil from "@lucide/svelte/icons/pencil";
	import SearchX from "@lucide/svelte/icons/search-x";
	import Send from "@lucide/svelte/icons/send";
	import Sparkles from "@lucide/svelte/icons/sparkles";
	import Trash from "@lucide/svelte/icons/trash";
	import FileDown from "@lucide/svelte/icons/file-down";
	import { Button } from "#lib/components/ui/button";
	import * as ContextMenu from "#lib/components/ui/context-menu";
	import { Skeleton } from "#lib/components/ui/skeleton";
	import { lib } from "#lib/stores/library.svelte";
	import BookGrid from "./BookGrid.svelte";
	import BookTable from "./BookTable.svelte";

	const single = $derived(lib.selected.size === 1);
	const first = $derived(lib.selectedBooks[0]);
	const count = $derived(lib.selected.size);

	function oncontextmenu(e: MouseEvent) {
		const target = (e.target as HTMLElement).closest<HTMLElement>("[data-book-id]");
		if (target) lib.selectContextTarget(Number(target.dataset.bookId));
		else lib.clearSelection();
	}
</script>

{#if !lib.loaded}
	<div class="grid gap-4 overflow-hidden p-5" style="grid-template-columns:repeat(auto-fill,minmax({lib.coverSize}px,1fr))">
		{#each Array(24) as _, i (i)}
			<div class="space-y-2">
				<Skeleton class="aspect-[2/3] w-full rounded-[4px]" />
				<Skeleton class="h-3 w-4/5" />
				<Skeleton class="h-3 w-1/2" />
			</div>
		{/each}
	</div>
{:else if !lib.books.length}
	<div class="flex size-full flex-col items-center justify-center gap-3 p-8 text-center">
		{#if lib.query}
			<div class="flex size-12 items-center justify-center rounded-full bg-muted"><SearchX class="size-5 text-muted-foreground" /></div>
			<div>
				<h2 class="text-base font-semibold">No books found</h2>
				<p class="mt-1 max-w-sm text-sm text-muted-foreground">
					Nothing matches <code class="rounded bg-muted px-1.5 py-0.5 text-xs text-foreground select-text">{lib.query}</code>
				</p>
			</div>
			<Button variant="outline" size="sm" onclick={() => lib.clearAll()}>Clear search</Button>
		{:else}
			<div class="flex size-12 items-center justify-center rounded-full bg-brand/20"><BookPlus class="size-5 text-brand-foreground dark:text-brand" /></div>
			<div>
				<h2 class="text-base font-semibold">This library is empty</h2>
				<p class="mt-1 max-w-sm text-sm text-muted-foreground">Add e-books from your computer, or drop files anywhere in this window.</p>
			</div>
			<Button size="sm" onclick={() => lib.pickAndAdd()}><BookPlus />Add books</Button>
		{/if}
	</div>
{:else}
	<ContextMenu.Root>
		<ContextMenu.Trigger class="block size-full" {oncontextmenu}>
			{#if lib.view === "grid"}
				<BookGrid />
			{:else}
				<BookTable />
			{/if}
		</ContextMenu.Trigger>
		<ContextMenu.Content class="w-60">
			{#if count}
				{#if count > 1}
					<ContextMenu.Label class="px-2 py-1 text-xs font-normal text-muted-foreground">{count} books selected</ContextMenu.Label>
				{/if}
				<ContextMenu.Item disabled={!single} onSelect={() => lib.readSelected()}>
					<BookOpen />Read<ContextMenu.Shortcut>Enter</ContextMenu.Shortcut>
				</ContextMenu.Item>
				<ContextMenu.Item disabled={!single} onSelect={() => first && lib.openExternally(first)}>
					<ExternalLink />Open with system app
				</ContextMenu.Item>
				<ContextMenu.Separator />
				<ContextMenu.Item onSelect={() => lib.editSelected()}>
					<Pencil />{single ? "Edit metadata" : `Edit ${count} books`}<ContextMenu.Shortcut>E</ContextMenu.Shortcut>
				</ContextMenu.Item>
				<ContextMenu.Item onSelect={() => lib.askDownloadMetadata()}><Sparkles />Download and apply metadata</ContextMenu.Item>
				<ContextMenu.Item onSelect={() => lib.embedSelected()}><FileDown />Embed metadata into files</ContextMenu.Item>
				<ContextMenu.Separator />
				<ContextMenu.Item onSelect={() => lib.sendSelected()}><Send />Send to device</ContextMenu.Item>
				<ContextMenu.Item disabled={!single} onSelect={() => first && lib.showInFolder(first)}><FolderOpen />Show in folder</ContextMenu.Item>
				<ContextMenu.Separator />
				<ContextMenu.Item variant="destructive" onSelect={() => lib.askDelete()}>
					<Trash />Delete<ContextMenu.Shortcut>Del</ContextMenu.Shortcut>
				</ContextMenu.Item>
			{:else}
				<ContextMenu.Item onSelect={() => lib.pickAndAdd()}><BookPlus />Add books</ContextMenu.Item>
				<ContextMenu.Item onSelect={() => lib.selectAll()}>Select all<ContextMenu.Shortcut>Ctrl+A</ContextMenu.Shortcut></ContextMenu.Item>
			{/if}
		</ContextMenu.Content>
	</ContextMenu.Root>
{/if}
