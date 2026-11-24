<script lang="ts">
	import { mode, setMode } from "mode-watcher";
	import LayoutGrid from "@lucide/svelte/icons/layout-grid";
	import Monitor from "@lucide/svelte/icons/monitor";
	import Moon from "@lucide/svelte/icons/moon";
	import PanelLeft from "@lucide/svelte/icons/panel-left";
	import PanelRight from "@lucide/svelte/icons/panel-right";
	import Plus from "@lucide/svelte/icons/plus";
	import Sun from "@lucide/svelte/icons/sun";
	import Table2 from "@lucide/svelte/icons/table-2";
	import Tablet from "@lucide/svelte/icons/tablet-smartphone";
	import { Button } from "#lib/components/ui/button";
	import * as DropdownMenu from "#lib/components/ui/dropdown-menu";
	import { Slider } from "#lib/components/ui/slider";
	import { lib } from "#lib/stores/library.svelte";
	import IconButton from "./IconButton.svelte";
	import LibrarySwitcher from "./LibrarySwitcher.svelte";
	import SearchBar from "./SearchBar.svelte";
	import SortMenu from "./SortMenu.svelte";
</script>

<header class="flex h-12 shrink-0 items-center gap-1.5 border-b bg-background px-2">
	<IconButton
		label={lib.sidebarOpen ? "Hide sidebar" : "Show sidebar"}
		shortcut="Ctrl+B"
		icon={PanelLeft}
		onclick={() => (lib.sidebarOpen = !lib.sidebarOpen)}
		aria-pressed={lib.sidebarOpen}
	/>
	<LibrarySwitcher />
	<div class="mx-1 flex min-w-0 max-w-xl flex-1 items-center">
		<SearchBar />
	</div>
	<div class="ml-auto flex shrink-0 items-center gap-1">
		<SortMenu />
		<div class="flex items-center rounded-md bg-muted/70 p-0.5" role="group" aria-label="View">
			<IconButton
				label="Cover grid"
				icon={LayoutGrid}
				size="icon-xs"
				variant={lib.view === "grid" ? "secondary" : "ghost"}
				class={lib.view === "grid" ? "bg-background shadow-xs hover:bg-background" : ""}
				aria-pressed={lib.view === "grid"}
				onclick={() => (lib.view = "grid")}
			/>
			<IconButton
				label="Table"
				icon={Table2}
				size="icon-xs"
				variant={lib.view === "table" ? "secondary" : "ghost"}
				class={lib.view === "table" ? "bg-background shadow-xs hover:bg-background" : ""}
				aria-pressed={lib.view === "table"}
				onclick={() => (lib.view = "table")}
			/>
		</div>
		{#if lib.view === "grid"}
			<div class="hidden w-24 items-center px-2 xl:flex" title="Cover size">
				<Slider type="single" bind:value={lib.coverSize} min={110} max={280} step={2} aria-label="Cover size" />
			</div>
		{/if}
		<Button size="sm" class="gap-1.5 px-2.5" onclick={() => lib.pickAndAdd()}>
			<Plus />
			<span class="hidden xl:inline">Add books</span>
			<span class="sr-only xl:hidden">Add books</span>
		</Button>
		<IconButton label="Devices" icon={Tablet} onclick={() => (lib.devicesOpen = true)} />
		<DropdownMenu.Root>
			<DropdownMenu.Trigger>
				{#snippet child({ props })}
					<IconButton {...props} label="Theme" icon={mode.current === "dark" ? Moon : Sun} />
				{/snippet}
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="w-36">
				<DropdownMenu.Item onSelect={() => setMode("light")}><Sun />Light</DropdownMenu.Item>
				<DropdownMenu.Item onSelect={() => setMode("dark")}><Moon />Dark</DropdownMenu.Item>
				<DropdownMenu.Item onSelect={() => setMode("system")}><Monitor />System</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
		<IconButton
			label={lib.detailsOpen ? "Hide details" : "Show details"}
			shortcut="I"
			icon={PanelRight}
			onclick={() => (lib.detailsOpen = !lib.detailsOpen)}
			aria-pressed={lib.detailsOpen}
		/>
	</div>
</header>
