<script lang="ts">
	import { onMount } from "svelte";
	import Upload from "@lucide/svelte/icons/upload";
	import { inTauri } from "#lib/api";
	import * as AlertDialog from "#lib/components/ui/alert-dialog";
	import * as Resizable from "#lib/components/ui/resizable";
	import BulkEditDialog from "#lib/components/edit/BulkEditDialog.svelte";
	import EditMetadataDialog from "#lib/components/edit/EditMetadataDialog.svelte";
	import DevicesSheet from "#lib/components/devices/DevicesSheet.svelte";
	import BookArea from "#lib/components/library/BookArea.svelte";
	import DetailsPanel from "#lib/components/library/DetailsPanel.svelte";
	import FilterChips from "#lib/components/library/FilterChips.svelte";
	import Sidebar from "#lib/components/library/Sidebar.svelte";
	import StatusBar from "#lib/components/library/StatusBar.svelte";
	import TopBar from "#lib/components/library/TopBar.svelte";
	import Welcome from "#lib/components/library/Welcome.svelte";
	import { isEditable, plural } from "#lib/format";
	import { lib } from "#lib/stores/library.svelte";

	const vw = typeof window === "undefined" ? 1400 : Math.max(window.innerWidth, 900);
	const pct = (px: number) => Math.round((px / vw) * 1000) / 10;

	onMount(() => {
		void lib.init();
		let unlisten: (() => void) | undefined;
		let disposed = false;
		if (inTauri) {
			import("@tauri-apps/api/webview").then(({ getCurrentWebview }) =>
				getCurrentWebview().onDragDropEvent((e) => {
					const p = e.payload;
					if (p.type === "enter" || p.type === "over") lib.dragging = true;
					else if (p.type === "drop") {
						lib.dragging = false;
						if (lib.info) void lib.addPaths(p.paths);
					} else lib.dragging = false;
				})
			).then((u) => (disposed ? u() : (unlisten = u)));
		}
		return () => {
			disposed = true;
			unlisten?.();
		};
	});

	$effect(() => {
		lib.savePrefs();
	});

	$effect(() => {
		document.title = lib.info ? `${lib.info.name} · Shelfmark` : "Shelfmark";
	});

	function onkeydown(e: KeyboardEvent) {
		if (!lib.info || e.defaultPrevented) return;
		const mod = e.ctrlKey || e.metaKey;
		const key = e.key.toLowerCase();
		const editable = isEditable(e.target);
		const overlay = document.querySelector('[role="dialog"],[role="alertdialog"],[role="menu"]');
		if (overlay) return;

		if (mod && key === "f") {
			e.preventDefault();
			lib.searchEl?.focus();
			lib.searchEl?.select();
			return;
		}
		if (mod && key === "b") {
			e.preventDefault();
			lib.sidebarOpen = !lib.sidebarOpen;
			return;
		}
		if (editable || e.altKey) return;
		const el = e.target instanceof HTMLElement ? e.target : null;
		if (el?.closest("[role='slider']")) return;
		if (e.key === "Enter" && el?.closest("button, a") && !el.closest("[role='option']")) return;

		if (key === "/" && !mod) {
			e.preventDefault();
			lib.searchEl?.focus();
		} else if (mod && key === "a") {
			e.preventDefault();
			lib.selectAll();
		} else if (mod) {
			return;
		} else if (e.key === "Escape") {
			lib.clearSelection();
		} else if (e.key === "ArrowDown") {
			e.preventDefault();
			lib.move(0, 1, e.shiftKey);
		} else if (e.key === "ArrowUp") {
			e.preventDefault();
			lib.move(0, -1, e.shiftKey);
		} else if (e.key === "ArrowRight" && lib.view === "grid") {
			e.preventDefault();
			lib.move(1, 0, e.shiftKey);
		} else if (e.key === "ArrowLeft" && lib.view === "grid") {
			e.preventDefault();
			lib.move(-1, 0, e.shiftKey);
		} else if (e.key === "Home" || e.key === "End") {
			e.preventDefault();
			const i = e.key === "Home" ? 0 : lib.books.length - 1;
			lib.selectIndex(i, { shift: e.shiftKey });
			lib.scrollTo?.(i);
		} else if (e.key === "Enter") {
			e.preventDefault();
			lib.readSelected();
		} else if (key === "e") {
			e.preventDefault();
			lib.editSelected();
		} else if (key === "i") {
			lib.detailsOpen = !lib.detailsOpen;
		} else if (e.key === "Delete") {
			e.preventDefault();
			lib.askDelete();
		}
	}

	const sidebarMin = pct(200);
	const sidebarMax = pct(460);
	const detailsMin = pct(280);
	const detailsMax = pct(560);
</script>

<svelte:window {onkeydown} />

{#if !lib.ready}
	<div class="size-full bg-background"></div>
{:else if !lib.info}
	<Welcome />
{:else}
	<div class="flex h-screen flex-col">
		<TopBar />
		<Resizable.PaneGroup direction="horizontal" class="min-h-0 flex-1">
			{#if lib.sidebarOpen}
				<Resizable.Pane
					order={1}
					defaultSize={lib.sidebarSize}
					minSize={sidebarMin}
					maxSize={sidebarMax}
					onResize={(s) => (lib.sidebarSize = Math.round(s * 10) / 10)}
				>
					<Sidebar />
				</Resizable.Pane>
				<Resizable.Handle />
			{/if}
			<Resizable.Pane order={2} minSize={30}>
				<main class="flex h-full min-w-0 flex-col bg-background">
					<FilterChips />
					<div class="min-h-0 flex-1"><BookArea /></div>
				</main>
			</Resizable.Pane>
			{#if lib.detailsOpen}
				<Resizable.Handle />
				<Resizable.Pane
					order={3}
					defaultSize={lib.detailsSize}
					minSize={detailsMin}
					maxSize={detailsMax}
					onResize={(s) => (lib.detailsSize = Math.round(s * 10) / 10)}
				>
					<DetailsPanel />
				</Resizable.Pane>
			{/if}
		</Resizable.PaneGroup>
		<StatusBar />
	</div>

	{#if lib.dragging}
		<div class="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-background/70 backdrop-blur-sm">
			<div class="m-6 flex size-full max-h-[calc(100%-3rem)] flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed border-brand">
				<Upload class="size-8 text-brand" />
				<p class="text-lg font-semibold">Drop e-books to add them</p>
			</div>
		</div>
	{/if}

	<EditMetadataDialog bind:open={lib.editOpen} bookId={lib.editId} onsaved={() => lib.reload()} />
	<BulkEditDialog bind:open={lib.bulkOpen} bookIds={lib.bulkIds} onsaved={() => lib.reload()} />
	<DevicesSheet bind:open={lib.devicesOpen} selectedIds={lib.selectedIds} onchanged={() => lib.reload()} />

	<AlertDialog.Root bind:open={lib.metaOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>Download metadata for {plural(lib.selected.size, "book")}?</AlertDialog.Title>
				<AlertDialog.Description>
					Title, authors, description, tags, publisher, date, languages and cover are replaced with what Open Library
					and Google Books return. Books without a confident match are left unchanged. Your ratings are kept.
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action onclick={() => {
						lib.metaOpen = false;
						void lib.downloadMetadataSelected();
					}}>Download and apply</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>

	<AlertDialog.Root bind:open={lib.deleteOpen}>
		<AlertDialog.Content>
			<AlertDialog.Header>
				<AlertDialog.Title>Delete {plural(lib.selected.size, "book")}?</AlertDialog.Title>
				<AlertDialog.Description>
					{#if lib.selectedBooks.length === 1}
						"{lib.selectedBooks[0].title}" and its files will be moved to the library's trash.
					{:else}
						The selected books and all their files will be moved to the library's trash.
					{/if}
				</AlertDialog.Description>
			</AlertDialog.Header>
			<AlertDialog.Footer>
				<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
				<AlertDialog.Action variant="destructive" onclick={() => {
						lib.deleteOpen = false;
						void lib.deleteSelected();
					}}>Delete</AlertDialog.Action>
			</AlertDialog.Footer>
		</AlertDialog.Content>
	</AlertDialog.Root>
{/if}
