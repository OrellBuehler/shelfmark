import { Channel, convertFileSrc, invoke as tauriInvoke } from "@tauri-apps/api/core";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
	if (inTauri) return tauriInvoke<T>(cmd, args);
	const { mockInvoke } = await import("./mock");
	return (await mockInvoke(cmd, args)) as T;
}

export type BookId = number;
export type AudiobookId = number;

export type Scope = "all" | "ebooks" | "audiobooks";
export type EntryKind = "ebook" | "audiobook" | "both";
export type AudioState = "present" | "missing" | "remote";
export type AudioLayout = "libation" | "author-title";

export interface AudioBrief {
	narrators: string[];
	durationMs: number;
	format: string;
	fileCount: number;
	size: number;
	asin: string | null;
	state: AudioState;
	count: number;
	hasCover: boolean;
}

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
	kind: EntryKind;
	audiobookId: AudiobookId | null;
	audio: AudioBrief | null;
	pairedBookId: BookId | null;
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
	| "publisher"
	| "duration";

export interface ListQuery {
	search?: string | null;
	sortBy?: SortField;
	descending?: boolean;
	scope?: Scope;
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

export interface AudioRoot {
	id: number;
	path: string;
	inLibrary: boolean;
	managed: boolean;
	layout: AudioLayout;
	label: string | null;
	enabled: boolean;
	lastScan: string | null;
	lastScanOk: boolean | null;
	audiobooks: number;
	missing: number;
}

export interface AudioIssue {
	path: string;
	message: string;
}

export interface AudioStatus {
	roots: AudioRoot[];
	audiobooks: number;
	issues: AudioIssue[];
}

export interface RootPatch {
	path?: string;
	managed?: boolean;
	layout?: AudioLayout;
	label?: string;
	enabled?: boolean;
}

export interface ScanSummary {
	audiobooks: number;
	added: number;
	updated: number;
	unchanged: number;
	missing: number;
	issues: number;
	complete: boolean;
}

export interface TransferTarget {
	destRootId?: number;
	destFolder?: string;
}

export interface TransferOptions extends TransferTarget {
	mode: "copy" | "move";
	verify: "quick" | "full";
}

export interface TransferPlanItem {
	audiobookId: AudiobookId;
	title: string;
	destDir: string;
	key: string;
	files: { src: string; dest: string; size: number }[];
	bytes: number;
	skip: string | null;
}

export interface TransferPlan {
	items: TransferPlanItem[];
	totalBytes: number;
	freeBytes: number | null;
	enoughSpace: boolean;
	destPath: string;
}

export interface TransferNote {
	audiobookId: AudiobookId;
	title: string;
	message: string;
}

export interface TransferResult {
	done: number;
	skipped: TransferNote[];
	failed: TransferNote[];
	bytes: number;
	cancelled: boolean;
}

export type JobEvent<R = ScanSummary> =
	| { event: "progress"; phase: "walking" | "reading" | "copying" | "done"; done: number; total: number; current: string | null }
	| { event: "done"; result: R }
	| { event: "failed"; message: string }
	| { event: "cancelled" };

export type JobProgress = Extract<JobEvent, { event: "progress" }>;
export type JobEnd<R = ScanSummary> = Exclude<JobEvent<R>, { event: "progress" }>;

export interface AudiobookFile {
	relPath: string;
	kind: "audio" | "cover" | "pdf" | "cue" | "meta";
	size: number;
	durationMs: number | null;
	track: number | null;
	title: string | null;
}

export interface AudiobookChapter {
	title: string;
	startMs: number;
}

export interface AudiobookInfo {
	id: AudiobookId;
	rootId: number | null;
	state: AudioState;
	title: string;
	subtitle: string | null;
	authors: string[];
	narrators: string[];
	series: string | null;
	seriesIndex: number | null;
	publisher: string | null;
	language: string | null;
	genres: string[];
	description: string | null;
	asin: string | null;
	isbn: string | null;
	releaseDate: string | null;
	durationMs: number;
	size: number;
	format: string;
	fileCount: number;
	hasCover: boolean;
	added: string;
	modified: string;
	dir: string | null;
	files: AudiobookFile[];
	chapters: AudiobookChapter[];
	location: string | null;
	link: { book: BookSummary; manual: boolean } | null;
	candidates: { book: BookSummary; score: number }[];
}

export function isAudioEntry(b: Pick<BookSummary, "kind" | "pairedBookId">) {
	return b.kind === "audiobook" || b.pairedBookId !== null;
}

export function runJob<R = ScanSummary>(cmd: string, args: Record<string, unknown>, onProgress: (p: JobProgress) => void) {
	let id: number | null = null;
	let cancelled = false;
	const result = new Promise<JobEnd<R>>((resolve, reject) => {
		const onEvent = (e: JobEvent<R>) => {
			if (e.event === "progress") onProgress(e);
			else resolve(e);
		};
		let channel: Channel<JobEvent<R>> | ((e: JobEvent<R>) => void) = onEvent;
		if (inTauri) {
			channel = new Channel<JobEvent<R>>();
			channel.onmessage = onEvent;
		}
		invoke<number>(cmd, { ...args, onEvent: channel }).then((started) => {
			id = started;
			if (cancelled) void invoke("job_cancel", { id });
		}, reject);
	});
	return {
		result,
		cancel() {
			if (cancelled) return;
			cancelled = true;
			if (id !== null) void invoke("job_cancel", { id });
		}
	};
}

export const api = {
	currentLibrary: () => invoke<LibraryInfo | null>("current_library"),
	recentLibraries: () => invoke<LibraryInfo[]>("recent_libraries"),
	openLibrary: (path: string) => invoke<LibraryInfo>("open_library", { path }),
	createLibrary: (path: string) => invoke<LibraryInfo>("create_library", { path }),

	listBooks: (query: ListQuery = {}) => invoke<BookSummary[]>("list_books", { query }),
	getBook: (id: BookId) => invoke<Book>("get_book", { id }),
	categories: (search?: string | null, scope?: Scope) =>
		invoke<Category[]>("get_categories", { search: search ?? null, scope: scope ?? null }),
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
	deleteAnnotation: (id: string) => invoke<void>("delete_annotation", { id }),

	audioStatus: () => invoke<AudioStatus>("audio_status"),
	audioAddRoot: (inLibrary: boolean, path?: string) =>
		invoke<AudioRoot>("audio_add_root", { path: path ?? null, inLibrary }),
	audioUpdateRoot: (id: number, patch: RootPatch) => invoke<AudioRoot>("audio_update_root", { id, patch }),
	audioRemoveRoot: (id: number) => invoke<void>("audio_remove_root", { id }),
	audioForgetMissing: (id: number) => invoke<number>("audio_forget_missing", { id }),
	audioScan: (rootIds: number[] | null, onProgress: (p: JobProgress) => void) =>
		runJob("audio_scan", { rootIds }, onProgress),
	audiobook: (id: AudiobookId) => invoke<AudiobookInfo>("audiobook", { id }),
	audioLink: (audiobookId: AudiobookId, bookId: BookId) => invoke<void>("audio_link", { audiobookId, bookId }),
	audioUnlink: (audiobookId: AudiobookId, bookId: BookId) => invoke<void>("audio_unlink", { audiobookId, bookId }),
	audioResetLink: (audiobookId: AudiobookId) => invoke<void>("audio_reset_link", { audiobookId }),
	audioTransferPlan: (ids: AudiobookId[], target: TransferTarget) =>
		invoke<TransferPlan>("audio_transfer_plan", {
			ids,
			destRootId: target.destRootId ?? null,
			destFolder: target.destFolder ?? null
		}),
	audioTransfer: (ids: AudiobookId[], options: TransferOptions, onProgress: (p: JobProgress) => void) =>
		runJob<TransferResult>(
			"audio_transfer",
			{
				ids,
				destRootId: options.destRootId ?? null,
				destFolder: options.destFolder ?? null,
				mode: options.mode,
				verify: options.verify
			},
			onProgress
		),
	audioShowInFolder: (id: AudiobookId) => invoke<void>("audio_show_in_folder", { id }),
	audioOpen: (id: AudiobookId) => invoke<void>("audio_open", { id })
};

export type CoverRef = Pick<BookSummary, "id" | "lastModified" | "hasCover" | "kind" | "audiobookId" | "pairedBookId" | "audio">;

export function showsAudioCover(book: CoverRef) {
	if (book.audiobookId === null) return false;
	return isAudioEntry(book) ? book.hasCover : !book.hasCover && !!book.audio?.hasCover;
}

export function entryHasCover(book: CoverRef) {
	return book.hasCover || showsAudioCover(book);
}

export function coverUrl(book: CoverRef, width = 400) {
	const path = showsAudioCover(book) ? `a/${book.audiobookId}` : String(book.id);
	return `${convertFileSrc(path, "cover")}?w=${width}&v=${encodeURIComponent(book.lastModified)}`;
}

export function bookFileUrl(id: BookId, format: string) {
	return convertFileSrc(`${id}/${format}`, "bookfile");
}
