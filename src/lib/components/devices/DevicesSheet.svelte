<script lang="ts">
	import TabletIcon from "@lucide/svelte/icons/tablet";
	import UsbIcon from "@lucide/svelte/icons/usb";
	import FolderIcon from "@lucide/svelte/icons/folder-open";
	import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
	import SearchIcon from "@lucide/svelte/icons/search";
	import SendIcon from "@lucide/svelte/icons/send";
	import Trash2Icon from "@lucide/svelte/icons/trash-2";
	import LoaderIcon from "@lucide/svelte/icons/loader-circle";
	import ArrowUpIcon from "@lucide/svelte/icons/arrow-up";
	import ArrowDownIcon from "@lucide/svelte/icons/arrow-down";
	import CircleCheckIcon from "@lucide/svelte/icons/circle-check";
	import CircleAlertIcon from "@lucide/svelte/icons/circle-alert";
	import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
	import BookOpenIcon from "@lucide/svelte/icons/book-open";
	import XIcon from "@lucide/svelte/icons/x";
	import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
	import { untrack } from "svelte";
	import { toast } from "svelte-sonner";
	import { api, inTauri, type Device, type DeviceBook, type DeviceKind } from "$lib/api";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Skeleton } from "$lib/components/ui/skeleton/index.js";
	import * as Sheet from "$lib/components/ui/sheet/index.js";
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
	import { cn } from "$lib/utils.js";
	import { humanSize } from "../edit/format";

	let {
		open = $bindable(false),
		selectedIds,
		onchanged
	}: { open: boolean; selectedIds: number[]; onchanged?: () => void } = $props();

	type SortKey = "title" | "authors" | "format" | "size";
	type Outcome = { bookId: number; title: string; error: string | null };

	const kindLabel: Record<DeviceKind, string> = {
		kobo: "Kobo",
		kindle: "Kindle",
		pocketBook: "PocketBook",
		tolino: "Tolino",
		generic: "E-reader",
		folder: "Folder"
	};

	let devices = $state<Device[]>([]);
	let devicesLoaded = $state(false);
	let devicesError = $state<string | null>(null);
	let refreshing = $state(false);
	let selectedId = $state<string | null>(null);
	let books = $state<DeviceBook[]>([]);
	let booksLoading = $state(false);
	let booksError = $state<string | null>(null);
	let search = $state("");
	let sortKey = $state<SortKey>("title");
	let sortDesc = $state(false);
	let sending = $state(false);
	let outcomes = $state<Outcome[] | null>(null);
	let deleting = $state<DeviceBook | null>(null);
	let booksToken = 0;

	const selected = $derived(devices.find((d) => d.id === selectedId) ?? null);

	const visibleBooks = $derived.by(() => {
		const q = search.trim().toLowerCase();
		const list = q
			? books.filter(
					(b) =>
						b.title.toLowerCase().includes(q) ||
						b.authors.join(" ").toLowerCase().includes(q) ||
						b.format.toLowerCase().includes(q)
				)
			: [...books];
		const dir = sortDesc ? -1 : 1;
		return list.sort((a, b) => {
			if (sortKey === "size") return (a.size - b.size) * dir;
			const av = sortKey === "authors" ? a.authors.join(", ") : a[sortKey];
			const bv = sortKey === "authors" ? b.authors.join(", ") : b[sortKey];
			return av.localeCompare(bv, undefined, { sensitivity: "base", numeric: true }) * dir;
		});
	});

	async function refresh(manual = false) {
		if (sending) return;
		if (manual) refreshing = true;
		try {
			devices = await api.detectDevices();
			devicesError = null;
			if (selectedId && !devices.some((d) => d.id === selectedId)) {
				selectedId = null;
				books = [];
			}
			if (!selectedId && devices.length === 1) selectDevice(devices[0]);
		} catch (e) {
			devicesError = String(e);
		} finally {
			devicesLoaded = true;
			refreshing = false;
		}
	}

	$effect(() => {
		if (!open) return;
		untrack(() => refresh());
		const timer = setInterval(refresh, 4000);
		return () => clearInterval(timer);
	});

	async function loadBooks(id: string) {
		const mine = ++booksToken;
		booksLoading = true;
		booksError = null;
		try {
			const list = await api.deviceBooks(id);
			if (mine === booksToken) books = list;
		} catch (e) {
			if (mine === booksToken) {
				booksError = String(e);
				books = [];
			}
		} finally {
			if (mine === booksToken) booksLoading = false;
		}
	}

	function selectDevice(d: Device) {
		if (selectedId === d.id) return;
		selectedId = d.id;
		books = [];
		search = "";
		outcomes = null;
		loadBooks(d.id);
	}

	function sortBy(key: SortKey) {
		if (sortKey === key) sortDesc = !sortDesc;
		else {
			sortKey = key;
			sortDesc = false;
		}
	}

	async function addFolder() {
		if (!inTauri) {
			toast.info("Picking a folder is only available in the desktop app");
			return;
		}
		const path = await pickDirectory({ directory: true, multiple: false });
		if (typeof path !== "string") return;
		try {
			const d = await api.addFolderDevice(path);
			await refresh();
			selectDevice(d);
		} catch (e) {
			toast.error(`Could not add folder: ${e}`);
		}
	}

	async function send() {
		if (!selected || sending || selectedIds.length === 0) return;
		const device = selected;
		const ids = [...selectedIds];
		sending = true;
		outcomes = null;
		try {
			const res = await api.sendToDevice(device.id, ids);
			const list: Outcome[] = await Promise.all(
				res.map(async (o) => {
					if (o.book) return { bookId: o.bookId, title: o.book.title, error: null };
					const title = await api
						.getBook(o.bookId)
						.then((b) => b.title)
						.catch(() => `Book #${o.bookId}`);
					return { bookId: o.bookId, title, error: o.error ?? "Unknown error" };
				})
			);
			outcomes = list;
			const ok = list.filter((o) => !o.error).length;
			if (ok === list.length) toast.success(`Sent ${ok} ${ok === 1 ? "book" : "books"} to ${device.name}`);
			else if (ok) toast.warning(`Sent ${ok} of ${list.length} books`);
			else toast.error("No books could be sent");
		} catch (e) {
			toast.error(`Sending failed: ${e}`);
		} finally {
			sending = false;
		}
		await refresh();
		if (selectedId === device.id) await loadBooks(device.id);
		onchanged?.();
	}

	async function confirmDelete() {
		const book = deleting;
		if (!book || !selected) return;
		deleting = null;
		try {
			await api.deleteDeviceBook(selected.id, book.path);
			books = books.filter((b) => b.path !== book.path);
			toast.success("Removed from device");
			refresh();
			onchanged?.();
		} catch (e) {
			toast.error(`Could not delete: ${e}`);
		}
	}

	function usedPct(d: Device) {
		return d.totalBytes > 0 ? Math.min(100, Math.max(0, ((d.totalBytes - d.freeBytes) / d.totalBytes) * 100)) : 0;
	}

	const cols = "grid-cols-[minmax(0,1fr)_7rem_3.25rem_3.75rem_1.75rem]";
	const failed = $derived(outcomes?.filter((o) => o.error) ?? []);
</script>

{#snippet sortHead(key: SortKey, text: string, align = "")}
	<button
		type="button"
		class={cn(
			"hover:text-foreground flex items-center gap-1 text-left text-xs font-medium transition-colors",
			sortKey === key ? "text-foreground" : "text-muted-foreground",
			align
		)}
		onclick={() => sortBy(key)}
	>
		{text}
		{#if sortKey === key}
			{#if sortDesc}<ArrowDownIcon class="size-3" />{:else}<ArrowUpIcon class="size-3" />{/if}
		{/if}
	</button>
{/snippet}

<Sheet.Root bind:open>
	<Sheet.Content
		side="right"
		class="gap-0 p-0 data-[side=right]:w-[560px] data-[side=right]:max-w-full data-[side=right]:sm:max-w-[560px]"
	>
		<header class="flex items-center gap-2 border-b py-3.5 pr-14 pl-6">
			<div class="min-w-0 flex-1">
				<Sheet.Title class="text-base font-semibold">Devices</Sheet.Title>
				<Sheet.Description class="text-muted-foreground mt-0.5 text-xs">
					Send books to an e-reader or a folder.
				</Sheet.Description>
			</div>
			<Button variant="ghost" size="icon-sm" aria-label="Refresh devices" onclick={() => refresh(true)}>
				<RefreshIcon class={cn((refreshing) && "animate-spin")} />
			</Button>
		</header>

		<div class="flex min-h-0 flex-1 flex-col">
			{#if !devicesLoaded}
				<div class="space-y-3 p-5">
					<Skeleton class="h-[5.5rem] w-full" />
					<Skeleton class="h-[5.5rem] w-full" />
				</div>
			{:else if devices.length === 0}
				<div class="flex flex-1 flex-col items-center justify-center gap-5 px-10 text-center">
					<div class="bg-muted text-muted-foreground grid size-16 place-items-center rounded-2xl">
						<UsbIcon class="size-8" />
					</div>
					<div class="space-y-2">
						<p class="text-base font-semibold">No device connected</p>
						<p class="text-muted-foreground text-sm leading-relaxed">
							Connect a Kobo, Kindle, PocketBook or Tolino by USB. It shows up here automatically.
						</p>
						<p class="text-muted-foreground text-xs leading-relaxed">
							Devices that only offer MTP (newer Kindles) are not supported yet. Copy to an SD card or a
							folder instead.
						</p>
					</div>
					{#if devicesError}
						<p class="text-destructive text-xs break-words">{devicesError}</p>
					{/if}
					<Button variant="outline" onclick={addFolder}>
						<FolderIcon />
						Add folder as device…
					</Button>
				</div>
			{:else}
				<div class="max-h-[44%] shrink-0 space-y-2 overflow-y-auto border-b p-4">
					{#each devices as d (d.id)}
						{@const Icon = d.kind === "folder" ? FolderIcon : d.kind === "generic" ? UsbIcon : TabletIcon}
						{@const low = d.totalBytes > 0 && d.freeBytes / d.totalBytes < 0.1}
						<button
							type="button"
							aria-pressed={selectedId === d.id}
							class={cn(
								"w-full rounded-xl border p-3.5 text-left transition-colors",
								selectedId === d.id
									? "border-brand bg-brand/10 ring-brand/30 ring-2"
									: "hover:bg-accent/50 border-border"
							)}
							onclick={() => selectDevice(d)}
						>
							<div class="flex items-start gap-3">
								<div
									class={cn(
										"grid size-10 shrink-0 place-items-center rounded-lg",
										selectedId === d.id ? "bg-brand text-brand-foreground" : "bg-muted text-muted-foreground"
									)}
								>
									<Icon class="size-5" />
								</div>
								<div class="min-w-0 flex-1">
									<div class="flex items-baseline gap-2">
										<span class="truncate text-sm font-semibold">{d.name}</span>
										<span class="text-muted-foreground shrink-0 text-xs">{kindLabel[d.kind]}</span>
									</div>
									<p class="text-muted-foreground mt-0.5 truncate font-mono text-[0.7rem]">{d.root}</p>
								</div>
							</div>
							{#if d.totalBytes > 0}
								<div class="mt-3">
									<div class="bg-muted h-1.5 overflow-hidden rounded-full">
										<div
											class={cn("h-full rounded-full", low ? "bg-destructive" : "bg-foreground/70")}
											style="width: {usedPct(d)}%"
										></div>
									</div>
									<p class={cn("mt-1 text-xs tabular-nums", low ? "text-destructive" : "text-muted-foreground")}>
										{humanSize(d.freeBytes)} free of {humanSize(d.totalBytes)}
									</p>
								</div>
							{/if}
							{#if d.supportedFormats.length}
								<div class="mt-2.5 flex flex-wrap gap-1">
									{#each d.supportedFormats as f (f)}
										<span class="bg-secondary text-secondary-foreground rounded px-1.5 py-px text-[0.65rem] font-semibold uppercase">
											{f}
										</span>
									{/each}
								</div>
							{/if}
						</button>
					{/each}
					<Button variant="ghost" size="sm" class="text-muted-foreground" onclick={addFolder}>
						<FolderIcon />
						Add folder as device…
					</Button>
				</div>

				{#if devicesError}
					<div class="bg-destructive/10 text-destructive flex items-center gap-2 px-5 py-2 text-xs">
						<TriangleAlertIcon class="size-3.5 shrink-0" />
						<span class="break-words">Could not refresh devices: {devicesError}</span>
					</div>
				{/if}

				{#if outcomes}
					<div class="shrink-0 border-b px-5 py-3">
						<div class="flex items-center gap-2 text-sm font-medium">
							{#if failed.length === 0}
								<CircleCheckIcon class="size-4 text-green-600" />
								Sent {outcomes.length} {outcomes.length === 1 ? "book" : "books"}
							{:else}
								<CircleAlertIcon class="text-destructive size-4" />
								Sent {outcomes.length - failed.length} of {outcomes.length}, {failed.length} failed
							{/if}
							<button
								type="button"
								class="text-muted-foreground hover:text-foreground ml-auto grid size-6 place-items-center rounded"
								aria-label="Dismiss"
								onclick={() => (outcomes = null)}
							>
								<XIcon class="size-3.5" />
							</button>
						</div>
						<ul class="mt-2 max-h-28 space-y-1 overflow-y-auto text-xs">
							{#each outcomes as o (o.bookId)}
								<li class="flex items-start gap-2">
									{#if o.error}
										<CircleAlertIcon class="text-destructive mt-px size-3.5 shrink-0" />
										<span class="min-w-0">
											<span class="font-medium">{o.title}</span>
											<span class="text-muted-foreground block break-words">{o.error}</span>
										</span>
									{:else}
										<CircleCheckIcon class="mt-px size-3.5 shrink-0 text-green-600" />
										<span class="truncate">{o.title}</span>
									{/if}
								</li>
							{/each}
						</ul>
					</div>
				{/if}

				<div class="flex min-h-0 flex-1 flex-col">
					{#if !selected}
						<div class="text-muted-foreground flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center text-sm">
							<BookOpenIcon class="size-6 opacity-60" />
							Select a device to see its books.
						</div>
					{:else}
						<div class="flex items-center gap-3 px-5 pt-3 pb-2">
							<h3 class="text-sm font-semibold">
								On device
								<span class="text-muted-foreground font-normal tabular-nums">{books.length}</span>
							</h3>
							<div class="relative ml-auto w-52">
								<SearchIcon class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2" />
								<input
									type="search"
									placeholder="Search device"
									aria-label="Search books on device"
									bind:value={search}
									class="dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-8 w-full rounded-md border bg-transparent pr-2 pl-8 text-sm shadow-xs outline-none placeholder:text-muted-foreground focus-visible:ring-3"
								/>
							</div>
						</div>
						<div class="grid {cols} items-center gap-3 border-b px-5 py-1.5">
							{@render sortHead("title", "Title")}
							{@render sortHead("authors", "Author")}
							{@render sortHead("format", "Type")}
							{@render sortHead("size", "Size", "justify-end")}
							<span></span>
						</div>
						<div class="min-h-0 flex-1 overflow-y-auto">
							{#if booksLoading && books.length === 0}
								<div class="space-y-3 p-5">
									{#each [0, 1, 2, 3, 4] as i (i)}
										<Skeleton class="h-4 w-full" />
									{/each}
								</div>
							{:else if booksError}
								<div class="flex flex-col items-center gap-3 p-8 text-center">
									<TriangleAlertIcon class="text-destructive size-6" />
									<p class="text-muted-foreground text-xs break-words">{booksError}</p>
									<Button variant="outline" size="sm" onclick={() => selectedId && loadBooks(selectedId)}>Try again</Button>
								</div>
							{:else if books.length === 0}
								<p class="text-muted-foreground p-8 text-center text-sm">No books on this device yet.</p>
							{:else if visibleBooks.length === 0}
								<p class="text-muted-foreground p-8 text-center text-sm">No books match “{search}”.</p>
							{:else}
								{#each visibleBooks as b (b.path)}
									<div
										class="group hover:bg-accent/50 grid {cols} items-center gap-3 px-5 py-1.5 text-sm [content-visibility:auto] [contain-intrinsic-size:auto_2rem]"
									>
										<span class="truncate" title={b.title}>{b.title}</span>
										<span class="text-muted-foreground truncate text-xs" title={b.authors.join(", ")}>
											{b.authors.join(", ") || "—"}
										</span>
										<span class="text-muted-foreground text-xs uppercase">{b.format}</span>
										<span class="text-muted-foreground text-right text-xs tabular-nums">{humanSize(b.size)}</span>
										<button
											type="button"
											aria-label="Delete {b.title} from device"
											class="text-muted-foreground hover:text-destructive grid size-7 place-items-center rounded-md opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
											onclick={() => (deleting = b)}
										>
											<Trash2Icon class="size-3.5" />
										</button>
									</div>
								{/each}
							{/if}
						</div>
					{/if}
				</div>
			{/if}
		</div>

		{#if devices.length > 0}
			<footer class="border-t px-5 py-3">
				{#if sending}
					<div class="bg-muted mb-2.5 h-1 overflow-hidden rounded-full">
						<div class="bg-brand h-full w-1/3 animate-pulse rounded-full"></div>
					</div>
				{/if}
				{#if selectedIds.length > 0}
					<Button class="w-full" size="lg" disabled={!selected || sending} onclick={send}>
						{#if sending}
							<LoaderIcon class="animate-spin" />
							Sending {selectedIds.length} {selectedIds.length === 1 ? "book" : "books"}…
						{:else}
							<SendIcon />
							Send {selectedIds.length} selected {selectedIds.length === 1 ? "book" : "books"}{selected ? ` to ${selected.name}` : ""}
						{/if}
					</Button>
					{#if !selected}
						<p class="text-muted-foreground mt-2 text-center text-xs">Select a device above to send to.</p>
					{/if}
				{:else}
					<p class="text-muted-foreground text-center text-xs">
						Select books in your library to send them to a device.
					</p>
				{/if}
			</footer>
		{/if}
	</Sheet.Content>
</Sheet.Root>

<AlertDialog.Root open={deleting !== null} onOpenChange={(v) => !v && (deleting = null)}>
	<AlertDialog.Content size="sm">
		<AlertDialog.Header>
			<AlertDialog.Title>Delete from device?</AlertDialog.Title>
			<AlertDialog.Description>
				“{deleting?.title}” is removed from {selected?.name ?? "the device"}. Your library copy is not affected.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action variant="destructive" onclick={confirmDelete}>Delete</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
