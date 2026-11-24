<script lang="ts" module>
	export type CurrentMetadata = {
		title: string;
		authors: string[];
		series: string;
		seriesIndex: number | null;
		publisher: string;
		pubdate: string;
		languages: string[];
		tags: string[];
		identifiers: Record<string, string>;
		comments: string;
		rating: number;
		coverSrc: string | null;
	};

	export type AppliedMetadata = Partial<{
		title: string;
		authors: string[];
		series: string;
		seriesIndex: number | null;
		publisher: string;
		pubdate: string;
		languages: string[];
		tags: string[];
		identifiers: Record<string, string>;
		comments: string;
		rating: number;
		coverUrl: string;
	}>;
</script>

<script lang="ts">
	import ArrowLeftIcon from "@lucide/svelte/icons/arrow-left";
	import SearchIcon from "@lucide/svelte/icons/search";
	import ImageIcon from "@lucide/svelte/icons/image";
	import CheckIcon from "@lucide/svelte/icons/check";
	import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
	import SearchXIcon from "@lucide/svelte/icons/search-x";
	import ArrowRightIcon from "@lucide/svelte/icons/arrow-right";
	import LoaderIcon from "@lucide/svelte/icons/loader-circle";
	import { onMount } from "svelte";
	import { api, type IdentifyQuery, type SourceResult } from "$lib/api";
	import { Button } from "$lib/components/ui/button/index.js";
		import { Skeleton } from "$lib/components/ui/skeleton/index.js";
	import { cn } from "$lib/utils.js";
	import Check from "./Check.svelte";
	import { languageName } from "./languages";
	import { htmlToText, sanitizeHtml } from "./sanitize";

	let {
		current,
		query: initial,
		onback,
		onapply
	}: {
		current: CurrentMetadata;
		query: { title: string; authors: string; isbn: string };
		onback: () => void;
		onapply: (applied: AppliedMetadata) => void;
	} = $props();

	let q = $state({ title: "", authors: "", isbn: "" });
	let results = $state<SourceResult[] | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let picked = $state<number | null>(null);
	let checks = $state<Record<string, boolean>>({});
	let tagsMode = $state<"merge" | "replace">("merge");
	let failedCovers = $state<Record<string, boolean>>({});
	let token = 0;

	const field =
		"dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-8 min-w-0 rounded-md border bg-transparent px-2.5 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:ring-3";

	const selected = $derived(picked !== null ? (results?.[picked] ?? null) : null);

	type Row = { key: string; label: string; cur: string; next: string; same: boolean; long?: boolean };

	function date(v: string | null | undefined) {
		return v ? v.slice(0, 10) : "";
	}

	function idString(ids: Record<string, string>) {
		return Object.entries(ids)
			.map(([k, v]) => `${k}: ${v}`)
			.join(", ");
	}

	function rowsFor(r: SourceResult): Row[] {
		const m = r.metadata;
		const list: Omit<Row, "same">[] = [];
		if (m.title) list.push({ key: "title", label: "Title", cur: current.title, next: m.title });
		if (m.authors.length)
			list.push({ key: "authors", label: "Authors", cur: current.authors.join(", "), next: m.authors.join(", ") });
		if (m.series)
			list.push({
				key: "series",
				label: "Series",
				cur: current.series ? `${current.series}${current.seriesIndex ? ` #${current.seriesIndex}` : ""}` : "",
				next: `${m.series}${m.seriesIndex ? ` #${m.seriesIndex}` : ""}`
			});
		if (m.publisher) list.push({ key: "publisher", label: "Publisher", cur: current.publisher, next: m.publisher });
		if (m.pubdate) list.push({ key: "pubdate", label: "Published", cur: date(current.pubdate), next: date(m.pubdate) });
		if (m.languages.length)
			list.push({
				key: "languages",
				label: "Languages",
				cur: current.languages.map(languageName).join(", "),
				next: m.languages.map(languageName).join(", ")
			});
		if (m.tags.length)
			list.push({ key: "tags", label: "Tags", cur: current.tags.join(", "), next: m.tags.join(", ") });
		if (Object.keys(m.identifiers).length)
			list.push({
				key: "identifiers",
				label: "Identifiers",
				cur: idString(current.identifiers),
				next: idString(m.identifiers)
			});
		if (m.rating) list.push({ key: "rating", label: "Rating", cur: current.rating ? `${current.rating / 2} / 5` : "", next: `${m.rating / 2} / 5` });
		if (m.comments)
			list.push({
				key: "comments",
				label: "Description",
				cur: htmlToText(current.comments),
				next: htmlToText(m.comments),
				long: true
			});
		return list.map((row) => ({ ...row, same: normalize(row.cur) === normalize(row.next) }));
	}

	function normalize(s: string) {
		return s.trim().toLowerCase();
	}

	const rows = $derived(selected ? rowsFor(selected) : []);
	const hasCover = $derived(!!selected?.coverUrl && !failedCovers[selected.coverUrl]);
	const tagsMergeable = $derived(!!selected && current.tags.length > 0);
	const count = $derived(
		rows.filter((r) => !r.same && checks[r.key]).length + (hasCover && checks.cover ? 1 : 0)
	);

	function pick(i: number) {
		picked = i;
		const r = results?.[i];
		if (!r) return;
		const next: Record<string, boolean> = {};
		for (const row of rowsFor(r)) next[row.key] = !row.same && (!row.cur || row.key === "identifiers" || row.key === "tags");
		next.cover = !!r.coverUrl && !current.coverSrc;
		checks = next;
		tagsMode = "merge";
	}

	async function search() {
		const title = q.title.trim();
		const isbn = q.isbn.trim();
		if (!title && !isbn) return;
		const mine = ++token;
		loading = true;
		error = null;
		picked = null;
		const query: IdentifyQuery = {
			title: title || null,
			authors: q.authors
				.split(/[,&;]/)
				.map((a) => a.trim())
				.filter(Boolean),
			isbn: isbn || null
		};
		try {
			const found = await api.identify(query);
			if (mine !== token) return;
			results = found;
			if (found.length) pick(0);
		} catch (e) {
			if (mine !== token) return;
			error = String(e);
			results = null;
		} finally {
			if (mine === token) loading = false;
		}
	}

	onMount(() => {
		q = { ...initial };
		search();
	});

	function apply() {
		if (!selected) return;
		const m = selected.metadata;
		const out: AppliedMetadata = {};
		for (const row of rows) {
			if (row.same || !checks[row.key]) continue;
			if (row.key === "title") out.title = m.title ?? "";
			else if (row.key === "authors") out.authors = m.authors;
			else if (row.key === "series") {
				out.series = m.series ?? "";
				out.seriesIndex = m.seriesIndex;
			} else if (row.key === "publisher") out.publisher = m.publisher ?? "";
			else if (row.key === "pubdate") out.pubdate = date(m.pubdate);
			else if (row.key === "languages") out.languages = m.languages;
			else if (row.key === "tags") {
				const base = tagsMode === "merge" ? current.tags : [];
				const seen = new Set(base.map((t) => t.toLowerCase()));
				out.tags = [...base, ...m.tags.filter((t) => !seen.has(t.toLowerCase()))];
			} else if (row.key === "identifiers") out.identifiers = { ...current.identifiers, ...m.identifiers };
			else if (row.key === "rating") out.rating = m.rating ?? 0;
			else if (row.key === "comments") out.comments = sanitizeHtml(m.comments);
		}
		if (hasCover && checks.cover && selected.coverUrl) out.coverUrl = selected.coverUrl;
		onapply(out);
	}

	function sourceColor(source: string) {
		const hues = [250, 160, 30, 330, 200, 90];
		let h = 0;
		for (const c of source) h = (h * 31 + c.charCodeAt(0)) % hues.length;
		return `color: oklch(0.5 0.14 ${hues[h]}); background: oklch(0.6 0.14 ${hues[h]} / 0.15)`;
	}
</script>

{#snippet thumb(url: string | null, cls: string)}
	{#if url && !failedCovers[url]}
		<img
			src={url}
			alt=""
			class="bg-muted object-cover {cls}"
			loading="lazy"
			onerror={() => (failedCovers[url] = true)}
		/>
	{:else}
		<div class="bg-muted text-muted-foreground/60 grid place-items-center {cls}">
			<ImageIcon class="size-4" />
		</div>
	{/if}
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex flex-wrap items-center gap-x-2 gap-y-2 border-b px-6 py-3">
		<Button variant="ghost" size="sm" class="-ml-2" onclick={onback}>
			<ArrowLeftIcon />
			Back
		</Button>
		<form
			class="flex min-w-0 flex-1 flex-wrap items-center gap-2"
			onsubmit={(e) => {
				e.preventDefault();
				search();
			}}
		>
			<input class="{field} min-w-40 flex-[2]" placeholder="Title" aria-label="Title" bind:value={q.title} />
			<input class="{field} min-w-32 flex-[1.5]" placeholder="Authors" aria-label="Authors" bind:value={q.authors} />
			<input class="{field} w-36" placeholder="ISBN" aria-label="ISBN" bind:value={q.isbn} />
			<Button type="submit" size="sm" disabled={loading || (!q.title.trim() && !q.isbn.trim())}>
				{#if loading}
					<LoaderIcon class="animate-spin" />
				{:else}
					<SearchIcon />
				{/if}
				Search
			</Button>
		</form>
	</div>

	<div class="grid min-h-0 flex-1 grid-cols-[19rem_1fr]">
		<div class="min-h-0 overflow-y-auto border-r p-2" aria-label="Results">
			{#if loading}
				{#each [0, 1, 2, 3] as i (i)}
					<div class="flex gap-3 p-2">
						<Skeleton class="h-[4.5rem] w-12 shrink-0" />
						<div class="flex-1 space-y-2 pt-1">
							<Skeleton class="h-3.5 w-4/5" />
							<Skeleton class="h-3 w-3/5" />
							<Skeleton class="h-3 w-2/5" />
						</div>
					</div>
				{/each}
			{:else if error}
				<div class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
					<TriangleAlertIcon class="text-destructive size-6" />
					<div>
						<p class="text-sm font-medium">Search failed</p>
						<p class="text-muted-foreground mt-1 text-xs break-words">{error}</p>
					</div>
					<Button variant="outline" size="sm" onclick={search}>Try again</Button>
				</div>
			{:else if results && results.length === 0}
				<div class="flex h-full flex-col items-center justify-center gap-2 p-6 text-center">
					<SearchXIcon class="text-muted-foreground size-6" />
					<p class="text-sm font-medium">No matches</p>
					<p class="text-muted-foreground text-xs">Try a shorter title, drop the author, or search by ISBN.</p>
				</div>
			{:else if results}
				<ul class="space-y-0.5">
					{#each results as r, i (i)}
						<li>
							<button
								type="button"
								class={cn(
									"hover:bg-accent/60 flex w-full gap-3 rounded-lg p-2 text-left transition-colors",
									picked === i && "bg-accent"
								)}
								aria-pressed={picked === i}
								onclick={() => pick(i)}
							>
								{@render thumb(r.coverUrl, "h-[4.5rem] w-12 shrink-0 rounded")}
								<div class="min-w-0 flex-1">
									<p class="line-clamp-2 text-sm leading-snug font-medium">{r.metadata.title ?? "Untitled"}</p>
									<p class="text-muted-foreground mt-0.5 truncate text-xs">{r.metadata.authors.join(", ") || "Unknown author"}</p>
									<p class="text-muted-foreground truncate text-xs">
										{[r.metadata.publisher, r.metadata.pubdate?.slice(0, 4)].filter(Boolean).join(" · ")}
									</p>
									<span
										class="mt-1 inline-block rounded px-1.5 py-px text-[0.65rem] font-semibold tracking-wide uppercase"
										style={sourceColor(r.source)}
									>
										{r.source}
									</span>
								</div>
							</button>
						</li>
					{/each}
				</ul>
			{:else}
				<p class="text-muted-foreground p-6 text-center text-xs">Enter a title or ISBN and search.</p>
			{/if}
		</div>

		<div class="flex min-h-0 flex-col">
			{#if selected}
				<div class="min-h-0 flex-1 overflow-y-auto p-5">
					<div class="mb-4 flex items-start gap-4">
						{@render thumb(selected.coverUrl, "h-28 w-[4.5rem] shrink-0 rounded-md shadow")}
						<div class="min-w-0 flex-1">
							<h3 class="text-base leading-snug font-semibold">{selected.metadata.title ?? "Untitled"}</h3>
							<p class="text-muted-foreground mt-0.5 text-sm">{selected.metadata.authors.join(", ")}</p>
							<p class="text-muted-foreground mt-1 text-xs">
								from <span class="text-foreground font-medium">{selected.source}</span>
								{#if selected.url}
									· <span class="break-all">{selected.url}</span>
								{/if}
							</p>
						</div>
					</div>

					<p class="text-muted-foreground mb-2 text-xs">Choose what to take over. Nothing is saved until you save the book.</p>

					<div class="divide-border border-border divide-y rounded-lg border text-sm">
						{#if selected.coverUrl}
							<label class="hover:bg-muted/40 flex cursor-pointer items-center gap-3 px-3 py-2.5">
								<Check bind:checked={checks.cover} disabled={!hasCover} />
								<span class="text-muted-foreground w-20 shrink-0 text-xs font-medium">Cover</span>
								<div class="flex min-w-0 flex-1 items-center gap-3">
									{@render thumb(current.coverSrc, "h-14 w-9 rounded")}
									<ArrowRightIcon class="text-muted-foreground size-3.5 shrink-0" />
									{@render thumb(selected.coverUrl, "h-14 w-9 rounded")}
									{#if !current.coverSrc}
										<span class="text-muted-foreground text-xs">No current cover</span>
									{/if}
								</div>
							</label>
						{/if}
						{#each rows as row (row.key)}
							<label
								class={cn(
									"flex items-start gap-3 px-3 py-2.5",
									row.same ? "opacity-60" : "hover:bg-muted/40 cursor-pointer"
								)}
							>
								<Check class="mt-0.5" bind:checked={checks[row.key]} disabled={row.same} />
								<span class="text-muted-foreground w-20 shrink-0 pt-px text-xs font-medium">{row.label}</span>
								<div class="min-w-0 flex-1">
									{#if row.same}
										<p class="text-muted-foreground text-xs">Same as current</p>
									{:else}
										<p class={cn("text-muted-foreground line-clamp-2 text-xs break-words", row.cur && "line-through decoration-muted-foreground/40")}>
											{row.cur || "empty"}
										</p>
										<p class={cn("font-medium break-words", row.long ? "line-clamp-4 text-xs font-normal leading-relaxed" : "line-clamp-3")}>
											{row.next}
										</p>
										{#if row.key === "tags" && tagsMergeable && checks.tags}
											<div class="bg-muted mt-1.5 inline-flex rounded-md p-0.5 text-xs" role="group" aria-label="Tags mode">
												{#each ["merge", "replace"] as mode (mode)}
													<button
														type="button"
														aria-pressed={tagsMode === mode}
														class={cn(
															"rounded px-2 py-0.5 capitalize transition-colors",
															tagsMode === mode ? "bg-background shadow-sm" : "text-muted-foreground"
														)}
														onclick={(e) => {
															e.preventDefault();
															tagsMode = mode as "merge" | "replace";
														}}
													>
														{mode}
													</button>
												{/each}
											</div>
										{/if}
									{/if}
								</div>
							</label>
						{/each}
					</div>
				</div>
				<div class="flex items-center justify-between gap-3 border-t px-5 py-3">
					<span class="text-muted-foreground text-xs">
						{count ? `${count} ${count === 1 ? "field" : "fields"} selected` : "Nothing selected"}
					</span>
					<Button size="sm" disabled={count === 0} onclick={apply}>
						<CheckIcon />
						Apply to form
					</Button>
				</div>
			{:else}
				<div class="text-muted-foreground flex flex-1 items-center justify-center p-6 text-center text-xs">
					{loading ? "Searching metadata sources…" : "Pick a result to compare it with your book."}
				</div>
			{/if}
		</div>
	</div>
</div>
