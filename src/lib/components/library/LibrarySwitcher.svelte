<script lang="ts">
	import Check from "@lucide/svelte/icons/check";
	import ChevronsUpDown from "@lucide/svelte/icons/chevrons-up-down";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import FolderPlus from "@lucide/svelte/icons/folder-plus";
	import Library from "@lucide/svelte/icons/library-big";
	import { Button } from "$lib/components/ui/button";
	import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
	import { lib } from "$lib/stores/library.svelte";
</script>

<DropdownMenu.Root onOpenChange={(o) => o && lib.refreshRecents()}>
	<DropdownMenu.Trigger>
		{#snippet child({ props })}
			<Button {...props} variant="ghost" size="sm" class="max-w-52 gap-1.5 px-2 font-semibold" aria-label="Switch library">
				<Library class="text-brand" />
				<span class="truncate">{lib.info?.name ?? "Library"}</span>
				<ChevronsUpDown class="size-3.5! text-muted-foreground" />
			</Button>
		{/snippet}
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="start" class="w-72">
		<DropdownMenu.Label class="px-2 py-1 text-xs font-normal text-muted-foreground">Libraries</DropdownMenu.Label>
		{#each lib.recents as r (r.path)}
			{@const current = r.path === lib.info?.path}
			<DropdownMenu.Item onSelect={() => !current && lib.openLibrary(r.path)} class="items-start gap-2">
				<div class="min-w-0 flex-1">
					<div class="truncate text-sm font-medium">{r.name}</div>
					<div class="truncate text-[11px] text-muted-foreground">{r.path}</div>
				</div>
				{#if current}<Check class="mt-0.5 size-4 text-brand" />{/if}
			</DropdownMenu.Item>
		{/each}
		<DropdownMenu.Separator />
		<DropdownMenu.Item onSelect={() => lib.pickAndOpen(false)}><FolderOpen />Open another library…</DropdownMenu.Item>
		<DropdownMenu.Item onSelect={() => lib.pickAndOpen(true)}><FolderPlus />Create new library…</DropdownMenu.Item>
	</DropdownMenu.Content>
</DropdownMenu.Root>
