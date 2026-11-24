import { api, bookFileUrl, inTauri, type Annotation, type BookId } from "$lib/api";
import type {
	FoliateBook,
	RelocateDetail,
	SearchHit,
	TocItem,
	View
} from "foliate-js/view.js";
import * as data from "./data";
import { prefs, type Prefs, type ThemeName } from "./prefs.svelte";
import { buildCss, colorValue, palettes } from "./themes";

export type PanelTab = "toc" | "search" | "notes";
export type ErrorKind = "missing" | "unsupported" | "failed";

export interface Place {
	fraction: number;
	chapter: string;
	tocHref: string | null;
	minutesLeft: number | null;
	chapterMinutesLeft: number | null;
	page: { current: number; total: number } | null;
}

export interface Box {
	left: number;
	top: number;
	width: number;
	height: number;
}

export interface Selection {
	cfi: string;
	text: string;
	index: number;
	chapter: string;
	box: Box;
}

export interface SearchGroup {
	label: string;
	hits: SearchHit[];
}

const MAX_SEARCH_HITS = 500;
const EXTENSIONS: Record<string, string> = {
	EPUB: "epub",
	KEPUB: "epub",
	MOBI: "mobi",
	AZW: "azw",
	AZW3: "azw3",
	PDF: "pdf",
	CBZ: "cbz",
	FB2: "fb2",
	FBZ: "fbz"
};

function textOf(value: unknown): string {
	if (!value) return "";
	if (typeof value === "string") return value;
	if (Array.isArray(value)) return value.map(textOf).filter(Boolean).join(", ");
	if (typeof value === "object") {
		const o = value as Record<string, unknown>;
		if ("name" in o) return textOf(o.name);
		const first = Object.values(o)[0];
		return textOf(first);
	}
	return "";
}

export class Reader {
	status = $state<"loading" | "ready" | "error">("loading");
	errorKind = $state<ErrorKind>("failed");
	errorMessage = $state("");
	title = $state("");
	author = $state("");
	toc = $state.raw<TocItem[]>([]);
	place = $state<Place | null>(null);
	fixed = $state(false);
	canSearch = $state(true);
	hasText = $state(true);
	annotations = $state<Annotation[]>([]);
	selection = $state<Selection | null>(null);
	editor = $state<{ id: string; box: Box } | null>(null);
	lightbox = $state<string | null>(null);
	jumpBack = $state(false);
	chromeVisible = $state(true);
	chromeHeld = $state(false);
	panel = $state<PanelTab | null>(null);
	appearanceOpen = $state(false);
	fullscreen = $state(false);
	sectionFractions = $state.raw<number[]>([]);
	search = $state({
		query: "",
		running: false,
		progress: 0,
		done: false,
		total: 0,
		active: -1,
		groups: [] as SearchGroup[]
	});

	bookmarkHere = $derived.by(() => {
		void this.place;
		return this.annotations.find((a) => a.kind === "bookmark" && this.#isHere(a.position)) ?? null;
	});
	searchHits = $derived(this.search.groups.flatMap((g) => g.hits));

	view: View | null = null;
	#host: HTMLElement | null = null;
	#prefs: Prefs | null = null;
	#theme: ThemeName = "light";
	#ready = false;
	#destroyed = false;
	#loadedPosition: string | null = null;
	#pending: { position: string; progress: number } | null = null;
	#saveTimer: ReturnType<typeof setTimeout> | undefined;
	#chromeTimer: ReturnType<typeof setTimeout> | undefined;
	#clickTimer: ReturnType<typeof setTimeout> | undefined;
	#jumpTimer: ReturnType<typeof setTimeout> | undefined;
	#annotationClicked = false;
	#searchRun = 0;
	#searchTimer: ReturnType<typeof setTimeout> | undefined;
	#wheelLock = false;
	#indexCache = new Map<string, number>();
	#cfi: typeof import("foliate-js/epubcfi.js") | null = null;
	#Overlayer: typeof import("foliate-js/overlayer.js").Overlayer | null = null;

	format = $state("");

	constructor(
		readonly id: BookId,
		format: string | null,
		readonly src: string | null
	) {
		this.format = format ?? "";
	}

	async #pickFormat() {
		try {
			const book = await api.getBook(this.id);
			const formats = book.files.map((f) => f.format.toUpperCase());
			const order = ["EPUB", "KEPUB", "AZW3", "MOBI", "FB2", "FBZ", "PDF", "CBZ"];
			return order.find((f) => formats.includes(f)) ?? formats[0] ?? "EPUB";
		} catch {
			return "EPUB";
		}
	}

	async open(host: HTMLElement) {
		this.#host = host;
		try {
			const [, overlayer, cfi] = await Promise.all([
				import("foliate-js/view.js"),
				import("foliate-js/overlayer.js"),
				import("foliate-js/epubcfi.js")
			]);
			this.#Overlayer = overlayer.Overlayer;
			this.#cfi = cfi;

			if (!this.format) this.format = await this.#pickFormat();
			const meta = api.getBook(this.id).catch(() => null);
			const file = await this.#fetchFile();
			if (this.#destroyed) return;

			const view = document.createElement("foliate-view") as View;
			view.setAttribute("autohide-cursor", "");
			view.style.cssText = "display:block;width:100%;height:100%;box-sizing:border-box";
			host.append(view);
			this.view = view;
			this.#listen(view);

			const { makeBook } = await import("foliate-js/view.js");
			const book = await makeBook(file);
			if (["PDF", "CBZ"].includes(this.format.toUpperCase()))
				book.rendition = { ...book.rendition, spread: "none" };
			await view.open(book);
			if (this.#destroyed) return;

			this.fixed = view.isFixedLayout;
			this.hasText = !["PDF", "CBZ"].includes(this.format.toUpperCase());
			this.canSearch = book.sections.some((s) => s.createDocument);
			this.toc = book.toc ?? [];
			this.sectionFractions = view.getSectionFractions();

			const info = await meta;
			this.title = info?.title || textOf(book.metadata?.title) || this.src?.split("/").pop() || "";
			this.author = info?.authors.join(", ") || textOf(book.metadata?.author);
			this.#guardResources(book);

			if (!this.fixed) view.renderer.setAttribute("margin", "52px");
			if (!matchMedia("(prefers-reduced-motion: reduce)").matches)
				view.renderer.setAttribute("animated", "");
			this.#apply();

			const [state, annotations] = await Promise.all([
				data.loadState(this.id),
				data.loadAnnotations(this.id)
			]);
			if (this.#destroyed) return;
			this.annotations = annotations;
			this.#loadedPosition =
				state?.position && state.format.toUpperCase() === this.format.toUpperCase()
					? state.position
					: null;
			try {
				await view.init({ lastLocation: this.#loadedPosition });
			} catch (e) {
				console.warn("could not restore position", e);
				await view.init({});
			}
			this.#ready = true;
			this.status = "ready";
			this.#schedulePoke();
		} catch (e) {
			if (this.#destroyed) return;
			console.error(e);
			await this.#fail(e);
		}
	}

	async #fetchFile() {
		const url = !inTauri && this.src ? this.src : bookFileUrl(this.id, this.format);
		let res: Response;
		try {
			res = await fetch(url);
		} catch (e) {
			throw new MissingFile(String(e));
		}
		if (!res.ok) throw new MissingFile(`${res.status} ${res.statusText}`);
		const blob = await res.blob();
		const ext = EXTENSIONS[this.format.toUpperCase()] ?? this.format.toLowerCase();
		return new File([blob], `book-${this.id}.${ext}`, { type: blob.type });
	}

	async #fail(e: unknown) {
		const { NotFoundError, UnsupportedTypeError } = await import("foliate-js/view.js");
		this.view?.remove();
		this.view = null;
		if (e instanceof MissingFile || e instanceof NotFoundError) {
			this.errorKind = "missing";
			this.errorMessage = "The book file could not be found. It may have been moved or deleted.";
		} else if (e instanceof UnsupportedTypeError) {
			this.errorKind = "unsupported";
			this.errorMessage = "This file type is not supported by the built-in reader.";
		} else {
			this.errorKind = "failed";
			this.errorMessage =
				"This book could not be opened. The file may be damaged, or protected by DRM.";
		}
		this.status = "error";
	}

	#guardResources(book: FoliateBook) {
		book.transformTarget?.addEventListener("data", (e) => {
			const detail = (e as CustomEvent<{ data: unknown }>).detail;
			detail.data = Promise.resolve(detail.data).catch((err: unknown) => {
				console.warn("failed to load resource", err);
				return "";
			});
		});
	}

	#listen(view: View) {
		const on = (name: string, fn: (detail: never) => void) =>
			view.addEventListener(name, (e) => fn((e as CustomEvent).detail as never));
		on("load", (d: { doc: Document; index: number }) => this.#onLoad(d));
		on("relocate", (d: RelocateDetail) => this.#onRelocate(d));
		on("create-overlay", (d: { index: number }) => this.#drawAnnotations(d.index));
		on(
			"draw-annotation",
			(d: { draw: (f: unknown, o: unknown) => void; annotation: { color?: string } }) =>
				d.draw(this.#Overlayer!.highlight, { color: colorValue(d.annotation.color) })
		);
		on("show-annotation", (d: { value: string; range: Range }) => this.#onShowAnnotation(d));
		view.addEventListener("external-link", (e) => {
			e.preventDefault();
			const { href_ } = (e as CustomEvent<{ href_: string }>).detail;
			void this.#openExternal(href_);
		});
		view.addEventListener("scroll", () => this.#dismissFloating(), true);
		view.history.addEventListener("index-change", () => this.#onHistory());
	}

	async #openExternal(href: string) {
		try {
			if (inTauri) {
				const { openUrl } = await import("@tauri-apps/plugin-opener");
				await openUrl(href);
			} else window.open(href, "_blank", "noopener");
		} catch (e) {
			console.error(e);
		}
	}

	#onHistory() {
		const h = this.view?.history;
		clearTimeout(this.#jumpTimer);
		if (!this.#ready || !h?.canGoBack) {
			this.jumpBack = false;
			return;
		}
		this.jumpBack = true;
		this.#jumpTimer = setTimeout(() => (this.jumpBack = false), 12000);
	}

	returnToPrevious() {
		this.view?.history.back();
	}

	applyPrefs(p: Prefs, theme: ThemeName) {
		this.#prefs = p;
		this.#theme = theme;
		this.#apply();
	}

	#apply() {
		const view = this.view;
		const p = this.#prefs;
		if (!view || !p) return;
		const r = view.renderer;
		const c = palettes[this.#theme];
		view.style.setProperty("--overlayer-highlight-opacity", c.highlightOpacity);
		view.style.setProperty("--overlayer-highlight-blend-mode", c.highlightBlend);
		if (this.fixed) {
			view.style.padding = "52px 0 66px";
			r.setAttribute("zoom", p.zoom);
			return;
		}
		r.setStyles?.(buildCss(p, this.#theme));
		r.setAttribute("flow", p.flow);
		r.setAttribute("gap", `${p.margin}%`);
		r.setAttribute("max-inline-size", `${p.lineWidth}px`);
		r.setAttribute("max-column-count", String(p.columns));
	}

	#onLoad({ doc, index }: { doc: Document; index: number }) {
		for (const img of doc.querySelectorAll<HTMLImageElement>("img[srcset]")) {
			if (img.getAttribute("src")?.startsWith("blob:") && !img.srcset.includes("blob:"))
				img.removeAttribute("srcset");
		}
		doc.addEventListener("keydown", (e) => this.onKey(e));
		doc.addEventListener("mousemove", () => this.poke());
		doc.addEventListener("click", (e) => this.#onClick(e, doc));
		doc.addEventListener("dblclick", () => clearTimeout(this.#clickTimer));
		doc.addEventListener("wheel", (e) => this.#onWheel(e), { passive: true });
		if (this.fixed) return;
		let pointerDown = false;
		let timer: ReturnType<typeof setTimeout> | undefined;
		const check = () => this.#checkSelection(doc, index);
		doc.addEventListener("pointerdown", () => {
			pointerDown = true;
			clearTimeout(timer);
			this.selection = null;
		});
		doc.addEventListener("pointerup", () => {
			pointerDown = false;
			clearTimeout(timer);
			timer = setTimeout(check, 30);
		});
		doc.addEventListener("selectionchange", () => {
			if (pointerDown) return;
			clearTimeout(timer);
			timer = setTimeout(check, 350);
		});
	}

	#frameBox(doc: Document, r: DOMRect | DOMRectReadOnly): Box {
		const f = doc.defaultView?.frameElement?.getBoundingClientRect();
		return {
			left: r.left + (f?.left ?? 0),
			top: r.top + (f?.top ?? 0),
			width: r.width,
			height: r.height
		};
	}

	#checkSelection(doc: Document, index: number) {
		const view = this.view;
		const sel = doc.getSelection();
		if (!view || !sel || sel.isCollapsed || !sel.rangeCount) {
			this.selection = null;
			return;
		}
		const range = sel.getRangeAt(0);
		const text = sel.toString().replace(/\s+/g, " ").trim();
		if (!text) {
			this.selection = null;
			return;
		}
		const rects = Array.from(range.getClientRects()).filter((r) => r.width > 1);
		const first = rects[0] ?? range.getBoundingClientRect();
		const bounds = range.getBoundingClientRect();
		const box = this.#frameBox(doc, first);
		box.left = this.#frameBox(doc, bounds).left;
		box.width = bounds.width;
		this.selection = {
			cfi: view.getCFI(index, range),
			text,
			index,
			chapter: view.getProgressOf(index, range)?.tocItem?.label ?? this.place?.chapter ?? "",
			box
		};
		this.editor = null;
	}

	#dismissFloating() {
		this.selection = null;
		this.editor = null;
	}

	clearSelection() {
		this.view?.deselect();
		this.selection = null;
	}

	#onClick(e: MouseEvent, doc: Document) {
		const host = this.#host;
		if (!host || !this.view) return;
		const target = e.target as Element | null;
		if (target?.closest?.("a[href]")) return;
		if (this.editor) {
			this.editor = null;
			return;
		}
		const sel = doc.getSelection();
		if (sel && !sel.isCollapsed) return;

		const frame = doc.defaultView?.frameElement?.getBoundingClientRect();
		const hostBox = host.getBoundingClientRect();
		const x = ((frame?.left ?? 0) + e.clientX - hostBox.left) / hostBox.width;
		const paged = this.fixed || this.#prefs?.flow !== "scrolled";
		const edge = paged && (x < 0.14 || x > 0.86);

		if (target instanceof (doc.defaultView as Window & typeof globalThis).HTMLImageElement) {
			if (!edge && !this.fixed && target.naturalWidth > 120) {
				this.lightbox = target.currentSrc || target.src;
				return;
			}
		}

		setTimeout(() => {
			if (this.#annotationClicked) {
				this.#annotationClicked = false;
				return;
			}
			if (this.panel) return;
			if (edge) {
				if (x < 0.5) void this.view?.goLeft();
				else void this.view?.goRight();
				return;
			}
			clearTimeout(this.#clickTimer);
			this.#clickTimer = setTimeout(() => {
				const s = doc.getSelection();
				if (s && !s.isCollapsed) return;
				this.chromeVisible = !this.chromeVisible;
				if (this.chromeVisible) this.#schedulePoke();
			}, 220);
		}, 0);
	}

	#onWheel(e: WheelEvent) {
		const view = this.view;
		if (!view || this.#wheelLock || this.panel) return;
		if (!this.fixed && this.#prefs?.flow === "scrolled") return;
		if (e.ctrlKey) return;
		if (this.fixed) {
			const r = view.renderer;
			if (r.scrollHeight > r.clientHeight + 2 || r.scrollWidth > r.clientWidth + 2) return;
		}
		const d = Math.abs(e.deltaY) > Math.abs(e.deltaX) ? e.deltaY : e.deltaX;
		if (Math.abs(d) < 24) return;
		this.#wheelLock = true;
		setTimeout(() => (this.#wheelLock = false), 380);
		if (d > 0) void view.next();
		else void view.prev();
	}

	poke() {
		this.chromeVisible = true;
		this.#schedulePoke();
	}

	#schedulePoke() {
		clearTimeout(this.#chromeTimer);
		this.#chromeTimer = setTimeout(() => {
			if (this.chromeHeld || this.panel || this.appearanceOpen || this.selection || this.editor) {
				this.#schedulePoke();
				return;
			}
			this.chromeVisible = false;
		}, 3000);
	}

	onKey(e: KeyboardEvent) {
		const view = this.view;
		const t = e.target as HTMLElement | null;
		if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable)) return;
		if (t?.tagName === "BUTTON" && (e.key === " " || e.key === "Enter")) return;
		if (t?.closest?.("[data-slot=slider], [role=dialog], [role=listbox]")) return;
		if (e.ctrlKey || e.metaKey) {
			if (e.key === "f") {
				e.preventDefault();
				if (this.canSearch) this.openPanel("search");
			} else if (e.key === "d") {
				e.preventDefault();
				void this.toggleBookmark();
			}
			return;
		}
		if (e.altKey || !view) return;
		const key = e.key;
		if (key === "Escape") {
			if (this.lightbox) this.lightbox = null;
			else if (this.editor) this.editor = null;
			else if (this.selection) this.clearSelection();
			else if (this.panel) this.panel = null;
			else if (this.appearanceOpen) this.appearanceOpen = false;
			else if (this.fullscreen) void this.toggleFullscreen();
			return;
		}
		if (this.panel || this.appearanceOpen || this.lightbox) return;
		const scrolled = !this.fixed && this.#prefs?.flow === "scrolled";
		let handled = true;
		if (key === "ArrowLeft") void view.goLeft();
		else if (key === "ArrowRight") void view.goRight();
		else if (key === "PageDown" || (key === " " && !e.shiftKey)) void view.next();
		else if (key === "PageUp" || (key === " " && e.shiftKey)) void view.prev();
		else if (key === "ArrowDown") void (scrolled ? view.next(72) : view.next());
		else if (key === "ArrowUp") void (scrolled ? view.prev(72) : view.prev());
		else if (key === "Home") void view.goToFraction(0);
		else if (key === "End") void view.goToFraction(1);
		else if (key === "t") this.openPanel("toc");
		else if (key === "/" && this.canSearch) this.openPanel("search");
		else if (key === "f") void this.toggleFullscreen();
		else if (key === "b") void this.toggleBookmark();
		else handled = false;
		if (handled) {
			e.preventDefault();
			this.poke();
		}
	}

	openPanel(tab: PanelTab) {
		this.panel = tab;
		this.appearanceOpen = false;
	}

	async toggleFullscreen() {
		try {
			if (inTauri) {
				const { getCurrentWindow } = await import("@tauri-apps/api/window");
				const w = getCurrentWindow();
				const next = !(await w.isFullscreen());
				await w.setFullscreen(next);
				this.fullscreen = next;
			} else if (document.fullscreenElement) {
				await document.exitFullscreen();
				this.fullscreen = false;
			} else {
				await document.documentElement.requestFullscreen();
				this.fullscreen = true;
			}
		} catch (e) {
			console.error(e);
		}
	}

	#onRelocate(d: RelocateDetail) {
		const view = this.view;
		if (!view) return;
		if (d.reason !== "selection") this.#dismissFloating();
		let fraction = d.fraction ?? 0;
		let page: Place["page"] = null;
		if (this.fixed && d.section) {
			page = { current: d.section.current + 1, total: d.section.total };
			fraction = page.current / page.total;
		}
		this.place = {
			fraction: Math.min(1, Math.max(0, fraction)),
			chapter: d.tocItem?.label?.trim() ?? "",
			tocHref: d.tocItem?.href ?? null,
			minutesLeft: this.fixed ? null : (d.time?.total ?? null),
			chapterMinutesLeft: this.fixed ? null : (d.time?.section ?? null),
			page
		};
		this.#updateFeet();
		if (!this.#ready || d.cfi === this.#loadedPosition) return;
		this.#pending = { position: d.cfi, progress: this.place.fraction };
		clearTimeout(this.#saveTimer);
		this.#saveTimer = setTimeout(() => void this.flush(), 1000);
	}

	#updateFeet() {
		const feet = this.view?.renderer.feet;
		const place = this.place;
		if (!feet?.length || !place) return;
		const percent = `${Math.round(place.fraction * 100)}%`;
		for (const f of feet) f.textContent = "";
		if (feet.length === 1) feet[0].textContent = [place.chapter, percent].filter(Boolean).join("  ·  ");
		else {
			feet[0].textContent = place.chapter;
			feet[feet.length - 1].textContent = percent;
		}
	}

	async flush() {
		clearTimeout(this.#saveTimer);
		const p = this.#pending;
		if (!p) return;
		this.#pending = null;
		this.#loadedPosition = p.position;
		await data.saveState({
			bookId: this.id,
			format: this.format,
			position: p.position,
			progress: p.progress,
			updated: ""
		});
	}

	#isHere(position: string): boolean {
		const loc = this.view?.lastLocation;
		const cfi = this.#cfi;
		if (!loc || !cfi) return false;
		if (this.fixed) return position === loc.cfi;
		try {
			const start = cfi.collapse(position);
			return (
				cfi.compare(start, cfi.collapse(loc.cfi)) >= 0 &&
				cfi.compare(start, cfi.collapse(loc.cfi, true)) < 0
			);
		} catch {
			return false;
		}
	}

	#indexOf(a: Annotation): number {
		let i = this.#indexCache.get(a.id);
		if (i === undefined) {
			try {
				const r = this.view?.resolveNavigation(a.position);
				i = r && !(r instanceof Promise) ? r.index : -1;
			} catch {
				i = -1;
			}
			this.#indexCache.set(a.id, i);
		}
		return i;
	}

	#drawAnnotations(index: number) {
		if (this.fixed) return;
		for (const a of this.annotations)
			if (a.kind === "highlight" && this.#indexOf(a) === index) this.#draw(a);
	}

	#draw(a: Annotation) {
		this.view?.addAnnotation({ value: a.position, color: a.color ?? "yellow" }).catch(() => {});
	}

	#onShowAnnotation({ value, range }: { value: string; range: Range }) {
		const a = this.annotations.find((x) => x.position === value);
		if (!a) return;
		this.#annotationClicked = true;
		const doc = range.startContainer.ownerDocument;
		if (!doc) return;
		const rects = Array.from(range.getClientRects()).filter((r) => r.width > 1);
		const first = rects[0] ?? range.getBoundingClientRect();
		const box = this.#frameBox(doc, first);
		const bounds = this.#frameBox(doc, range.getBoundingClientRect());
		box.left = bounds.left;
		box.width = bounds.width;
		this.selection = null;
		this.editor = { id: a.id, box };
	}

	async createHighlight(color: string, withNote = false) {
		const sel = this.selection;
		if (!sel) return;
		const now = new Date().toISOString();
		const a: Annotation = {
			id: crypto.randomUUID(),
			bookId: this.id,
			kind: "highlight",
			position: sel.cfi,
			text: sel.text,
			note: null,
			color,
			chapter: sel.chapter || null,
			created: now,
			updated: now
		};
		this.annotations = [...this.annotations, a];
		this.#indexCache.set(a.id, sel.index);
		this.#draw(a);
		this.clearSelection();
		await data.saveAnnotation(a);
		if (withNote) this.editor = { id: a.id, box: sel.box };
	}

	async updateAnnotation(id: string, patch: Partial<Pick<Annotation, "note" | "color">>) {
		const a = this.annotations.find((x) => x.id === id);
		if (!a) return;
		const next: Annotation = { ...a, ...patch, updated: new Date().toISOString() };
		this.annotations = this.annotations.map((x) => (x.id === id ? next : x));
		if (patch.color && next.kind === "highlight") this.#draw(next);
		await data.saveAnnotation(next);
	}

	async removeAnnotation(id: string) {
		const a = this.annotations.find((x) => x.id === id);
		if (!a) return;
		this.annotations = this.annotations.filter((x) => x.id !== id);
		if (this.editor?.id === id) this.editor = null;
		if (a.kind === "highlight") void this.view?.deleteAnnotation({ value: a.position });
		await data.deleteAnnotation(a);
	}

	async toggleBookmark() {
		const view = this.view;
		const loc = view?.lastLocation;
		if (!view || !loc) return;
		const existing = this.annotations.filter(
			(a) => a.kind === "bookmark" && this.#isHere(a.position)
		);
		if (existing.length) {
			for (const a of existing) await this.removeAnnotation(a.id);
			return;
		}
		const now = new Date().toISOString();
		const text = loc.range?.toString().replace(/\s+/g, " ").trim().slice(0, 160) ?? "";
		const a: Annotation = {
			id: crypto.randomUUID(),
			bookId: this.id,
			kind: "bookmark",
			position: loc.cfi,
			text: text || null,
			note: null,
			color: null,
			chapter: loc.tocItem?.label?.trim() || null,
			created: now,
			updated: now
		};
		this.annotations = [...this.annotations, a];
		await data.saveAnnotation(a);
	}

	get sortedAnnotations() {
		const cfi = this.#cfi;
		const list = [...this.annotations];
		if (!cfi) return list;
		return list.sort((a, b) => {
			try {
				return cfi.compare(a.position, b.position);
			} catch {
				return 0;
			}
		});
	}

	async goToAnnotation(a: Annotation) {
		this.panel = null;
		await this.view?.goTo(a.position);
	}

	goToToc(href: string) {
		this.panel = null;
		void this.view?.goTo(href);
	}

	goToFraction(f: number) {
		void this.view?.goToFraction(f);
	}

	goToPage(page: number) {
		void this.view?.goTo(page - 1);
	}

	queueSearch(query: string) {
		this.search.query = query;
		clearTimeout(this.#searchTimer);
		if (query.trim().length < 2) {
			if (!query.trim()) this.clearSearch(true);
			return;
		}
		this.#searchTimer = setTimeout(() => void this.runSearch(), 450);
	}

	async runSearch() {
		const view = this.view;
		const query = this.search.query.trim();
		clearTimeout(this.#searchTimer);
		if (!view || !query) return;
		const run = ++this.#searchRun;
		view.clearSearch();
		Object.assign(this.search, {
			running: true,
			progress: 0,
			done: false,
			total: 0,
			active: -1,
			groups: []
		});
		const groups: SearchGroup[] = [];
		let total = 0;
		try {
			for await (const r of view.search({
				query,
				draw: this.#Overlayer!.highlight as never,
				drawOptions: { color: "#f59e0b" }
			})) {
				if (run !== this.#searchRun) return;
				if (r === "done") break;
				if ("progress" in r) this.search.progress = r.progress;
				else {
					groups.push({ label: r.label, hits: r.subitems });
					total += r.subitems.length;
					this.search.groups = [...groups];
					this.search.total = total;
					if (total >= MAX_SEARCH_HITS) break;
				}
			}
		} catch (e) {
			console.error(e);
		}
		if (run === this.#searchRun) Object.assign(this.search, { running: false, done: true });
	}

	async goToHit(i: number) {
		const hits = this.searchHits;
		if (i < 0 || i >= hits.length) return;
		this.search.active = i;
		await this.view?.goTo(hits[i].cfi);
	}

	clearSearch(keepQuery = false) {
		this.#searchRun++;
		clearTimeout(this.#searchTimer);
		this.view?.clearSearch();
		Object.assign(this.search, {
			query: keepQuery ? this.search.query : "",
			running: false,
			progress: 0,
			done: false,
			total: 0,
			active: -1,
			groups: []
		});
	}

	async openExternally() {
		await api.openExternally(this.id, this.format);
	}

	destroy() {
		this.#destroyed = true;
		void this.flush();
		clearTimeout(this.#chromeTimer);
		clearTimeout(this.#clickTimer);
		clearTimeout(this.#jumpTimer);
		clearTimeout(this.#searchTimer);
		const view = this.view;
		this.view = null;
		if (view) {
			try {
				view.book?.destroy?.();
				view.close();
			} catch (e) {
				console.warn(e);
			}
			view.remove();
		}
	}
}

class MissingFile extends Error {}
