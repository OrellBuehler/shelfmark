<script lang="ts">
	import NotebookPen from "@lucide/svelte/icons/notebook-pen";
	import Search from "@lucide/svelte/icons/search";
	import TableOfContents from "@lucide/svelte/icons/table-of-contents";
	import * as Sheet from "#lib/components/ui/sheet/index.js";
	import * as Tabs from "#lib/components/ui/tabs/index.js";
	import NotesPanel from "./NotesPanel.svelte";
	import type { PanelTab, Reader } from "./reader.svelte";
	import SearchPanel from "./SearchPanel.svelte";
	import TocPanel from "./TocPanel.svelte";

	let { reader }: { reader: Reader } = $props();
</script>

<Sheet.Root
	open={reader.panel !== null}
	onOpenChange={(open) => {
		if (!open) reader.panel = null;
	}}
>
	<Sheet.Content
		side="left"
		showCloseButton={false}
		onOpenAutoFocus={(e) => e.preventDefault()}
		class="w-[min(400px,92vw)] gap-0 p-0 data-[side=left]:sm:max-w-[400px]"
	>
		<Sheet.Title class="sr-only">Book navigation</Sheet.Title>
		<Sheet.Description class="sr-only">Contents, search and notes</Sheet.Description>
		<Tabs.Root
			value={reader.panel ?? "toc"}
			onValueChange={(v) => (reader.panel = v as PanelTab)}
			class="flex min-h-0 flex-1 flex-col gap-0"
		>
			<div class="border-b px-3 py-3">
				<Tabs.List class="w-full">
					<Tabs.Trigger value="toc" disabled={!reader.toc.length}>
						<TableOfContents /> Contents
					</Tabs.Trigger>
					<Tabs.Trigger value="search" disabled={!reader.canSearch}>
						<Search /> Search
					</Tabs.Trigger>
					<Tabs.Trigger value="notes">
						<NotebookPen /> Notes
					</Tabs.Trigger>
				</Tabs.List>
			</div>
			<Tabs.Content value="toc" class="min-h-0 flex-1">
				<TocPanel {reader} />
			</Tabs.Content>
			<Tabs.Content value="search" class="min-h-0 flex-1">
				<SearchPanel {reader} />
			</Tabs.Content>
			<Tabs.Content value="notes" class="min-h-0 flex-1">
				<NotesPanel {reader} />
			</Tabs.Content>
		</Tabs.Root>
	</Sheet.Content>
</Sheet.Root>
