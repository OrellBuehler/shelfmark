<script lang="ts">
	import CloudDownloadIcon from "@lucide/svelte/icons/cloud-download";
	import WandIcon from "@lucide/svelte/icons/wand-sparkles";
	import XIcon from "@lucide/svelte/icons/x";
	import LoaderIcon from "@lucide/svelte/icons/loader-circle";
	import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
	import { tick, untrack } from "svelte";
	import { toast } from "svelte-sonner";
	import { api, coverUrl, inTauri, type Book, type BookPatch } from "$lib/api";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import { Skeleton } from "$lib/components/ui/skeleton/index.js";
	import * as Dialog from "$lib/components/ui/dialog/index.js";
	import * as AlertDialog from "$lib/components/ui/alert-dialog/index.js";
	import { cn } from "$lib/utils.js";
	import Autocomplete from "./Autocomplete.svelte";
	import ChipsInput from "./ChipsInput.svelte";
	import CoverPicker from "./CoverPicker.svelte";
	import DownloadMetadataPanel, {
		type AppliedMetadata,
		type CurrentMetadata
	} from "./DownloadMetadataPanel.svelte";
	import FormatsList from "./FormatsList.svelte";
	import HtmlEditor from "./HtmlEditor.svelte";
	import IdentifiersEditor, { type IdentifierRow } from "./IdentifiersEditor.svelte";
	import RatingStars from "./RatingStars.svelte";
	import { autoAuthorSort, autoTitleSort } from "./format";
	import { LANGUAGES, languageName, resolveLanguage } from "./languages";
	import { sanitizeHtml } from "./sanitize";

	let {
		open = $bindable(false),
		bookId,
		onsaved
	}: { open: boolean; bookId: number | null; onsaved?: () => void } = $props();

	type Form = {
		title: string;
		titleSort: string;
		authors: string[];
		authorSort: string;
		series: string;
		seriesIndex: number | null;
		tags: string[];
		publisher: string;
		pubdate: string;
		languages: string[];
		rating: number;
		identifiers: IdentifierRow[];
		comments: string;
	};

	const emptyForm = (): Form => ({
		title: "",
		titleSort: "",
		authors: [],
		authorSort: "",
		series: "",
		seriesIndex: null,
		tags: [],
		publisher: "",
		pubdate: "",
		languages: [],
		rating: 0,
		identifiers: [],
		comments: ""
	});

	let book = $state<Book | null>(null);
	let form = $state<Form>(emptyForm());
	let baselineComments = "";
	let titleSortAuto = $state(true);
	let authorSortAuto = $state(true);
	let pendingCover = $state<string | null>(null);
	let loading = $state(false);
	let loadError = $state<string | null>(null);
	let saving = $state(false);
	let view = $state<"edit" | "download">("edit");
	let panelMounted = $state(false);
	let confirmDiscard = $state(false);
	let libraryTouched = false;
	let titleEl = $state<HTMLInputElement | null>(null);
	let loadedId: number | null = null;

	let completions = $state({
		authors: [] as string[],
		series: [] as string[],
		tags: [] as string[],
		publisher: [] as string[],
		languages: [] as string[]
	});

	const languageOptions = $derived([
		...new Set([...LANGUAGES.map(([c]) => c), ...completions.languages])
	]);

	const effTitleSort = $derived(titleSortAuto ? autoTitleSort(form.title) : form.titleSort);
	const effAuthorSort = $derived(authorSortAuto ? autoAuthorSort(form.authors) : form.authorSort);

	function dateOf(v: string | null): string {
		if (!v) return "";
		const d = v.slice(0, 10);
		return /^(0000|0101)-/.test(d) ? "" : d;
	}

	function sameSet(a: string[], b: string[]) {
		return a.length === b.length && [...a].sort().join("\n") === [...b].sort().join("\n");
	}

	function idRecord(rows: IdentifierRow[]): Record<string, string> {
		const out: Record<string, string> = {};
		for (const r of rows) {
			const k = r.key.trim().toLowerCase();
			const v = r.value.trim();
			if (k && v) out[k] = v;
		}
		return out;
	}

	function idRows(rec: Record<string, string>): IdentifierRow[] {
		return Object.entries(rec).map(([key, value]) => ({ key, value }));
	}

	function sameRecord(a: Record<string, string>, b: Record<string, string>) {
		const ka = Object.keys(a).sort();
		const kb = Object.keys(b).sort();
		return ka.join("\n") === kb.join("\n") && ka.every((k) => a[k] === b[k]);
	}

	const patch = $derived.by<BookPatch>(() => {
		const b = book;
		if (!b) return {};
		const p: BookPatch = {};
		const title = form.title.trim();
		if (title !== b.title) p.title = title;
		const sort = effTitleSort.trim();
		if (sort !== (b.sort ?? "") && (title !== b.title || sort)) p.titleSort = sort;
		const authors = form.authors;
		if (authors.join("\n") !== b.authors.join("\n")) p.authors = authors;
		const authorSort = effAuthorSort.trim();
		if (authorSort !== (b.authorSort ?? "") && (authors.length || authorSort)) p.authorSort = authorSort;
		const series = form.series.trim();
		if (series !== (b.series ?? "")) {
			p.series = series;
			if (series) p.seriesIndex = form.seriesIndex ?? 1;
		} else if (series && form.seriesIndex !== null && form.seriesIndex !== b.seriesIndex) {
			p.seriesIndex = form.seriesIndex;
		}
		if (!sameSet(form.tags, b.tags)) p.tags = form.tags;
		const publisher = form.publisher.trim();
		if (publisher !== (b.publisher ?? "")) p.publisher = publisher;
		if (form.pubdate !== dateOf(b.pubdate)) p.pubdate = form.pubdate ? `${form.pubdate}T00:00:00+00:00` : "";
		if (!sameSet(form.languages, b.languages)) p.languages = form.languages;
		if (form.rating !== (b.rating ?? 0)) p.rating = form.rating;
		if (!sameRecord(idRecord(form.identifiers), b.identifiers)) p.identifiers = idRecord(form.identifiers);
		if (form.comments !== baselineComments) p.comments = form.comments;
		return p;
	});

	const dirty = $derived(Object.keys(patch).length > 0 || pendingCover !== null);

	function fill(b: Book) {
		book = b;
		const sort = b.sort ?? "";
		form = {
			title: b.title,
			titleSort: sort,
			authors: [...b.authors],
			authorSort: b.authorSort ?? "",
			series: b.series ?? "",
			seriesIndex: b.series ? b.seriesIndex : null,
			tags: [...b.tags],
			publisher: b.publisher ?? "",
			pubdate: dateOf(b.pubdate),
			languages: [...b.languages],
			rating: b.rating ?? 0,
			identifiers: idRows(b.identifiers),
			comments: sanitizeHtml(b.comments)
		};
		baselineComments = form.comments;
		titleSortAuto = !sort || sort === autoTitleSort(b.title);
		authorSortAuto = !b.authorSort || b.authorSort === autoAuthorSort(b.authors);
	}

	async function load(id: number) {
		loading = true;
		loadError = null;
		book = null;
		pendingCover = null;
		view = "edit";
		panelMounted = false;
		libraryTouched = false;
		loadedId = id;
		const [fetched, ...lists] = await Promise.allSettled([
			api.getBook(id),
			api.completions("authors"),
			api.completions("series"),
			api.completions("tags"),
			api.completions("publisher"),
			api.completions("languages")
		]);
		if (loadedId !== id) return;
		const val = (r: PromiseSettledResult<unknown>) => (r.status === "fulfilled" ? (r.value as string[]) : []);
		completions = {
			authors: val(lists[0]),
			series: val(lists[1]),
			tags: val(lists[2]),
			publisher: val(lists[3]),
			languages: val(lists[4])
		};
		if (fetched.status === "fulfilled") {
			fill(fetched.value);
			loading = false;
			await tick();
			titleEl?.focus();
			titleEl?.setSelectionRange(0, 0);
			if (titleEl) titleEl.scrollLeft = 0;
		} else {
			loadError = String(fetched.reason);
			loading = false;
		}
	}

	$effect(() => {
		if (open && bookId !== null) {
			const id = bookId;
			untrack(() => load(id));
		} else if (!open) {
			loadedId = null;
		}
	});

	async function refreshBook() {
		if (!book) return;
		libraryTouched = true;
		try {
			book = await api.getBook(book.id);
		} catch (e) {
			toast.error(`Could not reload book: ${e}`);
		}
	}

	function applyDownloaded(a: AppliedMetadata) {
		if (a.title !== undefined) form.title = a.title;
		if (a.authors) form.authors = a.authors;
		if (a.series !== undefined) {
			form.series = a.series;
			form.seriesIndex = a.seriesIndex ?? form.seriesIndex ?? 1;
		}
		if (a.publisher !== undefined) form.publisher = a.publisher;
		if (a.pubdate !== undefined) form.pubdate = a.pubdate;
		if (a.languages) form.languages = a.languages;
		if (a.tags) form.tags = a.tags;
		if (a.identifiers) form.identifiers = idRows(a.identifiers);
		if (a.rating !== undefined) form.rating = a.rating;
		if (a.comments !== undefined) form.comments = a.comments;
		if (a.coverUrl) pendingCover = a.coverUrl;
		view = "edit";
		tick().then(() => titleEl?.focus());
	}

	function close() {
		confirmDiscard = false;
		if (libraryTouched) onsaved?.();
		open = false;
	}

	function requestClose() {
		if (view === "download") view = "edit";
		else if (dirty && !saving) confirmDiscard = true;
		else close();
	}

	async function save() {
		if (!book || saving || view !== "edit") return;
		if (!form.title.trim()) {
			toast.error("Title can't be empty");
			titleEl?.focus();
			return;
		}
		saving = true;
		try {
			if (pendingCover) await api.setCoverFromUrl(book.id, pendingCover);
			if (Object.keys(patch).length) await api.updateBook(book.id, patch);
			toast.success("Metadata saved");
			libraryTouched = false;
			onsaved?.();
			open = false;
		} catch (e) {
			toast.error(`Could not save: ${e}`);
		} finally {
			saving = false;
		}
	}

	function onkeydown(e: KeyboardEvent) {
		if (!open || confirmDiscard) return;
		if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
			e.preventDefault();
			save();
		}
	}

	const current = $derived<CurrentMetadata>({
		title: form.title,
		authors: form.authors,
		series: form.series,
		seriesIndex: form.seriesIndex,
		publisher: form.publisher,
		pubdate: form.pubdate,
		languages: form.languages,
		tags: form.tags,
		identifiers: idRecord(form.identifiers),
		comments: form.comments,
		rating: form.rating,
		coverSrc: pendingCover ?? (book?.hasCover && inTauri ? coverUrl(book, 120) : null)
	});

	function startDownload() {
		panelMounted = true;
		view = "download";
	}

	const isbn = $derived(idRecord(form.identifiers).isbn ?? "");
</script>

{#snippet label(text: string, forId: string, hint?: string)}
	<label for={forId} class="text-muted-foreground mb-1.5 flex items-baseline gap-2 text-xs font-medium">
		{text}
		{#if hint}<span class="font-normal opacity-70">{hint}</span>{/if}
	</label>
{/snippet}

{#snippet sortRow(name: string, id: string, auto: boolean, value: string, oninput: (v: string) => void, reset: () => void)}
	<div class="mt-1.5 flex items-center gap-2">
		<label for={id} class="text-muted-foreground w-16 shrink-0 text-xs">{name}</label>
		<input
			{id}
			{value}
			spellcheck="false"
			class={cn(
				"border-input focus-visible:border-ring focus-visible:ring-ring/50 h-7 min-w-0 flex-1 rounded-md border bg-transparent px-2 text-xs shadow-xs outline-none focus-visible:ring-3",
				auto && "text-muted-foreground"
			)}
			oninput={(e) => oninput(e.currentTarget.value)}
		/>
		<button
			type="button"
			title={auto ? "Derived automatically" : "Derive automatically"}
			aria-label="Derive {name.toLowerCase()} automatically"
			aria-pressed={auto}
			class={cn(
				"grid size-7 shrink-0 place-items-center rounded-md transition-colors",
				auto ? "bg-brand/20 text-foreground" : "text-muted-foreground hover:bg-accent hover:text-foreground"
			)}
			onclick={reset}
		>
			<WandIcon class="size-3.5" />
		</button>
	</div>
{/snippet}

<svelte:window {onkeydown} />

<Dialog.Root
	bind:open={
		() => open,
		(v) => {
			if (v) open = true;
			else requestClose();
		}
	}
>
	<Dialog.Content
		class="flex h-[min(90vh,820px)] w-[900px] max-w-[calc(100%-2rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[900px]"
		onOpenAutoFocus={(e) => e.preventDefault()}
	>
		<header class="flex items-center gap-3 border-b py-3.5 pr-14 pl-6">
			<div class="min-w-0 flex-1">
				<Dialog.Title class="text-base font-semibold">
					{view === "download" ? "Download metadata" : "Edit metadata"}
				</Dialog.Title>
				<Dialog.Description class="text-muted-foreground mt-0.5 truncate text-xs">
					{#if book}
						{book.title}{book.authors.length ? ` · ${book.authors.join(", ")}` : ""}
					{:else}
						&nbsp;
					{/if}
				</Dialog.Description>
			</div>
			{#if view === "edit"}
				<Button variant="outline" size="sm" onclick={startDownload} disabled={!book}>
					<CloudDownloadIcon />
					Download metadata
				</Button>
			{/if}
		</header>

		{#if loading}
			<div class="grid flex-1 grid-cols-[13rem_1fr] gap-6 p-6">
				<div class="space-y-3">
					<Skeleton class="aspect-[2/3] w-full" />
					<Skeleton class="h-8 w-full" />
				</div>
				<div class="space-y-5">
					{#each [0, 1, 2, 3, 4] as i (i)}
						<div class="space-y-2">
							<Skeleton class="h-3 w-20" />
							<Skeleton class="h-9 w-full" />
						</div>
					{/each}
				</div>
			</div>
		{:else if loadError}
			<div class="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
				<TriangleAlertIcon class="text-destructive size-7" />
				<div>
					<p class="text-sm font-medium">Could not load this book</p>
					<p class="text-muted-foreground mt-1 max-w-sm text-xs break-words">{loadError}</p>
				</div>
				<Button variant="outline" size="sm" onclick={() => bookId !== null && load(bookId)}>Try again</Button>
			</div>
		{:else if book}
			<div class={cn("min-h-0 flex-1 grid-cols-[13rem_1fr] gap-7 overflow-y-auto p-6", view === "edit" ? "grid" : "hidden")}>
				<aside class="space-y-6 self-start">
					<CoverPicker
						{book}
						pendingUrl={pendingCover}
						onchanged={refreshBook}
						ondiscardpending={() => (pendingCover = null)}
					/>
					<div>
						<h3 class="text-muted-foreground mb-2 text-xs font-medium">Formats</h3>
						<FormatsList
							{book}
							onchanged={(b) => {
								book = b;
								libraryTouched = true;
							}}
						/>
					</div>
				</aside>

				<div class="min-w-0 space-y-5 pb-2">
					<div>
						{@render label("Title", "em-title")}
						<Input id="em-title" bind:value={form.title} bind:ref={titleEl} class="h-10 text-base font-medium" />
						{@render sortRow("Title sort", "em-title-sort", titleSortAuto, effTitleSort, (v) => {
							form.titleSort = v;
							titleSortAuto = false;
						}, () => (titleSortAuto = true))}
					</div>

					<div>
						{@render label("Authors", "em-authors", "Enter or & to add")}
						<ChipsInput
							id="em-authors"
							bind:value={form.authors}
							options={completions.authors}
							separators="&;"
							placeholder="Add author"
						/>
						{@render sortRow("Author sort", "em-author-sort", authorSortAuto, effAuthorSort, (v) => {
							form.authorSort = v;
							authorSortAuto = false;
						}, () => (authorSortAuto = true))}
					</div>

					<div class="grid grid-cols-[1fr_7rem] gap-3">
						<div>
							{@render label("Series", "em-series")}
							<Autocomplete id="em-series" bind:value={form.series} options={completions.series} placeholder="No series" />
						</div>
						<div>
							{@render label("Number", "em-series-index")}
							<Input
								id="em-series-index"
								type="number"
								step="1"
								min="0"
								bind:value={form.seriesIndex}
								disabled={!form.series.trim()}
								placeholder="1"
							/>
						</div>
					</div>

					<div>
						{@render label("Tags", "em-tags")}
						<ChipsInput id="em-tags" bind:value={form.tags} options={completions.tags} placeholder="Add tag" />
					</div>

					<div class="grid grid-cols-2 gap-3">
						<div>
							{@render label("Publisher", "em-publisher")}
							<Autocomplete id="em-publisher" bind:value={form.publisher} options={completions.publisher} />
						</div>
						<div>
							{@render label("Published", "em-pubdate")}
							<div class="relative">
								<Input id="em-pubdate" type="date" bind:value={form.pubdate} class="pr-8" />
								{#if form.pubdate}
									<button
										type="button"
										aria-label="Clear date"
										class="text-muted-foreground hover:text-foreground absolute top-1/2 right-8 grid size-5 -translate-y-1/2 place-items-center rounded"
										onclick={() => (form.pubdate = "")}
									>
										<XIcon class="size-3.5" />
									</button>
								{/if}
							</div>
						</div>
					</div>

					<div class="grid grid-cols-2 gap-3">
						<div>
							{@render label("Languages", "em-languages")}
							<ChipsInput
								id="em-languages"
								bind:value={form.languages}
								options={languageOptions}
								labelOf={languageName}
								resolve={resolveLanguage}
								placeholder="Add language"
							/>
						</div>
						<div>
							{@render label("Rating", "em-rating")}
							<RatingStars id="em-rating" bind:value={form.rating} class="h-9" />
						</div>
					</div>

					<div>
						{@render label("Identifiers", "em-identifiers")}
						<div id="em-identifiers">
							<IdentifiersEditor bind:rows={form.identifiers} />
						</div>
					</div>

					<div>
						{@render label("Description", "em-description")}
						<HtmlEditor id="em-description" bind:value={form.comments} />
					</div>
				</div>
			</div>

			{#if panelMounted}
				<div class={cn("min-h-0 flex-1 flex-col", view === "download" ? "flex" : "hidden")}>
					<DownloadMetadataPanel
						{current}
						query={{ title: form.title, authors: form.authors.join(", "), isbn }}
						onback={() => (view = "edit")}
						onapply={applyDownloaded}
					/>
				</div>
			{/if}
		{/if}

		{#if view === "edit"}
			<footer class="flex items-center gap-3 border-t px-6 py-3">
				<span class="text-muted-foreground mr-auto text-xs">
					{#if dirty}
						<span class="bg-brand mr-1.5 inline-block size-1.5 rounded-full align-middle"></span>
						Unsaved changes
					{:else}
						<kbd class="bg-muted rounded px-1.5 py-0.5 font-sans text-[0.7rem]">Ctrl</kbd>
						+
						<kbd class="bg-muted rounded px-1.5 py-0.5 font-sans text-[0.7rem]">Enter</kbd>
						to save
					{/if}
				</span>
				<Button variant="ghost" onclick={requestClose}>Cancel</Button>
				<Button onclick={save} disabled={!book || saving || !dirty}>
					{#if saving}
						<LoaderIcon class="animate-spin" />
					{/if}
					Save changes
				</Button>
			</footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<AlertDialog.Root bind:open={confirmDiscard}>
	<AlertDialog.Content size="sm">
		<AlertDialog.Header>
			<AlertDialog.Title>Discard changes?</AlertDialog.Title>
			<AlertDialog.Description>Your edits to this book have not been saved.</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Keep editing</AlertDialog.Cancel>
			<AlertDialog.Action variant="destructive" onclick={close}>Discard</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
