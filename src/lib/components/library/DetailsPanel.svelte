<script lang="ts">
	import BookOpen from "@lucide/svelte/icons/book-open";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import Info from "@lucide/svelte/icons/info";
	import Pencil from "@lucide/svelte/icons/pencil";
	import Send from "@lucide/svelte/icons/send";
	import Trash from "@lucide/svelte/icons/trash";
	import { api, isAudioEntry, type Book } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import { identifierLinks, formatDate, formatDateTime, formatSize, languageName, plural } from "#lib/format";
	import { openLink } from "#lib/links";
	import { sanitizeHtml } from "#lib/sanitize";
	import { lib } from "#lib/stores/library.svelte";
	import AudiobookDetails from "./AudiobookDetails.svelte";
	import AudiobookSection from "./AudiobookSection.svelte";
	import Cover from "./Cover.svelte";
	import Stars from "./Stars.svelte";

	const summary = $derived(lib.selectedBooks.length === 1 ? lib.selectedBooks[0] : null);
	const key = $derived(summary ? `${summary.id}:${summary.lastModified}` : null);

	let detail = $state<Book | null>(null);
	let token = 0;

	$effect(() => {
		const k = key;
		const id = summary?.id;
		if (k === null || id === undefined || (summary && isAudioEntry(summary))) {
			detail = null;
			return;
		}
		const mine = ++token;
		const timer = setTimeout(() => {
			api.getBook(id).then(
				(b) => {
					if (mine === token) detail = b;
				},
				(e) => lib.fail("Could not load book details", e)
			);
		}, 60);
		return () => clearTimeout(timer);
	});

	const book = $derived(summary && detail && detail.id === summary.id ? detail : null);
	const html = $derived(book?.comments ? sanitizeHtml(book.comments) : "");
	const ids = $derived(book ? identifierLinks(book.identifiers) : []);
	const fileMap = $derived(new Map((book?.files ?? []).map((f) => [f.format.toUpperCase(), f])));
	const stack = $derived(lib.selectedBooks.slice(0, 4));
	const noun = $derived(lib.selectedEbooks.length === lib.selected.size ? "book" : lib.selectedEbooks.length ? "item" : "audiobook");

	function onLinkClick(e: MouseEvent) {
		const a = (e.target as HTMLElement).closest("a");
		if (!a) return;
		e.preventDefault();
		openLink(a.href);
	}
</script>

<aside class="thin-scroll flex h-full flex-col overflow-y-auto bg-background text-sm" aria-label="Book details">
	{#if summary && isAudioEntry(summary)}
		{#key summary.id}
			<AudiobookDetails entry={summary} />
		{/key}
	{:else if summary}
		<div class="flex flex-col gap-5 p-4 select-text">
			<div class="flex justify-center rounded-xl bg-muted/50 px-6 py-5">
				<div class="w-full max-w-[200px] shadow-lg shadow-black/20">
					<Cover book={summary} width={600} eager />
				</div>
			</div>

			<div>
				<h2 class="text-lg leading-snug font-semibold text-balance">{summary.title}</h2>
				<p class="mt-1 text-[13px] text-muted-foreground">
					{#each summary.authors as a, i (a)}
						{#if i > 0}<span class="mr-1">,</span>{/if}<button
							type="button"
							class="rounded-sm underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
							onclick={() => lib.addFilter(`authors:"=${a}"`)}>{a}</button
						>
					{:else}
						Unknown author
					{/each}
				</p>
				{#if summary.series}
					<button
						type="button"
						class="mt-1 rounded-sm text-[13px] text-muted-foreground underline-offset-2 outline-none hover:text-foreground hover:underline focus-visible:ring-2 focus-visible:ring-ring"
						onclick={() => lib.addFilter(`series:"=${summary.series}"`)}
					>
						{summary.series} <span class="tabular-nums">#{summary.seriesIndex}</span>
					</button>
				{/if}
				<div class="mt-2.5">
					<Stars value={summary.rating} size={16} onchange={(r) => lib.setRating(summary.id, r)} class="-ml-0.5" />
				</div>
			</div>

			<div class="flex gap-2">
				<Button class="flex-1" onclick={() => lib.readBook(summary)}><BookOpen />Read</Button>
				<Button variant="outline" onclick={() => lib.editSelected()}><Pencil />Edit</Button>
				<Button variant="outline" size="icon" aria-label="Send to device" title="Send to device" onclick={() => lib.sendSelected()}>
					<Send />
				</Button>
			</div>

			{#if summary.tags.length}
				<div class="flex flex-wrap gap-1.5">
					{#each summary.tags as t (t)}
						<button
							type="button"
							class="rounded-full bg-secondary px-2.5 py-0.5 text-xs font-medium text-secondary-foreground outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-ring"
							onclick={() => lib.addFilter(`tags:"=${t}"`)}>{t}</button
						>
					{/each}
				</div>
			{/if}

			{#if book}
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
				{#if summary.publisher}
					<dt class="text-muted-foreground">Publisher</dt>
					<dd class="min-w-0">
						<button
							type="button"
							class="max-w-full truncate rounded-sm text-left underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring"
							onclick={() => lib.addFilter(`publisher:"=${summary.publisher}"`)}>{summary.publisher}</button
						>
					</dd>
				{/if}
				{#if formatDate(summary.pubdate)}
					<dt class="text-muted-foreground">Published</dt>
					<dd>{formatDate(summary.pubdate)}</dd>
				{/if}
				{#if summary.languages.length}
					<dt class="text-muted-foreground">Language</dt>
					<dd>{summary.languages.map(languageName).join(", ")}</dd>
				{/if}
				<dt class="text-muted-foreground">Added</dt>
				<dd>{formatDateTime(summary.timestamp)}</dd>
				<dt class="text-muted-foreground">Modified</dt>
				<dd>{formatDateTime(summary.lastModified)}</dd>
				<dt class="text-muted-foreground">Size</dt>
				<dd class="tabular-nums">{formatSize(summary.size)}</dd>
			</dl>

			{#if ids.length}
				<section>
					<h3 class="mb-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Identifiers</h3>
					<ul class="space-y-2 text-[13px]">
						{#each ids as id (id.label + id.value)}
							<li>
								<div class="flex items-baseline gap-2">
									<span class="w-24 shrink-0 text-muted-foreground">{id.label}</span>
									<span class="min-w-0 flex-1 truncate font-mono text-xs" title={id.value}>{id.value}</span>
								</div>
								{#if id.urls.length}
									<div class="mt-0.5 flex flex-wrap gap-x-3 pl-26">
										{#each id.urls as u (u.url)}
											<button
												type="button"
												class="inline-flex items-center gap-1 rounded-sm text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
												onclick={() => openLink(u.url)}
											>
												{u.name}<ExternalLink class="size-3" />
											</button>
										{/each}
									</div>
								{/if}
							</li>
						{/each}
					</ul>
				</section>
			{/if}

			<section>
				<h3 class="mb-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Formats</h3>
				<ul class="space-y-1">
					{#each summary.formats as f (f)}
						<li class="flex items-center gap-2 rounded-md border bg-card px-2.5 py-1.5">
							<span class="rounded bg-secondary px-1.5 py-0.5 text-[10px] font-semibold uppercase">{f}</span>
							<span class="flex-1 text-xs text-muted-foreground tabular-nums">
								{fileMap.get(f.toUpperCase()) ? formatSize(fileMap.get(f.toUpperCase())!.size) : ""}
							</span>
							<Button variant="ghost" size="xs" onclick={() => lib.readBook(summary, f)}>Read</Button>
						</li>
					{/each}
				</ul>
			</section>

			{#if summary.audio}
				<AudiobookSection {summary} />
			{/if}

			{#if book}
				<div class="flex items-start gap-2 text-xs text-muted-foreground">
					<button
						type="button"
						class="mt-px shrink-0 rounded-sm outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
						aria-label="Show in folder"
						title="Show in folder"
						onclick={() => lib.showInFolder(summary)}
					>
						<FolderOpen class="size-3.5" />
					</button>
					<span class="min-w-0 font-mono break-all">{book.path}</span>
				</div>
			{/if}
		</div>
	{:else if lib.selected.size > 1}
		<div class="flex flex-col gap-5 p-4">
			<div class="relative mx-auto mt-4 mb-2 h-56 w-full max-w-[220px]">
				{#each stack as b, i (b.id)}
					{@const n = stack.length}
					<div
						class="absolute top-0 left-1/2 w-32 -translate-x-1/2 shadow-lg shadow-black/25"
						style="transform:translateX(calc(-50% + {(i - (n - 1) / 2) * 34}px)) rotate({(i - (n - 1) / 2) * 7}deg);z-index:{i}"
					>
						<Cover book={b} width={300} />
					</div>
				{/each}
			</div>
			<div class="text-center">
				<h2 class="text-lg font-semibold">{plural(lib.selected.size, noun)} selected</h2>
				<p class="mt-0.5 text-[13px] text-muted-foreground tabular-nums">{formatSize(lib.selectedSize)} in total</p>
			</div>
			<div class="flex flex-col gap-2">
				{#if lib.selectedIds.length}
					<Button onclick={() => lib.editSelected()}><Pencil />Edit {plural(lib.selectedIds.length, "book")}</Button>
					<Button variant="outline" onclick={() => lib.sendSelected()}><Send />Send to device</Button>
					<Button variant="destructive" onclick={() => lib.askDelete()}><Trash />Delete</Button>
				{/if}
			</div>
		</div>
	{:else}
		<div class="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center text-muted-foreground">
			<Info class="size-6 opacity-50" />
			<p class="text-sm">Select a book to see its details</p>
		</div>
	{/if}
</aside>
