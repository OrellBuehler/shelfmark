<script lang="ts">
	import BookOpen from "@lucide/svelte/icons/book-open";
	import ChevronRight from "@lucide/svelte/icons/chevron-right";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import Link from "@lucide/svelte/icons/link";
	import Play from "@lucide/svelte/icons/play";
	import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
	import Unlink from "@lucide/svelte/icons/unlink";
	import { api, type AudiobookInfo, type BookSummary } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import { baseName, formatClock, formatDate, formatDateTime, formatDuration, formatSize, languageName } from "#lib/format";
	import { openLink } from "#lib/links";
	import { sanitizeHtml } from "#lib/sanitize";
	import { lib } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";
	import Cover from "./Cover.svelte";
	import LinkEbookDialog from "./LinkEbookDialog.svelte";

	const LONG = 8;

	let { entry }: { entry: BookSummary } = $props();

	let info = $state<AudiobookInfo | null>(null);
	let version = $state(0);
	let pickerOpen = $state(false);
	let chaptersOpen = $state<boolean | null>(null);
	let filesOpen = $state<boolean | null>(null);
	let token = 0;

	const id = $derived(entry.audiobookId);
	const key = $derived(`${id}:${entry.kind}:${entry.pairedBookId}:${entry.lastModified}:${version}`);

	$effect(() => {
		void key;
		const audiobookId = id;
		if (audiobookId === null) {
			info = null;
			return;
		}
		const mine = ++token;
		const timer = setTimeout(() => {
			api.audiobook(audiobookId).then(
				(r) => {
					if (mine === token) info = r;
				},
				(e) => lib.fail("Could not load audiobook details", e)
			);
		}, 60);
		return () => clearTimeout(timer);
	});

	const detail = $derived(info && info.id === id ? info : null);
	const brief = $derived(entry.audio);
	const audioState = $derived(detail?.state ?? brief?.state ?? "present");
	const narrators = $derived(detail?.narrators ?? brief?.narrators ?? []);
	const html = $derived(detail?.description ? sanitizeHtml(detail.description) : "");
	const chapters = $derived(detail?.chapters ?? []);
	const files = $derived(detail?.files ?? []);
	const chaptersShown = $derived(chaptersOpen ?? chapters.length <= LONG);
	const filesShown = $derived(filesOpen ?? files.length <= LONG);
	const pickerQuery = $derived(entry.title.split(/[:(\[]/)[0].trim());

	function onLinkClick(e: MouseEvent) {
		const a = (e.target as HTMLElement).closest("a");
		if (!a) return;
		e.preventDefault();
		openLink(a.href);
	}

	async function act(run: () => Promise<boolean>) {
		if (await run()) version++;
	}

	function percent(score: number) {
		return `${Math.round(score * 100)}%`;
	}
</script>

{#snippet matchRow(book: BookSummary, score?: number)}
	<div class="flex items-center gap-3">
		<div class="w-10 shrink-0"><Cover {book} width={120} /></div>
		<div class="min-w-0 flex-1">
			<div class="truncate text-[13px] font-medium" title={book.title}>{book.title}</div>
			<div class="truncate text-xs text-muted-foreground">{book.authors.join(", ") || "Unknown"}</div>
			{#if score !== undefined}<div class="mt-0.5 text-[11px] text-muted-foreground tabular-nums">{percent(score)} match</div>{/if}
		</div>
		{#if score !== undefined && id !== null}
			<Button size="xs" variant="outline" onclick={() => act(() => lib.linkAudio(id, book.id))}><Link />Link</Button>
		{/if}
	</div>
{/snippet}

<div class="flex flex-col gap-5 p-4 select-text">
	<div class="flex justify-center rounded-xl bg-muted/50 px-6 py-5">
		<div class="w-full max-w-[200px] shadow-lg shadow-black/20">
			<Cover book={entry} width={600} eager />
		</div>
	</div>

	<div>
		<h2 class="text-lg leading-snug font-semibold text-balance">{entry.title}</h2>
		{#if detail?.subtitle}<p class="mt-0.5 text-[13px] text-muted-foreground">{detail.subtitle}</p>{/if}
		<p class="mt-1 text-[13px] text-muted-foreground">
			{#each entry.authors as a, i (a)}
				{#if i > 0}<span class="mr-1">,</span>{/if}<button
					type="button"
					class="rounded-sm underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
					onclick={() => lib.addFilter(`authors:"=${a}"`)}>{a}</button
				>
			{:else}
				Unknown author
			{/each}
		</p>
		{#if narrators.length}
			<p class="mt-1 text-[13px] text-muted-foreground">
				Narrated by
				{#each narrators as n, i (n)}
					{#if i > 0}<span class="mr-1">,</span>{/if}<button
						type="button"
						class="rounded-sm underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
						onclick={() => lib.addFilter(`narrators:"=${n}"`)}>{n}</button
					>
				{/each}
			</p>
		{/if}
		{#if entry.series}
			<button
				type="button"
				class="mt-1 rounded-sm text-[13px] text-muted-foreground underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
				onclick={() => lib.addFilter(`series:"=${entry.series}"`)}
			>
				{entry.series} <span class="tabular-nums">#{entry.seriesIndex}</span>
			</button>
		{/if}
	</div>

	<div class="flex gap-2">
		<Button class="flex-1" disabled={audioState !== "present"} onclick={() => lib.playAudio(entry)}><Play />Play</Button>
		<Button
			variant="outline"
			size="icon"
			aria-label="Show in folder"
			title="Show in folder"
			disabled={audioState !== "present"}
			onclick={() => lib.showAudioInFolder(entry)}
		>
			<FolderOpen />
		</Button>
	</div>

	{#if audioState === "missing"}
		<p class="rounded-md border border-dashed px-3 py-2 text-xs text-muted-foreground">
			The audio files were not found at the last scan. Connect the drive or folder and scan again.
		</p>
	{:else if audioState === "remote"}
		<p class="rounded-md border border-dashed px-3 py-2 text-xs text-muted-foreground">
			Not downloaded yet. Only the catalog entry is known.
		</p>
	{/if}

	{#if entry.tags.length}
		<div class="flex flex-wrap gap-1.5">
			{#each entry.tags as t (t)}
				<button
					type="button"
					class="rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
					onclick={() => lib.addFilter(`tags:"=${t}"`)}>{t}</button
				>
			{/each}
		</div>
	{/if}

	{#if detail}
		{#if html}
			<div class="comments text-[13px] leading-relaxed text-foreground/90" onclick={onLinkClick} role="presentation">
				{@html html}
			</div>
		{/if}
	{:else}
		<div class="space-y-2">
			<div class="h-3 w-full animate-pulse rounded bg-muted"></div>
			<div class="h-3 w-11/12 animate-pulse rounded bg-muted"></div>
			<div class="h-3 w-2/3 animate-pulse rounded bg-muted"></div>
		</div>
	{/if}

	<dl class="grid grid-cols-[88px_1fr] gap-x-3 gap-y-2 text-[13px]">
		{#if brief}
			<dt class="text-muted-foreground">Length</dt>
			<dd class="tabular-nums">{formatDuration(brief.durationMs)}</dd>
			<dt class="text-muted-foreground">Format</dt>
			<dd class="tabular-nums"><span class="uppercase">{brief.format}</span> · {formatSize(brief.size)}</dd>
			<dt class="text-muted-foreground">Files</dt>
			<dd class="tabular-nums">{brief.fileCount.toLocaleString()}</dd>
		{/if}
		{#if entry.publisher}
			<dt class="text-muted-foreground">Publisher</dt>
			<dd class="min-w-0">
				<button
					type="button"
					class="max-w-full truncate rounded-sm text-left underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring"
					onclick={() => lib.addFilter(`publisher:"=${entry.publisher}"`)}>{entry.publisher}</button
				>
			</dd>
		{/if}
		{#if formatDate(entry.pubdate)}
			<dt class="text-muted-foreground">Released</dt>
			<dd>{formatDate(entry.pubdate)}</dd>
		{/if}
		{#if entry.languages.length}
			<dt class="text-muted-foreground">Language</dt>
			<dd>{entry.languages.map(languageName).join(", ")}</dd>
		{/if}
		<dt class="text-muted-foreground">Added</dt>
		<dd>{formatDateTime(entry.timestamp)}</dd>
		{#if detail?.asin}
			<dt class="text-muted-foreground">ASIN</dt>
			<dd class="min-w-0 truncate font-mono text-xs">{detail.asin}</dd>
		{/if}
		{#if detail?.isbn}
			<dt class="text-muted-foreground">ISBN</dt>
			<dd class="min-w-0 truncate font-mono text-xs">{detail.isbn}</dd>
		{/if}
	</dl>

	<section>
		<h3 class="mb-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Ebook</h3>
		{#if !detail}
			<div class="h-14 animate-pulse rounded-md bg-muted"></div>
		{:else if detail.link}
			{@const link = detail.link}
			<div class="space-y-2.5 rounded-md border bg-card p-2.5">
				{@render matchRow(link.book)}
				<p class="text-[11px] text-muted-foreground">{link.manual ? "Linked manually" : "Matched automatically"}</p>
				<div class="flex flex-wrap gap-1.5">
					<Button size="sm" variant="outline" onclick={() => lib.readBook(link.book)}><BookOpen />Open ebook</Button>
					<Button size="sm" variant="ghost" onclick={() => id !== null && act(() => lib.unlinkAudio(id, link.book.id))}>
						<Unlink />Unlink
					</Button>
					{#if link.manual}
						<Button size="sm" variant="ghost" onclick={() => id !== null && act(() => lib.resetAudioLink(id))}>
							<RotateCcw />Match automatically
						</Button>
					{/if}
				</div>
			</div>
		{:else}
			<div class="space-y-3">
				{#if detail.candidates.length}
					<div>
						<p class="mb-1.5 text-xs text-muted-foreground">Possible matches</p>
						<ul class="space-y-1.5">
							{#each detail.candidates as c (c.book.id)}
								<li class="rounded-md border bg-card p-2">{@render matchRow(c.book, c.score)}</li>
							{/each}
						</ul>
					</div>
				{:else}
					<p class="text-[13px] text-muted-foreground">Not linked to an ebook.</p>
				{/if}
				<div class="flex flex-wrap gap-1.5">
					<Button size="sm" variant="outline" onclick={() => (pickerOpen = true)}><Link />Link to ebook…</Button>
					<Button
						size="sm"
						variant="ghost"
						title="Forget manual links and rejected matches for this audiobook"
						onclick={() => id !== null && act(() => lib.resetAudioLink(id))}
					>
						<RotateCcw />Reset matching
					</Button>
				</div>
			</div>
		{/if}
	</section>

	{#if chapters.length}
		<section>
			<button
				type="button"
				class="mb-1.5 flex w-full items-center gap-1 rounded-sm text-xs font-medium tracking-wide text-muted-foreground uppercase outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
				aria-expanded={chaptersShown}
				onclick={() => (chaptersOpen = !chaptersShown)}
			>
				<ChevronRight class={cn("size-3.5 transition-transform", chaptersShown && "rotate-90")} />
				Chapters
				<span class="font-normal normal-case tabular-nums opacity-70">{chapters.length}</span>
			</button>
			{#if chaptersShown}
				<ol class="space-y-0.5 text-[13px]">
					{#each chapters as c, i (i)}
						<li class="flex items-baseline gap-2">
							<span class="min-w-0 flex-1 truncate" title={c.title}>{c.title}</span>
							<span class="text-xs text-muted-foreground tabular-nums">{formatClock(c.startMs)}</span>
						</li>
					{/each}
				</ol>
			{/if}
		</section>
	{/if}

	{#if detail && detail.fileCount > 1 && files.length}
		<section>
			<button
				type="button"
				class="mb-1.5 flex w-full items-center gap-1 rounded-sm text-xs font-medium tracking-wide text-muted-foreground uppercase outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
				aria-expanded={filesShown}
				onclick={() => (filesOpen = !filesShown)}
			>
				<ChevronRight class={cn("size-3.5 transition-transform", filesShown && "rotate-90")} />
				Files
				<span class="font-normal normal-case tabular-nums opacity-70">{files.length}</span>
			</button>
			{#if filesShown}
				<ul class="space-y-0.5 text-[13px]">
					{#each files as f (f.relPath)}
						<li class="flex items-baseline gap-2">
							<span class="min-w-0 flex-1 truncate" title={f.relPath}>{baseName(f.relPath)}</span>
							{#if f.kind !== "audio"}
								<span class="rounded bg-secondary px-1 py-px text-[10px] font-semibold text-secondary-foreground uppercase">{f.kind}</span>
							{:else if f.durationMs}
								<span class="text-xs text-muted-foreground tabular-nums">{formatDuration(f.durationMs)}</span>
							{/if}
							<span class="w-14 text-right text-xs text-muted-foreground tabular-nums">{formatSize(f.size)}</span>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
	{/if}

	{#if detail?.location}
		<div class="flex items-start gap-2 text-xs text-muted-foreground">
			<button
				type="button"
				class="mt-px shrink-0 rounded-sm outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
				aria-label="Show in folder"
				title="Show in folder"
				onclick={() => lib.showAudioInFolder(entry)}
			>
				<FolderOpen class="size-3.5" />
			</button>
			<span class="min-w-0 font-mono break-all">{detail.location}</span>
		</div>
	{/if}
</div>

{#if id !== null}
	<LinkEbookDialog bind:open={pickerOpen} audiobookId={id} query={pickerQuery} onlinked={() => version++} />
{/if}
