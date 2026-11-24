declare module "foliate-js/view.js" {
	export interface TocItem {
		id?: number;
		label: string;
		href: string;
		subitems?: TocItem[] | null;
	}

	export interface RelocateDetail {
		fraction?: number;
		section?: { current: number; total: number };
		location?: { current: number; next: number; total: number };
		time?: { section: number; total: number };
		tocItem?: TocItem | null;
		pageItem?: TocItem | null;
		cfi: string;
		range: Range | null;
		reason?: string;
	}

	export interface SearchExcerpt {
		pre: string;
		match: string;
		post: string;
	}

	export interface SearchHit {
		cfi: string;
		excerpt: SearchExcerpt;
	}

	export type SearchEvent =
		| "done"
		| { progress: number }
		| { label: string; subitems: SearchHit[] };

	export interface SearchOptions {
		query: string;
		index?: number;
		matchCase?: boolean;
		matchDiacritics?: boolean;
		matchWholeWords?: boolean;
		draw?: (rects: DOMRectList, options?: unknown) => SVGElement;
		drawOptions?: Record<string, unknown>;
	}

	export interface Resolved {
		index: number;
		anchor?: (doc: Document) => Range | Element;
	}

	export interface Annotation {
		value: string;
		color?: string;
	}

	export interface FoliateRenderer extends HTMLElement {
		setStyles?(css: string | string[]): void;
		next(distance?: number): Promise<void>;
		prev(distance?: number): Promise<void>;
		scrollBy?(dx: number, dy: number): void;
		firstSection?(): Promise<void>;
		lastSection?(): Promise<void>;
		getContents(): { index: number; doc: Document; overlayer?: unknown }[];
		scrolled?: boolean;
		atStart?: boolean;
		atEnd?: boolean;
		heads?: HTMLElement[] | null;
		feet?: HTMLElement[] | null;
	}

	export interface FoliateBook {
		metadata?: {
			title?: string | Record<string, string>;
			author?: unknown;
			language?: string | string[];
			publisher?: unknown;
			description?: string;
		};
		toc?: TocItem[];
		sections: {
			id: unknown;
			size: number;
			linear?: string;
			createDocument?: () => Promise<Document>;
		}[];
		dir?: string;
		rendition?: { layout?: string; spread?: string };
		getCover?: () => Promise<Blob | string | null>;
		destroy?: () => void;
		transformTarget?: EventTarget;
	}

	export class View extends HTMLElement {
		book: FoliateBook;
		renderer: FoliateRenderer;
		isFixedLayout: boolean;
		lastLocation: RelocateDetail | null;
		history: EventTarget & {
			canGoBack: boolean;
			canGoForward: boolean;
			back(): void;
			forward(): void;
		};
		open(book: File | Blob | string | FoliateBook): Promise<void>;
		close(): void;
		init(opts: { lastLocation?: string | null; showTextStart?: boolean }): Promise<void>;
		goTo(target: string | number): Promise<Resolved | undefined>;
		goToFraction(fraction: number): Promise<void>;
		goToTextStart(): Promise<void>;
		prev(distance?: number): Promise<void>;
		next(distance?: number): Promise<void>;
		goLeft(): Promise<void>;
		goRight(): Promise<void>;
		select(target: string): Promise<void>;
		deselect(): void;
		getSectionFractions(): number[];
		getProgressOf(index: number, range?: Range | null): { tocItem?: TocItem | null; pageItem?: TocItem | null };
		getCFI(index: number, range?: Range | null): string;
		resolveNavigation(target: string | number | { fraction: number }): Resolved | undefined;
		addAnnotation(annotation: Annotation, remove?: boolean): Promise<{ index: number; label: string } | undefined>;
		deleteAnnotation(annotation: Annotation): Promise<unknown>;
		showAnnotation(annotation: Annotation): Promise<void>;
		search(opts: SearchOptions): AsyncGenerator<SearchEvent>;
		clearSearch(): void;
	}

	export function makeBook(file: File | Blob | string): Promise<FoliateBook>;

	export class ResponseError extends Error {}
	export class NotFoundError extends Error {}
	export class UnsupportedTypeError extends Error {}
}

declare module "foliate-js/overlayer.js" {
	export class Overlayer {
		static highlight(rects: DOMRectList, options?: { color?: string }): SVGElement;
		static outline(rects: DOMRectList, options?: { color?: string; width?: number; radius?: number }): SVGElement;
		static underline(rects: DOMRectList, options?: { color?: string; width?: number }): SVGElement;
	}
}

declare module "foliate-js/epubcfi.js" {
	export const isCFI: RegExp;
	export function compare(a: string, b: string): number;
	export function collapse(cfi: string, toEnd?: boolean): string;
}
