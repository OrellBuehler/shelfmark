import { goto } from "$app/navigation";
import { toast } from "svelte-sonner";
import {
	api,
	inTauri,
	type Book,
	type BookPatch,
	type BookSummary,
	type Category,
	type CategoryItem,
	type LibraryInfo,
	type SavedSearch,
	type SortField
} from "#lib/api";
import { sanitizeHtml } from "#lib/components/edit/sanitize";
import { EBOOK_EXTENSIONS, baseName, pickReadFormat, plural } from "#lib/format";

function preview(titles: string[]) {
	return titles.slice(0, 3).join(", ") + (titles.length > 3 ? ` and ${titles.length - 3} more` : "");
}

export interface Filter {
	search: string;
	label: string;
}

export type ViewMode = "grid" | "table";

const PREFS_KEY = "shelfmark:prefs";

export function errorMessage(e: unknown): string {
	if (typeof e === "string") return e;
	if (e instanceof Error) return e.message;
	return String(e);
}

const FIELD_LABELS: Record<string, string> = {
	authors: "Author",
	author: "Author",
	series: "Series",
	tags: "Tag",
	tag: "Tag",
	publisher: "Publisher",
	publishers: "Publisher",
	languages: "Language",
	formats: "Format",
	rating: "Rating",
	identifiers: "Identifier"
};

function labelFor(search: string): string {
	const m = /^(\w+):(.*)$/.exec(search);
	if (!m) return search;
	let value = m[2].replace(/^"(.*)"$/, "$1").replace(/^=/, "");
	if (m[1] === "identifiers") value = value.replace(/:true$/, "");
	if (m[1] === "rating") value = `${value} ${value === "1" ? "star" : "stars"}`;
	return `${FIELD_LABELS[m[1]] ?? m[1]}: ${value}`;
}

function splitAnd(q: string): string[] | null {
	if (/[()]/.test(q.replace(/"[^"]*"/g, ""))) return null;
	const parts: string[] = [];
	let cur = "";
	for (const t of q.match(/(?:[^\s"]|"[^"]*")+|\s+/g) ?? []) {
		if (/^(or|not)$/i.test(t)) return null;
		if (/^and$/i.test(t)) {
			parts.push(cur.trim());
			cur = "";
		} else cur += t;
	}
	parts.push(cur.trim());
	return parts.filter(Boolean);
}

export function parseQuery(q: string): { filters: Filter[]; text: string } {
	const parts = splitAnd(q);
	if (!parts) return { filters: [], text: q };
	const filters: Filter[] = [];
	const rest: string[] = [];
	for (const p of parts) {
		if (/^(authors|series|tags|publisher|languages|formats|rating):"=[^"]*"$/.test(p) || /^identifiers:\S+:true$/.test(p))
			filters.push({ search: p, label: labelFor(p) });
		else rest.push(p);
	}
	return { filters, text: rest.join(" and ") };
}

function defaultPane(px: number) {
	const w = typeof window === "undefined" ? 1400 : Math.max(window.innerWidth, 900);
	return Math.round((px / w) * 1000) / 10;
}

export const paneSizes = { sidebar: 240, details: 330 };

class LibraryState {
	ready = $state(false);
	info = $state<LibraryInfo | null>(null);
	recents = $state.raw<LibraryInfo[]>([]);

	books = $state.raw<BookSummary[]>([]);
	loaded = $state(false);
	loading = $state(false);
	categories = $state.raw<Category[]>([]);
	progress = $state.raw<Record<string, number>>({});
	savedSearches = $state.raw<SavedSearch[]>([]);
	totalBooks = $state(0);

	filters = $state.raw<Filter[]>([]);
	text = $state("");
	appliedText = $state("");
	sortBy = $state<SortField>("added");
	descending = $state(true);

	view = $state<ViewMode>("grid");
	coverSize = $state(160);
	sidebarOpen = $state(true);
	detailsOpen = $state(true);
	sidebarSize = $state(defaultPane(paneSizes.sidebar));
	detailsSize = $state(defaultPane(paneSizes.details));
	collapsed = $state.raw<Record<string, boolean>>({ tags: false });
	columnWidths = $state.raw<Record<string, number>>({});

	selected = $state.raw<Set<number>>(new Set());
	cursor = $state<number | null>(null);
	anchor: number | null = null;
	cols = 1;
	scrollTops = { grid: 0, table: 0 };
	scrollTo: ((index: number) => void) | null = null;

	busy = $state(0);
	busyLabel = $state("");

	editOpen = $state(false);
	editId = $state<number | null>(null);
	bulkOpen = $state(false);
	bulkIds = $state.raw<number[]>([]);
	devicesOpen = $state(false);
	deleteOpen = $state(false);
	metaOpen = $state(false);
	metaRunning = false;
	dragging = $state(false);
	searchEl: HTMLInputElement | null = null;

	query = $derived.by(() => {
		const parts = this.filters.map((f) => f.search);
		const t = this.appliedText.trim();
		if (t) parts.push(/\bor\b/i.test(t) && parts.length ? `(${t})` : t);
		return parts.join(" and ");
	});

	indexById = $derived(new Map(this.books.map((b, i) => [b.id, i])));

	selectedBooks = $derived.by(() => {
		if (!this.selected.size) return [] as BookSummary[];
		const idx = this.indexById;
		const out: BookSummary[] = [];
		for (const id of this.selected) {
			const i = idx.get(id);
			if (i !== undefined) out.push(this.books[i]);
		}
		return out;
	});

	selectedIds = $derived(this.selectedBooks.map((b) => b.id));
	selectedSize = $derived(this.selectedBooks.reduce((a, b) => a + b.size, 0));

	private booksToken = 0;
	private catsToken = 0;
	private debounce: ReturnType<typeof setTimeout> | undefined;
	private loadedPrefs = false;

	loadPrefs() {
		if (this.loadedPrefs) return;
		this.loadedPrefs = true;
		try {
			const raw = localStorage.getItem(PREFS_KEY);
			if (!raw && window.innerWidth < 1200) this.detailsOpen = false;
			const p = JSON.parse(raw ?? "{}");
			if (p.view === "grid" || p.view === "table") this.view = p.view;
			if (typeof p.coverSize === "number") this.coverSize = p.coverSize;
			if (typeof p.sidebarOpen === "boolean") this.sidebarOpen = p.sidebarOpen;
			if (typeof p.detailsOpen === "boolean") this.detailsOpen = p.detailsOpen;
			if (typeof p.sidebarSize === "number") this.sidebarSize = p.sidebarSize;
			if (typeof p.detailsSize === "number") this.detailsSize = p.detailsSize;
			if (typeof p.sortBy === "string") this.sortBy = p.sortBy;
			if (typeof p.descending === "boolean") this.descending = p.descending;
			if (p.collapsed && typeof p.collapsed === "object") this.collapsed = p.collapsed;
			if (p.columnWidths && typeof p.columnWidths === "object") this.columnWidths = p.columnWidths;
		} catch {
			/* ignore */
		}
	}

	savePrefs() {
		const p = {
			view: this.view,
			coverSize: this.coverSize,
			sidebarOpen: this.sidebarOpen,
			detailsOpen: this.detailsOpen,
			sidebarSize: this.sidebarSize,
			detailsSize: this.detailsSize,
			sortBy: this.sortBy,
			descending: this.descending,
			collapsed: this.collapsed,
			columnWidths: this.columnWidths
		};
		try {
			localStorage.setItem(PREFS_KEY, JSON.stringify(p));
		} catch {
			/* ignore */
		}
	}

	fail(what: string, e: unknown) {
		toast.error(`${what}: ${errorMessage(e)}`);
	}

	async track<T>(label: string, fn: () => Promise<T>): Promise<T> {
		this.busy++;
		this.busyLabel = label;
		try {
			return await fn();
		} finally {
			this.busy--;
		}
	}

	async init() {
		this.loadPrefs();
		if (this.ready) {
			if (this.info) await this.loadProgress();
			return;
		}
		try {
			const [info, recents] = await Promise.all([api.currentLibrary(), api.recentLibraries()]);
			this.recents = recents;
			this.info = info;
		} catch (e) {
			this.fail("Could not read library", e);
		}
		this.ready = true;
		if (this.info) await this.loadAll();
	}

	async refreshRecents() {
		try {
			this.recents = await api.recentLibraries();
		} catch (e) {
			this.fail("Could not load recent libraries", e);
		}
	}

	private resetView() {
		this.books = [];
		this.categories = [];
		this.progress = {};
		this.loaded = false;
		this.filters = [];
		this.text = "";
		this.appliedText = "";
		this.selected = new Set();
		this.cursor = null;
		this.anchor = null;
		this.scrollTops = { grid: 0, table: 0 };
	}

	async openLibrary(path: string, create = false) {
		try {
			const info = await this.track(create ? "Creating library" : "Opening library", () =>
				create ? api.createLibrary(path) : api.openLibrary(path)
			);
			this.resetView();
			this.info = info;
			await Promise.all([this.refreshRecents(), this.loadAll()]);
		} catch (e) {
			this.fail(create ? "Could not create library" : "Could not open library", e);
		}
	}

	async pickAndOpen(create: boolean) {
		const path = await pickFolder(create ? "Choose a folder for the new library" : "Choose a calibre library folder");
		if (path) await this.openLibrary(path, create);
	}

	async loadAll() {
		api.savedSearches().then(
			(s) => (this.savedSearches = s),
			(e) => this.fail("Could not load saved searches", e)
		);
		await Promise.all([this.loadBooks(), this.loadCategories(), this.loadProgress()]);
	}

	async loadBooks() {
		const token = ++this.booksToken;
		this.loading = true;
		try {
			const books = await api.listBooks({ search: this.query || null, sortBy: this.sortBy, descending: this.descending });
			if (token !== this.booksToken) return;
			this.books = books;
			this.loaded = true;
			if (!this.query && this.info) this.totalBooks = books.length;
			this.pruneSelection();
		} catch (e) {
			if (token === this.booksToken) {
				this.loaded = true;
				this.fail("Could not load books", e);
			}
		} finally {
			if (token === this.booksToken) this.loading = false;
		}
	}

	async loadCategories() {
		const token = ++this.catsToken;
		try {
			const cats = await api.categories(this.query || null);
			if (token === this.catsToken) this.categories = cats;
		} catch (e) {
			if (token === this.catsToken) this.fail("Could not load categories", e);
		}
	}

	async loadProgress() {
		try {
			this.progress = await api.readingProgress();
		} catch (e) {
			this.fail("Could not load reading progress", e);
		}
	}

	async reload() {
		await Promise.all([this.loadBooks(), this.loadCategories(), this.loadProgress()]);
		if (this.info) {
			api.currentLibrary().then(
				(i) => i && (this.info = i),
				() => {}
			);
		}
	}

	private pruneSelection() {
		if (!this.selected.size) return;
		const idx = this.indexById;
		const kept = [...this.selected].filter((id) => idx.has(id));
		if (kept.length !== this.selected.size) this.selected = new Set(kept);
		if (this.cursor !== null && this.cursor >= this.books.length) this.cursor = this.books.length ? this.books.length - 1 : null;
	}

	queryChanged() {
		this.selected = new Set();
		this.cursor = null;
		this.anchor = null;
		this.scrollTops = { grid: 0, table: 0 };
		this.scrollTo?.(0);
		void this.loadBooks();
		void this.loadCategories();
	}

	setText(v: string) {
		this.text = v;
		clearTimeout(this.debounce);
		this.debounce = setTimeout(() => {
			this.appliedText = v;
			this.queryChanged();
		}, 220);
	}

	private applyTextNow(v: string) {
		clearTimeout(this.debounce);
		this.text = v;
		this.appliedText = v;
	}

	clearText() {
		this.applyTextNow("");
		this.queryChanged();
	}

	setQuery(q: string) {
		const p = parseQuery(q);
		this.filters = p.filters;
		this.applyTextNow(p.text);
		this.queryChanged();
	}

	clearAll() {
		this.filters = [];
		this.applyTextNow("");
		this.queryChanged();
	}

	isActive(item: CategoryItem) {
		return this.filters.some((f) => f.search === item.search);
	}

	toggleFilter(item: CategoryItem, additive: boolean) {
		const active = this.isActive(item);
		if (active) this.filters = this.filters.filter((f) => f.search !== item.search);
		else if (additive) this.filters = [...this.filters, { search: item.search, label: labelFor(item.search) }];
		else {
			this.filters = [{ search: item.search, label: labelFor(item.search) }];
			this.applyTextNow("");
		}
		this.queryChanged();
	}

	addFilter(search: string) {
		this.filters = [{ search, label: labelFor(search) }];
		this.applyTextNow("");
		this.queryChanged();
	}

	removeFilter(search: string) {
		this.filters = this.filters.filter((f) => f.search !== search);
		this.queryChanged();
	}

	async saveCurrentSearch(name: string) {
		const q = this.query;
		if (!q || !name.trim()) return;
		const next = [...this.savedSearches.filter((s) => s.name !== name.trim()), { name: name.trim(), query: q }];
		await this.persistSaved(next);
	}

	async deleteSavedSearch(name: string) {
		await this.persistSaved(this.savedSearches.filter((s) => s.name !== name));
	}

	private async persistSaved(next: SavedSearch[]) {
		const prev = this.savedSearches;
		this.savedSearches = next;
		try {
			await api.setSavedSearches(next);
		} catch (e) {
			this.savedSearches = prev;
			this.fail("Could not save searches", e);
		}
	}

	setSort(field: SortField, descending?: boolean) {
		if (descending === undefined) {
			if (this.sortBy === field) descending = !this.descending;
			else descending = field === "added" || field === "published" || field === "modified" || field === "rating" || field === "size";
		}
		this.sortBy = field;
		this.descending = descending;
		this.resetScroll();
	}

	setDescending(d: boolean) {
		this.descending = d;
		this.resetScroll();
	}

	private resetScroll() {
		this.scrollTops = { grid: 0, table: 0 };
		void this.loadBooks().then(() => this.scrollTo?.(0));
	}

	isSelected(id: number) {
		return this.selected.has(id);
	}

	selectIndex(i: number, mods: { shift?: boolean; toggle?: boolean } = {}) {
		const book = this.books[i];
		if (!book) return;
		if (mods.shift && this.anchor !== null) {
			const [a, b] = this.anchor < i ? [this.anchor, i] : [i, this.anchor];
			const next = new Set(mods.toggle ? this.selected : []);
			for (let k = a; k <= b; k++) next.add(this.books[k].id);
			this.selected = next;
		} else if (mods.toggle) {
			const next = new Set(this.selected);
			if (!next.delete(book.id)) next.add(book.id);
			this.selected = next;
			this.anchor = i;
		} else {
			this.selected = new Set([book.id]);
			this.anchor = i;
		}
		this.cursor = i;
	}

	selectAll() {
		this.selected = new Set(this.books.map((b) => b.id));
	}

	clearSelection() {
		this.selected = new Set();
		this.anchor = null;
	}

	selectContextTarget(id: number) {
		if (this.selected.has(id)) return;
		const i = this.indexById.get(id);
		if (i !== undefined) this.selectIndex(i);
	}

	move(dx: number, dy: number, extend: boolean) {
		const n = this.books.length;
		if (!n) return;
		let i: number;
		if (this.cursor === null || this.cursor >= n) i = 0;
		else i = Math.max(0, Math.min(n - 1, this.cursor + dx + dy * this.cols));
		this.selectIndex(i, { shift: extend });
		this.scrollTo?.(i);
	}

	patchBook(book: Book) {
		const i = this.indexById.get(book.id);
		if (i === undefined) return;
		const { comments: _c, uuid: _u, path: _p, files: _f, ...summary } = book;
		const next = this.books.slice();
		next[i] = summary;
		this.books = next;
	}

	async setRating(id: number, rating: number) {
		try {
			const book = await api.updateBook(id, { rating });
			this.patchBook(book);
			void this.loadCategories();
			return book;
		} catch (e) {
			this.fail("Could not update rating", e);
		}
	}

	readBook(book: BookSummary, format?: string) {
		const f = format ?? pickReadFormat(book.formats);
		if (f) void goto(`/reader/${book.id}?format=${encodeURIComponent(f)}`);
		else if (book.formats.length) {
			api.openExternally(book.id, book.formats[0]).then(
				() => toast.info(`Opened ${book.formats[0]} in the system app`),
				(e) => this.fail("Could not open book", e)
			);
		} else toast.error("This book has no files");
	}

	readSelected() {
		const b = this.selectedBooks[0];
		if (b) this.readBook(b);
	}

	openExternally(book: BookSummary) {
		const f = book.formats[0];
		if (!f) return toast.error("This book has no files");
		api.openExternally(book.id, f).catch((e) => this.fail("Could not open book", e));
	}

	showInFolder(book: BookSummary) {
		api.showInFolder(book.id).catch((e) => this.fail("Could not show in folder", e));
	}

	editSelected() {
		const ids = this.selectedIds;
		if (ids.length === 1) {
			this.editId = ids[0];
			this.editOpen = true;
		} else if (ids.length > 1) {
			this.bulkIds = ids;
			this.bulkOpen = true;
		}
	}

	editBook(id: number) {
		this.editId = id;
		this.editOpen = true;
	}

	sendSelected() {
		if (this.selected.size) this.devicesOpen = true;
	}

	askDelete() {
		if (this.selected.size) this.deleteOpen = true;
	}

	async deleteSelected() {
		const ids = this.selectedIds;
		if (!ids.length) return;
		try {
			await this.track("Deleting", () => api.deleteBooks(ids));
			toast.success(`Deleted ${plural(ids.length, "book")}`, { description: "Moved to the library's trash." });
			this.selected = new Set();
			this.anchor = null;
			await this.reload();
		} catch (e) {
			this.fail("Could not delete", e);
		}
	}

	async embedSelected() {
		const ids = this.selectedIds;
		if (!ids.length) return;
		try {
			const n = await this.track("Embedding metadata", () => api.embedMetadata(ids));
			toast.success(`Embedded metadata into ${plural(n, "book")}`);
		} catch (e) {
			this.fail("Could not embed metadata", e);
		}
	}

	askDownloadMetadata() {
		if (this.selected.size && !this.metaRunning) this.metaOpen = true;
	}

	async downloadMetadataSelected() {
		const books = this.selectedBooks;
		if (!books.length || this.metaRunning) return;
		this.metaRunning = true;
		let stop = false;
		let updated = 0;
		const missed: string[] = [];
		const failed: string[] = [];
		const id = toast.loading("Downloading metadata");
		for (const [i, book] of books.entries()) {
			if (stop) break;
			toast.loading(`Downloading metadata ${i + 1} of ${books.length}`, {
				id,
				description: book.title,
				action: { label: "Stop", onClick: () => (stop = true) }
			});
			try {
				const r = await api.bestMetadata(book.id);
				if (!r) {
					missed.push(book.title);
					continue;
				}
				const m = r.metadata;
				const patch: BookPatch = { identifiers: { ...book.identifiers, ...m.identifiers } };
				if (m.title) patch.title = m.title;
				if (m.authors.length) patch.authors = m.authors;
				if (m.series) {
					patch.series = m.series;
					patch.seriesIndex = m.seriesIndex ?? 1;
				}
				if (m.tags.length) patch.tags = m.tags;
				if (m.publisher) patch.publisher = m.publisher;
				if (m.pubdate) patch.pubdate = m.pubdate.slice(0, 10);
				if (m.languages.length) patch.languages = m.languages;
				if (m.comments) patch.comments = sanitizeHtml(m.comments);
				await api.updateBook(book.id, patch);
				if (r.coverUrl) await api.setCoverFromUrl(book.id, r.coverUrl).catch(() => {});
				updated++;
			} catch {
				failed.push(book.title);
			}
		}
		this.metaRunning = false;
		const notes = [];
		if (missed.length) notes.push(`No confident match for ${plural(missed.length, "book")}: ${preview(missed)}`);
		if (failed.length) notes.push(`Failed for ${plural(failed.length, "book")}: ${preview(failed)}`);
		if (stop) notes.push("Stopped early.");
		const done = `Updated metadata for ${plural(updated, "book")}`;
		const opts = { id, description: notes.join(" ") || undefined, action: undefined, duration: notes.length ? 15000 : 4000 };
		if (updated) toast.success(done, opts);
		else toast.warning(done, opts);
		await this.reload();
	}

	async addPaths(paths: string[]) {
		const wanted = paths.filter((p) => {
			const ext = p.split(".").pop()?.toLowerCase() ?? "";
			return !p.split(/[\\/]/).pop()?.includes(".") || EBOOK_EXTENSIONS.includes(ext);
		});
		const skipped = paths.length - wanted.length;
		if (!wanted.length) return toast.error("No supported e-book files to add");
		try {
			const out = await this.track(`Adding ${plural(wanted.length, "file")}`, () => api.addBooks(wanted));
			const ok = out.filter((o) => o.bookId !== null);
			const bad = out.filter((o) => o.error);
			const detail = bad
				.slice(0, 4)
				.map((o) => `${baseName(o.path)}: ${o.error}`)
				.join("\n");
			const more = bad.length > 4 ? `\n and ${bad.length - 4} more` : "";
			if (!ok.length) toast.error(`Could not add ${plural(bad.length, "file")}`, { description: detail + more });
			else if (bad.length) toast.warning(`Added ${ok.length} of ${wanted.length} files`, { description: detail + more });
			else
				toast.success(`Added ${plural(ok.length, "book")}`, {
					description: skipped ? `Skipped ${plural(skipped, "unsupported file")}` : undefined
				});
			await this.reload();
			if (ok.length) {
				const first = this.indexById.get(ok[0].bookId!);
				if (first !== undefined && ok.length === 1) this.selectIndex(first);
			}
		} catch (e) {
			this.fail("Could not add books", e);
		}
	}

	async pickAndAdd() {
		if (!inTauri) {
			return this.addPaths(["/home/reader/Downloads/New Arrival.epub", "/home/reader/Downloads/Scan 0042.pdf", "/home/reader/Downloads/notes.xyz"]);
		}
		const { open } = await import("@tauri-apps/plugin-dialog");
		try {
			const picked = await open({
				multiple: true,
				directory: false,
				title: "Add books",
				filters: [{ name: "E-books", extensions: EBOOK_EXTENSIONS }]
			});
			if (picked && picked.length) await this.addPaths(picked);
		} catch (e) {
			this.fail("Could not open file picker", e);
		}
	}
}

export async function pickFolder(title: string): Promise<string | null> {
	if (!inTauri) return `/home/reader/Libraries/Library ${Math.floor(Math.random() * 90 + 10)}`;
	try {
		const { open } = await import("@tauri-apps/plugin-dialog");
		const p = await open({ directory: true, multiple: false, title });
		return typeof p === "string" ? p : null;
	} catch (e) {
		toast.error(`Could not open folder picker: ${errorMessage(e)}`);
		return null;
	}
}

export const lib = new LibraryState();
