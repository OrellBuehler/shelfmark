<script lang="ts">
	import CircleCheck from "@lucide/svelte/icons/circle-check";
	import Copy from "@lucide/svelte/icons/copy";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import Trash2 from "@lucide/svelte/icons/trash-2";
	import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
	import { toast } from "svelte-sonner";
	import { api, type AudioCopy, type AudioIssue } from "#lib/api";
	import { Badge } from "#lib/components/ui/badge";
	import { Button } from "#lib/components/ui/button";
	import { baseName, errorMessage, formatDateTime, formatDuration, formatSize } from "#lib/format";
	import { audio } from "#lib/stores/audio.svelte";
	import { cn } from "#lib/utils";

	interface Item {
		key: string;
		rootId: number;
		kind: AudioIssue["kind"];
		paths: string[];
		message: string;
		title: string;
	}

	const items = $derived.by(() => {
		const map = new Map<string, Item>();
		for (const i of audio.status.issues) {
			const key = i.set ?? `${i.rootId}|${i.path}`;
			const found = map.get(key);
			if (found) {
				found.paths.push(i.path);
				continue;
			}
			const dir = i.path.includes("/") ? i.path.slice(0, i.path.lastIndexOf("/")) : "";
			map.set(key, {
				key,
				rootId: i.rootId,
				kind: i.kind,
				paths: [i.path],
				message: i.message,
				title: (i.kind === "duplicate" && dir ? baseName(dir) : baseName(i.path)).replace(/\s*\[[0-9A-Za-z]{10}\]/, "")
			});
		}
		return [...map.values()].sort((a, b) => a.kind.localeCompare(b.kind) || a.title.localeCompare(b.title));
	});

	let selectedKey = $state<string | null>(null);
	let copies = $state.raw<AudioCopy[] | null>(null);
	let loadError = $state<string | null>(null);
	let deleting = $state<string | null>(null);
	let working = $state(false);

	const selected = $derived(items.find((i) => i.key === selectedKey) ?? items[0] ?? null);
	const busy = $derived(working || !!audio.job);

	$effect(() => {
		const item = selected;
		copies = null;
		loadError = null;
		deleting = null;
		if (!item || item.kind !== "duplicate") return;
		let stale = false;
		api.audioIssueCopies(item.rootId, item.paths[0]).then(
			(c) => {
				if (!stale) copies = c;
			},
			(e) => {
				if (!stale) loadError = errorMessage(e);
			}
		);
		return () => {
			stale = true;
		};
	});

	const list = (v: string[]) => v.join(", ");

	const FIELDS: { label: string; get: (c: AudioCopy) => string }[] = [
		{ label: "Size", get: (c) => formatSize(c.size) },
		{ label: "Modified", get: (c) => formatDateTime(c.modified) },
		{ label: "Format", get: (c) => c.format },
		{ label: "Duration", get: (c) => (c.durationMs ? formatDuration(c.durationMs) : "") },
		{ label: "Bitrate", get: (c) => (c.bitrate ? `${Math.round(c.bitrate / 1000)} kbps` : "") },
		{ label: "Title", get: (c) => c.title ?? "" },
		{ label: "Album", get: (c) => c.album ?? "" },
		{ label: "Subtitle", get: (c) => c.subtitle ?? "" },
		{ label: "Authors", get: (c) => list(c.authors) },
		{ label: "Narrators", get: (c) => list(c.narrators) },
		{ label: "Series", get: (c) => (c.series ? `${c.series}${c.seriesIndex != null ? ` #${c.seriesIndex}` : ""}` : "") },
		{ label: "Publisher", get: (c) => c.publisher ?? "" },
		{ label: "Language", get: (c) => c.language ?? "" },
		{ label: "Released", get: (c) => c.releaseDate ?? "" },
		{ label: "Genres", get: (c) => list(c.genres) },
		{ label: "ASIN", get: (c) => c.asin ?? "" },
		{ label: "Embedded cover", get: (c) => (c.error ? "" : c.hasCover ? "Yes" : "No") }
	];

	const rows = $derived(
		(copies ?? []).length
			? FIELDS.map((f) => {
					const values = copies!.map(f.get);
					return { label: f.label, values, differs: new Set(values).size > 1 };
				}).filter((r) => r.values.some(Boolean))
			: []
	);
	const differences = $derived(rows.filter((r) => r.differs).length);

	async function act(what: string, run: () => Promise<void>, rescan: number | null, done: string) {
		working = true;
		try {
			await run();
			toast.success(done);
			if (rescan === null) await audio.refresh();
			else audio.scan([rescan]);
		} catch (e) {
			toast.error(`${what}: ${errorMessage(e)}`);
		}
		working = false;
	}

	function use(item: Item, c: AudioCopy) {
		act("Could not switch copies", () => api.audioIssuePick(item.rootId, c.path), item.rootId, "Shelfmark now uses this copy");
	}

	function remove(item: Item, c: AudioCopy) {
		deleting = null;
		act("Could not delete the file", () => api.audioIssueDelete(item.rootId, c.path), item.rootId, "File deleted");
	}

	function hide(item: Item) {
		const paths = copies?.map((c) => c.path) ?? item.paths;
		act("Could not hide the issue", () => api.audioIssueIgnore(item.rootId, paths), null, "Issue hidden");
	}

	async function reveal(rootId: number, path: string) {
		try {
			await api.audioIssueReveal(rootId, path);
		} catch (e) {
			toast.error(errorMessage(e));
		}
	}
</script>

{#if !items.length}
	<div class="flex h-full flex-col items-center justify-center gap-2 p-10 text-center">
		<CircleCheck class="size-8 text-muted-foreground" />
		<p class="text-sm font-medium">No issues</p>
		<p class="max-w-sm text-[13px] text-muted-foreground">
			Duplicate copies and files that could not be read show up here after a scan.
		</p>
	</div>
{:else}
	<div class="flex h-full min-h-0 flex-col md:flex-row">
		<ul class="thin-scroll max-h-40 shrink-0 space-y-1 overflow-y-auto border-b p-2 md:max-h-none md:w-64 md:border-r md:border-b-0">
			{#each items as item (item.key)}
				<li>
					<button
						type="button"
						class={cn(
							"flex w-full items-start gap-2 rounded-md px-2.5 py-2 text-left outline-none hover:bg-accent/60 focus-visible:ring-2 focus-visible:ring-ring",
							selected?.key === item.key && "bg-accent"
						)}
						aria-current={selected?.key === item.key}
						onclick={() => (selectedKey = item.key)}
					>
						{#if item.kind === "duplicate"}
							<Copy class="mt-0.5 size-3.5 shrink-0 text-muted-foreground" />
						{:else}
							<TriangleAlert class="mt-0.5 size-3.5 shrink-0 text-destructive" />
						{/if}
						<span class="min-w-0">
							<span class="block truncate text-[13px] font-medium">{item.title}</span>
							<span class="block truncate text-xs text-muted-foreground">
								{item.kind === "duplicate" ? `${item.paths.length + 1} copies` : item.message}
							</span>
						</span>
					</button>
				</li>
			{/each}
		</ul>

		{#if selected}
			{@const item = selected}
			<div class="thin-scroll min-h-0 min-w-0 flex-1 space-y-4 overflow-y-auto p-5">
				{#if item.kind === "error"}
					<div>
						<h3 class="text-sm font-semibold">This file could not be read</h3>
						<p class="mt-1 font-mono text-[11px] break-all text-muted-foreground">{item.paths[0]}</p>
					</div>
					<p class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px]">{item.message}</p>
					<p class="text-[13px] text-muted-foreground">
						The file may be damaged or incomplete. Replace it and scan again, or hide this issue if the file is fine as it is.
					</p>
					<div class="flex flex-wrap gap-2">
						<Button size="sm" variant="outline" onclick={() => reveal(item.rootId, item.paths[0])}><FolderOpen />Show in folder</Button>
						<Button size="sm" variant="ghost" disabled={busy} onclick={() => hide(item)}>Hide this issue</Button>
					</div>
				{:else}
					<div>
						<h3 class="text-sm font-semibold">{item.title}</h3>
						<p class="mt-1 text-[13px] text-muted-foreground">
							This folder holds more than one copy of the same audiobook. Shelfmark uses one and ignores the rest. Choose which copy to use, delete the one you don't need, or keep them all.
						</p>
					</div>

					{#if loadError}
						<p class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px]">
							Could not read the copies: {loadError}
						</p>
					{:else if !copies}
						<p class="flex items-center gap-2 text-[13px] text-muted-foreground">
							<LoaderCircle class="size-3.5 animate-spin" />Reading both files
						</p>
					{:else}
						<p class="text-xs text-muted-foreground">
							{differences ? `${differences} of ${rows.length} details differ, highlighted below.` : "All details are identical."}
						</p>
						<div class="overflow-x-auto rounded-lg border">
							<table class="w-full border-collapse text-[13px]">
								<thead>
									<tr class="border-b bg-muted/40 align-top">
										<th class="w-32 px-3 py-2.5"></th>
										{#each copies as c (c.path)}
											<th class="min-w-56 px-3 py-2.5 text-left font-normal">
												<div class="flex items-center gap-2">
													<Badge variant={c.inLibrary ? "default" : "secondary"}>{c.inLibrary ? "In library" : "Ignored"}</Badge>
												</div>
												<div class="mt-1.5 font-mono text-[11px] break-all" title={c.fullPath}>{baseName(c.path)}</div>
												{#if c.error}
													<div class="mt-1 text-xs text-destructive">{c.error}</div>
												{/if}
												<div class="mt-2 flex flex-wrap gap-1.5">
													{#if !c.inLibrary}
														<Button size="xs" disabled={busy} onclick={() => use(item, c)}>Use this copy</Button>
													{/if}
													<Button size="xs" variant="outline" onclick={() => reveal(item.rootId, c.path)}><FolderOpen />Show</Button>
													<Button
														size="xs"
														variant="outline"
														disabled={busy || copies.length < 2}
														onclick={() => (deleting = deleting === c.path ? null : c.path)}
													>
														<Trash2 />Delete
													</Button>
												</div>
												{#if deleting === c.path}
													<div class="mt-2 space-y-2 rounded-md bg-destructive/10 p-2.5 text-xs">
														<p>
															Delete this file from disk? This cannot be undone.{c.inLibrary ? " Shelfmark switches to the other copy." : ""}
														</p>
														<div class="flex gap-1.5">
															<Button size="xs" variant="ghost" onclick={() => (deleting = null)}>Cancel</Button>
															<Button size="xs" variant="destructive" onclick={() => remove(item, c)}>Delete file</Button>
														</div>
													</div>
												{/if}
											</th>
										{/each}
									</tr>
								</thead>
								<tbody>
									{#each rows as row (row.label)}
										<tr class={cn("border-b align-top last:border-b-0", row.differs && "bg-amber-500/10")}>
											<th class="px-3 py-1.5 text-left text-xs font-medium text-muted-foreground">{row.label}</th>
											{#each row.values as v, i (i)}
												<td class="px-3 py-1.5 break-words">{v || "–"}</td>
											{/each}
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}

					<div class="flex flex-wrap items-center gap-2">
						<Button size="sm" variant="outline" disabled={busy} onclick={() => hide(item)}>Keep all copies and hide this</Button>
						{#if audio.job}
							<span class="flex items-center gap-1.5 text-xs text-muted-foreground">
								<LoaderCircle class="size-3 animate-spin" />Scanning to apply the change
							</span>
						{/if}
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/if}
