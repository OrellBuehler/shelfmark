<script lang="ts">
	import ArrowDownWideNarrow from "@lucide/svelte/icons/arrow-down-wide-narrow";
	import ArrowUpNarrowWide from "@lucide/svelte/icons/arrow-up-narrow-wide";
	import type { SortField } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import * as DropdownMenu from "#lib/components/ui/dropdown-menu";
	import { audio } from "#lib/stores/audio.svelte";
	import { lib } from "#lib/stores/library.svelte";

	const all: { value: SortField; label: string }[] = [
		{ value: "title", label: "Title" },
		{ value: "author", label: "Author" },
		{ value: "series", label: "Series" },
		{ value: "added", label: "Date added" },
		{ value: "published", label: "Date published" },
		{ value: "modified", label: "Date modified" },
		{ value: "rating", label: "Rating" },
		{ value: "size", label: "Size" },
		{ value: "publisher", label: "Publisher" },
		{ value: "duration", label: "Length" }
	];
	const fields = $derived(audio.hasAudio && lib.scope !== "ebooks" ? all : all.filter((f) => f.value !== "duration"));
	const current = $derived(fields.find((f) => f.value === lib.sortBy)?.label ?? "Sort");
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			<Button {...props} variant="ghost" size="sm" class="gap-1.5 px-2 text-muted-foreground" aria-label="Sort books">
				{#if lib.descending}<ArrowDownWideNarrow />{:else}<ArrowUpNarrowWide />{/if}
				<span class="hidden text-foreground lg:inline">{current}</span>
			</Button>
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-48">
		<DropdownMenu.Label class="px-2 py-1 text-xs font-normal text-muted-foreground">Sort by</DropdownMenu.Label>
		<DropdownMenu.RadioGroup value={lib.sortBy} onValueChange={(v) => lib.setSort(v as SortField, lib.descending)}>
			{#each fields as f (f.value)}
				<DropdownMenu.RadioItem value={f.value}>{f.label}</DropdownMenu.RadioItem>
			{/each}
		</DropdownMenu.RadioGroup>
		<DropdownMenu.Separator />
		<DropdownMenu.RadioGroup value={lib.descending ? "desc" : "asc"} onValueChange={(v) => lib.setDescending(v === "desc")}>
			<DropdownMenu.RadioItem value="asc">Ascending</DropdownMenu.RadioItem>
			<DropdownMenu.RadioItem value="desc">Descending</DropdownMenu.RadioItem>
		</DropdownMenu.RadioGroup>
	</DropdownMenu.Content>
</DropdownMenu.Root>
