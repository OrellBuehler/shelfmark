import { convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (inTauri) return tauriInvoke<T>(cmd, args);
	const { mockInvoke } = await import("./mock");
	return (await mockInvoke(cmd, args)) as T;
}

export type BookId = number;

export interface Metadata {
	title: string | null;
	titleSort: string | null;
	authors: string[];
	authorSort: string | null;
	series: string | null;
	seriesIndex: number | null;
	tags: string[];
	publisher: string | null;
	pubdate: string | null;
	languages: string[];
	identifiers: Record<string, string>;
	rating: number | null;
	comments: string | null;
	uuid: string | null;
}

export interface BookSummary {
	id: BookId;
	title: string;
	sort: string;
	authors: string[];
	authorSort: string;
	series: string | null;
	seriesIndex: number;
	tags: string[];
	publisher: string | null;
	languages: string[];
	rating: number | null;
	formats: string[];
	identifiers: Record<string, string>;
	hasCover: boolean;
	size: number;
	timestamp: string;
	pubdate: string | null;
	lastModified: string;
}

export interface FormatFile {
	format: string;
	fileName: string;
	size: number;
}

export interface Book extends BookSummary {
	comments: string | null;
	uuid: string;
	path: string;
	files: FormatFile[];
}

export type SortField =
	| "title"
	| "author"
	| "series"
	| "added"
	| "published"
	| "modified"
	| "rating"
	| "size"
	| "publisher";

export interface ListQuery {
	search?: string | null;
	sortBy?: SortField;
	descending?: boolean;
}

export interface BookPatch {
	title?: string;
	titleSort?: string;
	authors?: string[];
	authorSort?: string;
	series?: string;
	seriesIndex?: number;
	tags?: string[];
	publisher?: string;
	pubdate?: string;
	languages?: string[];
	identifiers?: Record<string, string>;
	rating?: number;
	comments?: string;
}

export interface BulkPatch {
	authors?: string[];
	series?: string;
	seriesStart?: number;
	addTags?: string[];
	removeTags?: string[];
	clearTags?: boolean;
	publisher?: string;
	pubdate?: string;
	languages?: string[];
	rating?: number;
}

export interface CategoryItem {
	id: number | null;
	name: string;
	count: number;
	search: string;
}

export interface Category {
	key: string;
	items: CategoryItem[];
}

export interface SavedSearch {
	name: string;
	query: string;
}

export interface LibraryInfo {
	path: string;
	name: string;
	bookCount: number;
}

export interface AddOutcome {
	path: string;
	bookId: BookId | null;
	error: string | null;
}

export interface IdentifyQuery {
	title?: string | null;
	authors?: string[];
	isbn?: string | null;
}

export interface SourceResult {
	source: string;
	metadata: Metadata;
	coverUrl: string | null;
	url: string | null;
}

export type DeviceKind = "kobo" | "kindle" | "pocketBook" | "tolino" | "generic" | "folder";

export interface Device {
	id: string;
	name: string;
	kind: DeviceKind;
	root: string;
	booksDir: string;
	totalBytes: number;
	freeBytes: number;
	supportedFormats: string[];
	info: DeviceInfo[];
}

export interface DeviceInfo {
	label: string;
	value: string;
}

export interface DeviceBook {
	path: string;
	relPath: string;
	title: string;
	authors: string[];
	format: string;
	size: number;
	modified: string | null;
	uuid: string | null;
}

export interface SendOutcome {
	bookId: BookId;
	book: DeviceBook | null;
	converted: boolean;
	error: string | null;
}

export interface ReadingState {
	bookId: BookId;
	format: string;
	position: string | null;
	progress: number;
	updated: string;
}

export type AnnotationKind = "highlight" | "bookmark";

export interface Annotation {
	id: string;
	bookId: BookId;
	kind: AnnotationKind;
	position: string;
	text: string | null;
	note: string | null;
	color: string | null;
	chapter: string | null;
	created: string;
	updated: string;
}

export const api = {
	currentLibrary: () => invoke<LibraryInfo | null>("current_library"),
	recentLibraries: () => invoke<LibraryInfo[]>("recent_libraries"),
	openLibrary: (path: string) => invoke<LibraryInfo>("open_library", { path }),
	createLibrary: (path: string) => invoke<LibraryInfo>("create_library", { path }),

	listBooks: (query: ListQuery = {}) => invoke<BookSummary[]>("list_books", { query }),
	getBook: (id: BookId) => invoke<Book>("get_book", { id }),
	categories: (search?: string | null) =>
		invoke<Category[]>("get_categories", { search: search ?? null }),
	completions: (field: "authors" | "series" | "tags" | "publisher" | "languages") =>
		invoke<string[]>("completions", { field }),
	savedSearches: () => invoke<SavedSearch[]>("saved_searches"),
	setSavedSearches: (searches: SavedSearch[]) => invoke<void>("set_saved_searches", { searches }),

	updateBook: (id: BookId, patch: BookPatch) => invoke<Book>("update_book", { id, patch }),
	bulkUpdate: (ids: BookId[], patch: BulkPatch) => invoke<void>("bulk_update", { ids, patch }),
	setCoverFromFile: (id: BookId, path: string) => invoke<void>("set_cover_from_file", { id, path }),
	setCoverFromUrl: (id: BookId, url: string) => invoke<void>("set_cover_from_url", { id, url }),
	removeCover: (id: BookId) => invoke<void>("remove_cover", { id }),
	embedMetadata: (ids: BookId[]) => invoke<number>("embed_metadata", { ids }),

	addBooks: (paths: string[]) => invoke<AddOutcome[]>("add_books", { paths }),
	addFormat: (id: BookId, path: string) => invoke<Book>("add_format", { id, path }),
	removeFormat: (id: BookId, format: string) => invoke<Book>("remove_format", { id, format }),
	deleteBooks: (ids: BookId[]) => invoke<void>("delete_books", { ids }),
	showInFolder: (id: BookId) => invoke<void>("show_in_folder", { id }),
	openExternally: (id: BookId, format: string) => invoke<void>("open_externally", { id, format }),

	identify: (query: IdentifyQuery) => invoke<SourceResult[]>("identify", { query }),
	bestMetadata: (id: BookId) => invoke<SourceResult | null>("best_metadata", { id }),

	detectDevices: () => invoke<Device[]>("detect_devices"),
	addFolderDevice: (path: string) => invoke<Device>("add_folder_device", { path }),
	deviceBooks: (deviceId: string) => invoke<DeviceBook[]>("device_books", { deviceId }),
	sendToDevice: (deviceId: string, ids: BookId[]) =>
		invoke<SendOutcome[]>("send_to_device", { deviceId, ids }),
	importFromDevice: (deviceId: string, paths: string[]) =>
		invoke<AddOutcome[]>("import_from_device", { deviceId, paths }),
	deleteDeviceBook: (deviceId: string, path: string) =>
		invoke<void>("delete_device_book", { deviceId, path }),

	readingProgress: () => invoke<Record<string, number>>("reading_progress"),
	readingState: (id: BookId) => invoke<ReadingState | null>("reading_state", { id }),
	setReadingState: (state: ReadingState) => invoke<void>("set_reading_state", { state }),
	annotations: (id: BookId) => invoke<Annotation[]>("annotations", { id }),
	saveAnnotation: (annotation: Annotation) => invoke<void>("save_annotation", { annotation }),
	deleteAnnotation: (id: string) => invoke<void>("delete_annotation", { id })
};

export function coverUrl(book: Pick<BookSummary, "id" | "lastModified">, width = 400) {
	return `${convertFileSrc(String(book.id), "cover")}?w=${width}&v=${encodeURIComponent(book.lastModified)}`;
}

export function bookFileUrl(id: BookId, format: string) {
	return convertFileSrc(`${id}/${format}`, "bookfile");
}
