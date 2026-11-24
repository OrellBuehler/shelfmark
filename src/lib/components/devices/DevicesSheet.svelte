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
	import BookPlusIcon from "@lucide/svelte/icons/book-plus";
	import InfoIcon from "@lucide/svelte/icons/info";
	import ChevronDownIcon from "@lucide/svelte/icons/chevron-down";
	import CopyIcon from "@lucide/svelte/icons/copy";
	import { open as pickDirectory } from "@tauri-apps/plugin-dialog";
	import { untrack } from "svelte";
	import { toast } from "svelte-sonner";
	import { api, inTauri, type Device, type DeviceBook, type DeviceKind } from "$lib/api";
	import Check from "../edit/Check.svelte";
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
	type Outcome = { key: string; title: string; error: string | null; note: string | null };
	type Report = { kind: "send" | "import"; total: number; added: number; rows: Outcome[] };

	const copyProtected = ["AZW", "AZW3", "KFX", "MOBI"];

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
	let report = $state<Report | null>(null);
	let picked = $state<Set<string>>(new Set());
	let anchor: string | null = null;
	let importing = $state<string[]>([]);
	let libraryKeys = $state<Set<string>>(new Set());
	let detailsOpen = $state(false);
	let deleting = $state<DeviceBook | null>(null);
	let booksToken = 0;

	const selected = $derived(devices.find((d) => d.id === selectedId) ?? null);

	function norm(s: string) {
		return s
			.normalize("NFD")
			.replace(/\p{M}/gu, "")
			.toLowerCase()
			.replace(/[^\p{L}\p{N}]+/gu, " ")
			.trim();
	}

	function matchKey(title: string, authors: string[]) {
		const a = norm(authors[0] ?? "");
		return `${norm(title)}|${a === "unknown" || a === "unbekannt" ? "" : a}`;
	}

	const inLibrary = (b: DeviceBook) => libraryKeys.has(matchKey(b.title, b.authors));

	async function loadLibrary() {
		try {
			const list = await api.listBooks();
			libraryKeys = new Set(list.map((b) => matchKey(b.title, b.authors)));
		} catch {
			libraryKeys = new Set();
		}
	}

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
				picked = new Set();
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
		untrack(() => {
			refresh();
			loadLibrary();
		});
		const timer = setInterval(refresh, 4000);
		return () => clearInterval(timer);
	});

	async function loadBooks(id: string) {
		const mine = ++booksToken;
		booksLoading = true;
		booksError = null;
		try {
			const list = await api.deviceBooks(id);
			if (mine === booksToken) {
				books = list;
				const paths = new Set(list.map((b) => b.path));
				picked = new Set([...picked].filter((p) => paths.has(p)));
			}
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
		report = null;
		picked = new Set();
		anchor = null;
		detailsOpen = false;
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
		report = null;
		try {
			const res = await api.sendToDevice(device.id, ids);
			const list: Outcome[] = await Promise.all(
				res.map(async (o) => {
					const key = String(o.bookId);
					if (o.book)
						return {
							key,
							title: o.book.title,
							error: null,
							note: o.converted ? `Converted to ${o.book.format.toUpperCase()}` : null
						};
					const title = await api
						.getBook(o.bookId)
						.then((b) => b.title)
						.catch(() => `Book #${o.bookId}`);
					return { key, title, error: o.error ?? "Unknown error", note: null };
				})
			);
			const ok = list.filter((o) => !o.error).length;
			report = { kind: "send", total: list.length, added: ok, rows: list };
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
		loadLibrary();
		onchanged?.();
	}

	function setPicked(path: string, on: boolean) {
		const next = new Set(picked);
		if (on) next.add(path);
		else next.delete(path);
		picked = next;
	}

	function toggle(b: DeviceBook, on: boolean, range: boolean) {
		const from = range && anchor ? visibleBooks.findIndex((x) => x.path === anchor) : -1;
		const to = visibleBooks.findIndex((x) => x.path === b.path);
		if (from >= 0 && to >= 0) {
			const next = new Set(picked);
			for (const x of visibleBooks.slice(Math.min(from, to), Math.max(from, to) + 1)) {
				if (on) next.add(x.path);
				else next.delete(x.path);
			}
			picked = next;
		} else setPicked(b.path, on);
		anchor = b.path;
	}

	function toggleAll(on: boolean) {
		const next = new Set(picked);
		for (const b of visibleBooks) {
			if (on && !inLibrary(b)) next.add(b.path);
			else if (!on) next.delete(b.path);
		}
		picked = next;
		anchor = null;
	}

	async function importBooks(paths: string[]) {
		if (!selected || busy || paths.length === 0) return;
		const device = selected;
		const titles = new Map(books.map((b) => [b.path, b.title]));
		importing = paths;
		report = null;
		try {
			const res = await api.importFromDevice(device.id, paths);
			const rows: Outcome[] = res.map((o) => ({
				key: o.path,
				title: o.error ? (o.path.split(/[\\/]/).pop() ?? o.path) : (titles.get(o.path) ?? o.path),
				error: o.error,
				note: null
			}));
			const ok = rows.filter((r) => !r.error).length;
			report = { kind: "import", total: rows.length, added: ok, rows };
			const done = new Set(res.filter((o) => !o.error).map((o) => o.path));
			picked = new Set([...picked].filter((p) => !done.has(p)));
			if (ok === rows.length) toast.success(`Added ${ok} ${ok === 1 ? "book" : "books"} to the library`);
			else if (ok) toast.warning(`Added ${ok} of ${rows.length} books`);
			else toast.error("No books could be added");
		} catch (e) {
			toast.error(`Import failed: ${e}`);
		} finally {
			importing = [];
		}
		await loadLibrary();
		onchanged?.();
	}

	function copy(value: string) {
		navigator.clipboard
			.writeText(value)
			.then(() => toast.success("Copied"))
			.catch(() => toast.error("Could not copy"));
	}

	async function confirmDelete() {
		const book = deleting;
		if (!book || !selected) return;
		deleting = null;
		try {
			await api.deleteDeviceBook(selected.id, book.path);
			books = books.filter((b) => b.path !== book.path);
			setPicked(book.path, false);
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

	const cols = "grid-cols-[1rem_minmax(0,1fr)_6.5rem_3rem_3.5rem_3.75rem]";
	const failed = $derived(report?.rows.filter((o) => o.error) ?? []);
	const selectable = $derived(visibleBooks.filter((b) => !inLibrary(b)));
	const allPicked = $derived(selectable.length > 0 && selectable.every((b) => picked.has(b.path)));
	const pickedBooks = $derived(books.filter((b) => picked.has(b.path)));
	const showKindleHint = $derived(
		selected?.kind === "kindle" && pickedBooks.some((b) => copyProtected.includes(b.format.toUpperCase()))
	);
	const busy = $derived(sending || importing.length > 0);
</script>

{#snippet infoGrid(d: Device)}
	{@const collapsible = d.info.length > 4}
	<div class="border-brand/20 border-t px-3.5 py-2">
		{#if collapsible}
			<button
				type="button"
				aria-expanded={detailsOpen}
				class="text-muted-foreground hover:text-foreground flex w-full items-center gap-1 text-xs font-medium"
				onclick={() => (detailsOpen = !detailsOpen)}
			>
				Details
				<span class="font-normal tabular-nums">{d.info.length}</span>
				<ChevronDownIcon class={cn("ml-auto size-3.5 transition-transform", detailsOpen && "rotate-180")} />
			</button>
		{/if}
		{#if !collapsible || detailsOpen}
			<dl class={cn("grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-1 text-xs", collapsible && "mt-1.5")}>
				{#each d.info as item (item.label)}
					<div class="group/info col-span-2 grid grid-cols-subgrid items-center">
						<dt class="text-muted-foreground whitespace-nowrap">{item.label}</dt>
						<dd class="flex min-w-0 items-center gap-1">
							<span class="min-w-0 truncate select-all" title={item.value}>{item.value}</span>
							<button
								type="button"
								aria-label="Copy {item.label}"
								class="text-muted-foreground hover:text-foreground grid size-5 shrink-0 place-items-center rounded opacity-0 transition-opacity group-hover/info:opacity-100 focus-visible:opacity-100"
								onclick={() => copy(item.value)}
							>
								<CopyIcon class="size-3" />
							</button>
						</dd>
					</div>
				{/each}
			</dl>
		{/if}
	</div>
{/snippet}

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
		class="gap-0 p-0 data-[side=right]:w-[640px] data-[side=right]:max-w-full data-[side=right]:sm:max-w-[640px]"
	>
		<header class="flex items-center gap-2 border-b py-3.5 pr-14 pl-6">
			<div class="min-w-0 flex-1">
				<Sheet.Title class="text-base font-semibold">Devices</Sheet.Title>
				<Sheet.Description class="text-muted-foreground mt-0.5 text-xs">
					Send books to an e-reader or a folder, or copy them back into your library.
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
				<div class="max-h-[50%] shrink-0 space-y-2 overflow-y-auto border-b p-4">
					{#each devices as d (d.id)}
						{@const Icon = d.kind === "folder" ? FolderIcon : d.kind === "generic" ? UsbIcon : TabletIcon}
						{@const low = d.totalBytes > 0 && d.freeBytes / d.totalBytes < 0.1}
						<div
							class={cn(
								"rounded-xl border transition-colors",
								selectedId === d.id ? "border-brand bg-brand/10 ring-brand/30 ring-2" : "hover:bg-accent/50 border-border"
							)}
						>
							<button
								type="button"
								aria-pressed={selectedId === d.id}
								class="focus-visible:ring-ring/50 w-full rounded-xl p-3.5 text-left outline-none focus-visible:ring-3"
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
								<div class="mt-3">
									{#if d.totalBytes > 0}
										<div class="bg-muted h-1.5 overflow-hidden rounded-full">
											<div
												class={cn("h-full rounded-full", low ? "bg-destructive" : "bg-foreground/70")}
												style="width: {usedPct(d)}%"
											></div>
										</div>
									{/if}
									<div class="mt-1.5 flex items-center justify-between gap-3">
										<p class={cn("shrink-0 text-xs tabular-nums", low ? "text-destructive" : "text-muted-foreground")}>
											{#if d.totalBytes > 0}{humanSize(d.freeBytes)} free of {humanSize(d.totalBytes)}{/if}
										</p>
										<div class="flex min-w-0 flex-wrap justify-end gap-1">
											{#each d.supportedFormats as f (f)}
												<span class="bg-secondary text-secondary-foreground rounded px-1.5 py-px text-[0.65rem] font-semibold uppercase">
													{f}
												</span>
											{/each}
										</div>
									</div>
								</div>
							</button>
							{#if selectedId === d.id && d.info.length > 0}
								{@render infoGrid(d)}
							{/if}
						</div>
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

				{#if report}
					{@const rows = report.kind === "send" ? report.rows : failed}
					<div class="shrink-0 border-b px-5 py-3">
						<div class="flex items-center gap-2 text-sm font-medium">
							{#if failed.length === 0}
								<CircleCheckIcon class="size-4 text-green-600" />
								{report.kind === "send" ? "Sent" : "Added"}
								{report.total}
								{report.total === 1 ? "book" : "books"}{report.kind === "import" ? " to the library" : ""}
							{:else}
								<CircleAlertIcon class="text-destructive size-4" />
								{report.kind === "send" ? "Sent" : "Added"}
								{report.added} of {report.total}, {failed.length} failed
							{/if}
							<button
								type="button"
								class="text-muted-foreground hover:text-foreground ml-auto grid size-6 place-items-center rounded"
								aria-label="Dismiss"
								onclick={() => (report = null)}
							>
								<XIcon class="size-3.5" />
							</button>
						</div>
						{#if rows.length}
							<ul class="mt-2 max-h-28 space-y-1 overflow-y-auto text-xs">
								{#each rows as o (o.key)}
									<li class="flex items-start gap-2">
										{#if o.error}
											<CircleAlertIcon class="text-destructive mt-px size-3.5 shrink-0" />
											<span class="min-w-0">
												<span class="block truncate font-medium" title={o.title}>{o.title}</span>
												<span class="text-muted-foreground block break-words">{o.error}</span>
											</span>
										{:else}
											<CircleCheckIcon class="mt-px size-3.5 shrink-0 text-green-600" />
											<span class="min-w-0 truncate" title={o.title}>{o.title}</span>
											{#if o.note}
												<span class="bg-secondary text-secondary-foreground shrink-0 rounded px-1.5 py-px text-[0.65rem] font-medium">
													{o.note}
												</span>
											{/if}
										{/if}
									</li>
								{/each}
							</ul>
						{/if}
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
						<div class="grid {cols} items-center gap-2.5 border-b px-5 py-1.5">
							<span
								role="presentation"
								class="grid place-items-center"
								title="Select all not yet in the library"
								onclickcapture={(e) => {
									e.stopPropagation();
									if (selectable.length) toggleAll(!allPicked);
								}}
							>
								<Check checked={allPicked} disabled={selectable.length === 0} />
							</span>
							{@render sortHead("title", "Title")}
							{@render sortHead("authors", "Author")}
							{@render sortHead("format", "Type")}
							{@render sortHead("size", "Size", "justify-end")}
							<span></span>
						</div>
						{#if picked.size > 0}
							<div class="bg-muted/50 shrink-0 space-y-1.5 border-b px-5 py-2">
								<div class="flex items-center gap-3">
									<span class="text-xs font-medium tabular-nums">{picked.size} selected</span>
									<button
										type="button"
										class="text-muted-foreground hover:text-foreground text-xs underline-offset-2 hover:underline"
										onclick={() => (picked = new Set())}
									>
										Clear
									</button>
									<Button class="ml-auto" size="sm" disabled={busy} onclick={() => importBooks([...picked])}>
										{#if importing.length > 1}
											<LoaderIcon class="animate-spin" />
											Adding…
										{:else}
											<BookPlusIcon />
											Add {picked.size} to library
										{/if}
									</Button>
								</div>
								{#if showKindleHint}
									<p class="text-muted-foreground flex items-start gap-1.5 text-xs leading-snug">
										<InfoIcon class="mt-px size-3.5 shrink-0" />
										Books bought from the Kindle store are copy-protected and may not open after copying.
									</p>
								{/if}
							</div>
						{/if}
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
									{@const lib = inLibrary(b)}
									{@const on = picked.has(b.path)}
									{@const adding = importing.includes(b.path)}
									<div
										class={cn(
											"group hover:bg-accent/50 grid items-center gap-2.5 px-5 py-1.5 text-sm [content-visibility:auto] [contain-intrinsic-size:auto_2rem]",
											cols,
											on && "bg-accent/40"
										)}
									>
										<span
											role="presentation"
											class="grid place-items-center"
											onclickcapture={(e) => {
												e.stopPropagation();
												toggle(b, !on, e.shiftKey);
											}}
										>
											<Check checked={on} />
										</span>
										<span class="flex min-w-0 items-center gap-1.5">
											<span class={cn("truncate", lib && !on && "text-muted-foreground")} title={b.title}>{b.title}</span>
											{#if lib}
												<span class="bg-secondary text-muted-foreground shrink-0 rounded px-1.5 py-px text-[0.65rem] font-medium">
													In library
												</span>
											{/if}
										</span>
										<span class="text-muted-foreground truncate text-xs" title={b.authors.join(", ")}>
											{b.authors.join(", ") || "—"}
										</span>
										<span class="text-muted-foreground text-xs uppercase">{b.format}</span>
										<span class="text-muted-foreground text-right text-xs tabular-nums">{humanSize(b.size)}</span>
										<span class="flex justify-end">
											{#if adding}
												<span class="text-muted-foreground grid size-7 place-items-center">
													<LoaderIcon class="size-3.5 animate-spin" />
												</span>
											{:else}
												<button
													type="button"
													aria-label="Add {b.title} to library"
													title="Add to library"
													disabled={busy}
													class="text-muted-foreground hover:text-foreground grid size-7 place-items-center rounded-md opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 disabled:pointer-events-none"
													onclick={() => importBooks([b.path])}
												>
													<BookPlusIcon class="size-3.5" />
												</button>
											{/if}
											<button
												type="button"
												aria-label="Delete {b.title} from device"
												title="Delete from device"
												class="text-muted-foreground hover:text-destructive grid size-7 place-items-center rounded-md opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100"
												onclick={() => (deleting = b)}
											>
												<Trash2Icon class="size-3.5" />
											</button>
										</span>
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
					<Button class="w-full" size="lg" disabled={!selected || busy} onclick={send}>
						{#if sending}
							<LoaderIcon class="animate-spin" />
							Sending…
						{:else}
							<SendIcon />
							Send {selectedIds.length} selected {selectedIds.length === 1 ? "book" : "books"}{selected ? ` to ${selected.name}` : ""}
						{/if}
					</Button>
					{#if sending}
						<p class="text-muted-foreground mt-2 text-center text-xs">
							Sending {selectedIds.length} {selectedIds.length === 1 ? "book" : "books"}. Books may be converted first, which can take a few seconds each.
						</p>
					{:else if !selected}
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
