import type {
	AddOutcome,
	Annotation,
	AudioBrief,
	AudioIssue,
	AudioRoot,
	AudiobookFile,
	AudiobookInfo,
	Book,
	BookPatch,
	BookSummary,
	BulkPatch,
	Category,
	CategoryItem,
	Device,
	DeviceBook,
	IdentifyQuery,
	JobEvent,
	LibraryInfo,
	ListQuery,
	ReadingState,
	RootPatch,
	SavedSearch,
	ScanSummary,
	Scope,
	SendOutcome,
	SortField,
	SourceResult,
	TransferPlan,
	TransferPlanItem,
	TransferResult
} from "./api";
import { generatedCover } from "./covers";
import { EBOOK_EXTENSIONS, baseName, hashString } from "./format";

type MockBook = Book & { hay: string };

const EBOOK_ONLY = { kind: "ebook", audiobookId: null, audio: null, pairedBookId: null } as const;

function mulberry32(seed: number) {
	let a = seed >>> 0;
	return () => {
		a = (a + 0x6d2b79f5) >>> 0;
		let t = a;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

const ADJ = ["Silent", "Crimson", "Forgotten", "Hollow", "Gilded", "Distant", "Winter", "Broken", "Quiet", "Burning", "Wandering", "Pale", "Endless", "Secret", "Salted", "Velvet", "Iron", "Fading", "Restless", "Hidden", "Golden", "Bitter", "Paper", "Amber", "Midnight", "Lonely", "Wild", "Ashen", "Clockwork", "Tidal", "Northern", "Scarlet", "Brittle", "Luminous", "Narrow", "Sunken", "Thorned", "Ivory", "Starlit", "Drowned"];
const NOUN = ["Garden", "Lighthouse", "Archive", "Compass", "Orchard", "Harbor", "Cartographer", "Window", "Letters", "River", "Mirror", "Library", "Engine", "Season", "Bridge", "Station", "Orchestra", "Lantern", "Tide", "Atlas", "Clocktower", "Meridian", "Almanac", "Violin", "Cathedral", "Voyage", "Inheritance", "Equation", "Shore", "Menagerie", "Telegraph", "Cipher", "Pilgrim", "Thread", "Horizon", "Glasshouse", "Kingdom", "Ledger", "Fable", "Rain"];
const PLACE = ["Brannock", "Valmere", "the North", "Eastholt", "Orsenna", "Kellwater", "Dunmarrow", "Ashford Reach", "Tessaly", "Marrow Bay", "Lindqvist", "Corvane", "the Low Country", "Hadleigh", "Ostrava Falls", "Pellam", "Winterhaven", "Sunder"];
const FIRST = ["Alma", "Tobias", "Ingrid", "Marek", "Odile", "Casimir", "Wren", "Leona", "Anselm", "Priya", "Joaquin", "Hana", "Silas", "Imogen", "Dmitri", "Nadia", "Emeric", "Thea", "Rafael", "Yuki", "Beatrix", "Lucan", "Soren", "Mireille", "Okonkwo", "Carys", "Ewan", "Zofia", "Idris", "Maren", "Teodor", "Linnea", "Callum", "Rosalind", "Anouk", "Gideon"];
const LAST = ["Vandermeer", "Okafor", "Lindgren", "Halloran", "Castellan", "Voss", "Aldous", "Brennick", "Tanaka-Reyes", "Quill", "Mirande", "Fairweather", "Strand", "Kowalczyk", "Ashdown", "Delacroix", "Rask", "Penhallow", "Navarro", "Thorne", "Abara", "Winslow", "Harkness", "Ferreira", "Lund", "Marchetti", "Oyelaran", "Crane", "Sorensen", "Whitlock", "Bellamy", "Ito", "Hargreave", "Zelenka"];
const TAGS = ["Fantasy", "Science fiction", "Mystery", "Thriller", "Romance", "Historical", "Non-fiction", "Biography", "Science", "Philosophy", "Poetry", "Horror", "Young adult", "Classics", "Travel", "Essays", "Humor", "Technology", "Cooking", "History", "Adventure", "Crime", "Drama", "Graphic novel"];
const PUBLISHERS = ["Lantern & Quill", "Northgate Press", "Ember House", "Harbor Lane Books", "Saltmarsh Editions", "Fernwood", "Copperleaf Publishing", "Meridian Books", "Quiet Hours Press", "Blackthorn & Daughters", "Open Field", "Tidewater Press", "Greywing", "Lumen House", "Paperboat", "Old Mill Books", "Kestrel", "Longview", "Hollis & Reed", "Verso Lane"];
const LANGS = ["eng", "eng", "eng", "eng", "eng", "eng", "eng", "deu", "fra", "spa", "ita", "jpn", "nld"];
const SENT = [
	"Set against a landscape that remembers more than its people do, this is a story about {n1} and the cost of keeping a promise.",
	"When a letter arrives decades late, the town of {p} is forced to reckon with what lies beneath its {n1}.",
	"A sweeping, tender novel that follows three generations as they argue over the {n1} and what it was worth.",
	"Part memoir, part field guide, it traces the history of the {n1} from the first surveys to the last.",
	"With wit and a steady hand, the author unpicks how the {n1} came to shape every decision made in {p}.",
	"Readers will find dry humour, an unreliable narrator and a mystery that refuses to resolve itself neatly.",
	"Spare, luminous prose carries a story of departure, return, and the quiet work of repair.",
	"An essential account, drawing on newly opened archives and the voices of those who were there."
];

const NOW = Date.UTC(2026, 10, 24);

function params() {
	const sp = new URLSearchParams(typeof location === "undefined" ? "" : location.search);
	return {
		noLibrary: sp.has("nolib"),
		empty: sp.has("empty"),
		audio: sp.has("audio"),
		count: Number(sp.get("n")) || 2000
	};
}

function pick<T>(r: () => number, a: T[]): T {
	return a[Math.floor(r() * a.length)];
}

function makeHay(b: Book) {
	return [b.title, b.authors.join(" "), b.series ?? "", b.tags.join(" "), b.publisher ?? "", b.comments ?? "", Object.values(b.identifiers).join(" ")]
		.join("\n")
		.toLowerCase();
}

function sortKey(title: string) {
	return title.replace(/^(the|a|an)\s+/i, "").toLowerCase();
}

function generate(count: number, seed: number): MockBook[] {
	const r = mulberry32(seed);
	const authors = Array.from({ length: Math.max(8, Math.floor(count / 2.4)) }, () => `${pick(r, FIRST)} ${pick(r, LAST)}`);
	const seriesCount = Math.floor(count / 14);
	const series = Array.from({ length: seriesCount }, () => ({
		name: pick(r, [() => `The ${pick(r, PLACE)} Chronicles`, () => `${pick(r, ADJ)} ${pick(r, NOUN)} Cycle`, () => `Tales of ${pick(r, PLACE)}`, () => `The ${pick(r, NOUN)} Quartet`])(),
		author: pick(r, authors),
		tag: pick(r, TAGS),
		publisher: pick(r, PUBLISHERS),
		lang: pick(r, LANGS),
		next: 1
	}));
	const books: MockBook[] = [];
	const seen = new Set<string>();
	for (let i = 1; i <= count; i++) {
		let title: string;
		do {
			title = pick(r, [
				() => `The ${pick(r, ADJ)} ${pick(r, NOUN)}`,
				() => `${pick(r, NOUN)} of ${pick(r, PLACE)}`,
				() => `A ${pick(r, ADJ)} ${pick(r, NOUN)}`,
				() => `The ${pick(r, NOUN)}'s ${pick(r, NOUN)}`,
				() => `${pick(r, ADJ)} ${pick(r, NOUN)}s`,
				() => `Last ${pick(r, NOUN)} in ${pick(r, PLACE)}`,
				() => `${pick(r, FIRST)} and the ${pick(r, ADJ)} ${pick(r, NOUN)}`,
				() => `The ${pick(r, ADJ)} ${pick(r, NOUN)}: A ${pick(r, ADJ)} History`
			])();
		} while (seen.has(title) && r() < 0.97);
		seen.add(title);
		const s = r() < 0.32 ? series[Math.floor(r() * series.length)] : null;
		const author = s ? s.author : pick(r, authors);
		const authorList = r() < 0.12 ? [author, pick(r, authors)].filter((a, k, arr) => arr.indexOf(a) === k) : [author];
		const tags = new Set<string>([s ? s.tag : pick(r, TAGS)]);
		if (r() < 0.55) tags.add(pick(r, TAGS));
		if (r() < 0.2) tags.add(pick(r, TAGS));
		const lang = s ? s.lang : pick(r, LANGS);
		const formats = ["EPUB"];
		if (r() < 0.35) formats.push("PDF");
		if (r() < 0.18) formats.push("MOBI");
		if (r() < 0.12) formats.push("AZW3");
		if (r() < 0.06) formats.push("KEPUB");
		if (tags.has("Graphic novel")) formats.splice(0, formats.length, "CBZ");
		if (r() < 0.04) formats.splice(0, formats.length, "FB2");
		const files = formats.map((f) => ({
			format: f,
			fileName: `${title.replace(/[^\w ]/g, "").slice(0, 40)} - ${author}`,
			size: Math.round(f === "CBZ" ? 40e6 + r() * 220e6 : f === "PDF" ? 1.5e6 + r() * 26e6 : 0.25e6 + r() * 3.5e6)
		}));
		const added = NOW - Math.floor(Math.pow(r(), 1.6) * 7 * 365 * 864e5);
		const pubYear = 1925 + Math.floor(Math.pow(r(), 0.55) * 100);
		const ids: Record<string, string> = {};
		if (r() < 0.8) ids.isbn = String(9780000000000 + Math.floor(r() * 99999999999));
		if (r() < 0.3) ids.amazon = "B0" + Math.floor(r() * 1e8).toString(36).toUpperCase().padStart(8, "0");
		if (r() < 0.2) ids.goodreads = String(Math.floor(r() * 5e7));
		if (r() < 0.06) ids.google = Math.floor(r() * 1e12).toString(36);
		if (r() < 0.03) ids.doi = `10.${1000 + Math.floor(r() * 8999)}/${Math.floor(r() * 1e6)}`;
		const paras = Array.from({ length: 1 + Math.floor(r() * 3) }, () =>
			pick(r, SENT).replace("{n1}", pick(r, NOUN).toLowerCase()).replace("{p}", pick(r, PLACE))
		);
		let comments = paras.map((p) => `<p>${r() < 0.2 ? `<b>${p.split(",")[0]}</b>${p.slice(p.indexOf(",") > 0 ? p.indexOf(",") : 0)}` : p}</p>`).join("");
		if (r() < 0.15) comments += `<ul><li>${pick(r, ADJ)} prose</li><li>${pick(r, ADJ)} pacing</li><li><i>${pick(r, NOUN)}</i> as metaphor</li></ul>`;
		if (i === 1) comments = `<p>Sanitiser check <script>window.__pwned = 1</script><b onclick="alert(1)">bold</b> <a href="javascript:alert(1)">bad link</a> <a href="https://example.com/">good link</a><img src=x onerror="alert(2)"><iframe src="https://example.com"></iframe></p><style>body{display:none}</style>`;
		const stamp = new Date(added).toISOString();
		const book: MockBook = {
			id: i,
			title,
			sort: sortKey(title),
			authors: authorList,
			authorSort: authorList.map((a) => a.split(" ").reverse().join(", ")).join(" & "),
			series: s?.name ?? null,
			seriesIndex: s ? s.next++ : 1,
			tags: [...tags],
			publisher: s ? s.publisher : r() < 0.9 ? pick(r, PUBLISHERS) : null,
			languages: [lang],
			rating: r() < 0.45 ? null : 2 * (1 + Math.floor(r() * 5)),
			formats,
			identifiers: ids,
			hasCover: r() < 0.93,
			size: files.reduce((a, f) => a + f.size, 0),
			timestamp: stamp,
			pubdate: r() < 0.06 ? null : new Date(Date.UTC(pubYear, Math.floor(r() * 12), 1 + Math.floor(r() * 27))).toISOString(),
			lastModified: new Date(added + Math.floor(r() * 3e8)).toISOString(),
			comments,
			uuid: `00000000-0000-4000-8000-${i.toString(16).padStart(12, "0")}`,
			path: `${author}/${title} (${i})`,
			files,
			hay: "",
			...EBOOK_ONLY
		};
		book.hay = makeHay(book);
		books.push(book);
	}
	return books;
}

interface MockAudio extends Omit<AudiobookInfo, "location" | "link" | "candidates"> {
	auto: number | null;
	willBeMissing: boolean;
}

interface AudioMock {
	roots: AudioRoot[];
	books: MockAudio[];
	links: { audiobookId: number; bookId: number; state: "manual" | "rejected" }[];
	jobs: Map<number, { cancelled: boolean }>;
	issues: AudioIssue[];
	nextRoot: number;
	nextJob: number;
}

const SUBTITLES = ["A Novel", "Unabridged", "The Complete Story", "A Memoir", "Book One"];

function generateAudio(books: MockBook[], seed: number): MockAudio[] {
	const r = mulberry32(seed);
	const narrators = Array.from({ length: 16 }, () => `${pick(r, FIRST)} ${pick(r, LAST)}`);
	const out: MockAudio[] = [];
	const add = (title: string, authors: string[], series: string | null, seriesIndex: number | null, auto: MockBook | null) => {
		const id = out.length + 1;
		const multi = r() < 0.18;
		const durationMs = Math.round((2 + r() * 26) * 3.6e6);
		const narratorList = r() < 0.2 ? [...new Set([pick(r, narrators), pick(r, narrators)])] : [pick(r, narrators)];
		const fileCount = multi ? 8 + Math.floor(r() * 30) : 1;
		const files: AudiobookFile[] = [];
		const audioSize = Math.round((durationMs / 1000) * (multi ? 16000 : 8000));
		for (let n = 1; n <= fileCount; n++) {
			files.push({
				relPath: multi ? `${String(n).padStart(2, "0")} - Part ${n}.mp3` : `${title.replace(/[^\w ]/g, "")}.m4b`,
				kind: "audio",
				size: Math.round(audioSize / fileCount),
				durationMs: Math.round(durationMs / fileCount),
				track: multi ? n : null,
				title: multi ? `Part ${n}` : null
			});
		}
		const hasCover = r() < 0.9;
		if (hasCover) files.push({ relPath: "cover.jpg", kind: "cover", size: 180000, durationMs: null, track: null, title: null });
		if (r() < 0.3) files.push({ relPath: "metadata.json", kind: "meta", size: 2400, durationMs: null, track: null, title: null });
		const chapterCount = multi ? 0 : 6 + Math.floor(r() * 40);
		const chapters = Array.from({ length: chapterCount }, (_, n) => ({
			title: n === 0 ? "Opening Credits" : n === chapterCount - 1 ? "Epilogue" : `Chapter ${n}`,
			startMs: Math.round((durationMs / chapterCount) * n)
		}));
		const added = NOW - Math.floor(r() * 400 * 864e5);
		const paras = Array.from({ length: 1 + Math.floor(r() * 2) }, () =>
			pick(r, SENT).replace("{n1}", pick(r, NOUN).toLowerCase()).replace("{p}", pick(r, PLACE))
		);
		out.push({
			id,
			rootId: null,
			state: "present",
			title,
			subtitle: r() < 0.25 ? pick(r, SUBTITLES) : null,
			authors,
			narrators: narratorList,
			series,
			seriesIndex,
			publisher: r() < 0.8 ? pick(r, ["Audible Studios", "Tantor Audio", "Penguin Audio", "Blackstone Publishing", "Podium Audio"]) : null,
			language: auto?.languages[0] ?? pick(r, LANGS),
			genres: auto ? auto.tags.slice(0, 2) : [pick(r, TAGS)],
			description: paras.map((p) => `<p>${p}</p>`).join(""),
			asin: r() < 0.85 ? "B0" + Math.floor(r() * 1e8).toString(36).toUpperCase().padStart(8, "0") : null,
			isbn: r() < 0.3 ? String(9780000000000 + Math.floor(r() * 99999999999)) : null,
			releaseDate: new Date(Date.UTC(2005 + Math.floor(r() * 20), Math.floor(r() * 12), 1 + Math.floor(r() * 27))).toISOString(),
			durationMs,
			size: audioSize + (hasCover ? 180000 : 0),
			format: multi ? "mp3" : "m4b",
			fileCount,
			hasCover,
			added: new Date(added).toISOString(),
			modified: new Date(added + Math.floor(r() * 2e8)).toISOString(),
			dir: `${authors[0]}/${title.replace(/[:?]/g, "")}`,
			files,
			chapters,
			auto: auto?.id ?? null,
			willBeMissing: false
		});
	};
	const pairedCount = Math.min(40, Math.floor(books.length / 2));
	for (let i = 0; i < pairedCount; i++) {
		const b = books[Math.floor(((i + 0.5) * books.length) / pairedCount)];
		add(b.title, b.authors, b.series, b.series ? b.seriesIndex : null, b);
	}
	const near = Math.min(12, Math.floor(books.length / 8));
	for (let i = 0; i < near; i++) {
		const b = books[Math.floor(((i + 0.25) * books.length) / near)];
		add(`${b.title}: Complete Edition`, b.authors, null, null, null);
	}
	const authors = Array.from({ length: 24 }, () => `${pick(r, FIRST)} ${pick(r, LAST)}`);
	const series = Array.from({ length: 6 }, () => ({ name: `The ${pick(r, PLACE)} Saga`, author: pick(r, authors), next: 1 }));
	for (let i = 0; i < 60 - near; i++) {
		const s = r() < 0.3 ? pick(r, series) : null;
		add(
			pick(r, [
				() => `The ${pick(r, ADJ)} ${pick(r, NOUN)}`,
				() => `${pick(r, NOUN)} of ${pick(r, PLACE)}`,
				() => `${pick(r, ADJ)} ${pick(r, NOUN)}s`,
				() => `Last ${pick(r, NOUN)} in ${pick(r, PLACE)}`
			])(),
			[s ? s.author : pick(r, authors)],
			s?.name ?? null,
			s ? s.next++ : null,
			null
		);
	}
	for (const i of [7, 31]) out[Math.min(i, out.length - 1)].willBeMissing = true;
	const failing = out[Math.min(60, out.length - 1)];
	failing.title = "The Failing Lighthouse";
	failing.dir = `${failing.authors[0]}/The Failing Lighthouse`;
	return out;
}

function newAudio(books: MockBook[], scanned: boolean): AudioMock {
	const au: AudioMock = { roots: [], books: generateAudio(books, 777), links: [], jobs: new Map(), issues: [], nextRoot: 1, nextJob: 0 };
	if (scanned) {
		au.roots.push({
			id: au.nextRoot++,
			path: "/home/reader/Audiobooks",
			inLibrary: false,
			managed: false,
			layout: "libation",
			label: null,
			enabled: true,
			lastScan: new Date(NOW).toISOString(),
			lastScanOk: true,
			audiobooks: 0,
			missing: 0
		});
		for (const a of au.books) {
			a.rootId = 1;
			if (a.willBeMissing) a.state = "missing";
		}
		au.issues = sampleIssues("/home/reader/Audiobooks");
	}
	return au;
}

function sampleIssues(root: string): AudioIssue[] {
	return [
		{ path: `${root}/Unknown Author/Broken Download/part 3.mp3`, message: "Could not read tags: unexpected end of file" },
		{ path: `${root}/Scans/Booklet.pdf`, message: "Ignored: not part of an audiobook" }
	];
}

function brief(list: MockAudio[]): AudioBrief {
	const a = list[0];
	return { narrators: a.narrators, durationMs: a.durationMs, format: a.format, fileCount: a.fileCount, size: a.size, asin: a.asin, state: a.state, count: list.length, hasCover: a.hasCover };
}

function pairOf(au: AudioMock, a: MockAudio, byId: Map<number, MockBook>): { bookId: number; manual: boolean } | null {
	const manual = au.links.find((l) => l.audiobookId === a.id && l.state === "manual" && byId.has(l.bookId));
	if (manual) return { bookId: manual.bookId, manual: true };
	if (a.auto === null || !byId.has(a.auto)) return null;
	if (au.links.some((l) => l.audiobookId === a.id && l.bookId === a.auto && l.state === "rejected")) return null;
	return { bookId: a.auto, manual: false };
}

function catalog(s: State) {
	const on = new Set(s.audio.roots.filter((r) => r.enabled).map((r) => r.id));
	const list = s.audio.books.filter((a) => a.rootId !== null && on.has(a.rootId));
	const byId = new Map(s.books.map((b) => [b.id, b]));
	const pairs = new Map<number, { bookId: number; manual: boolean }>();
	const byBook = new Map<number, MockAudio[]>();
	for (const a of list) {
		const p = pairOf(s.audio, a, byId);
		if (!p) continue;
		pairs.set(a.id, p);
		const group = byBook.get(p.bookId);
		if (group) group.push(a);
		else byBook.set(p.bookId, [a]);
	}
	return { list, byId, pairs, byBook };
}

interface State {
	library: LibraryInfo | null;
	recents: LibraryInfo[];
	books: MockBook[];
	nextId: number;
	saved: SavedSearch[];
	progress: Record<string, number>;
	reading: Map<number, ReadingState>;
	annotations: Annotation[];
	deviceFiles: Map<string, DeviceBook[]>;
	audio: AudioMock;
}

let state: State | null = null;
const audioByPath = new Map<string, AudioMock>();

function init(): State {
	const p = params();
	const books = p.empty ? [] : generate(p.count, 20260101);
	const lib: LibraryInfo = { path: "/home/reader/Calibre Library", name: "Calibre Library", bookCount: books.length };
	const r = mulberry32(99);
	const progress: Record<string, number> = {};
	for (const b of books) if (r() < 0.14) progress[b.id] = r() < 0.3 ? 1 : Math.round(r() * 100) / 100;
	const dev = (id: string) => books.slice(0, 6).map<DeviceBook>((b, i) => ({
		path: `/media/${id}/Books/${b.title}.epub`,
		relPath: `Books/${b.title}.epub`,
		title: b.title,
		authors: b.authors,
		format: "EPUB",
		size: b.files[0].size,
		modified: b.lastModified,
		uuid: i % 2 ? b.uuid : null
	}));
	const kindleFiles = (lib: MockBook[]) => {
		const mk = (title: string, authors: string[], format: string, size: number, uuid: string | null = null): DeviceBook => ({
			path: `/media/Kindle/documents/${title}.${format.toLowerCase()}`,
			relPath: `documents/${title}.${format.toLowerCase()}`,
			title, authors, format, size, modified: "2026-10-12T18:20:00Z", uuid
		});
		const mine = lib.slice(6, 9).map((b, i) => mk(b.title, b.authors, ["AZW3", "MOBI", "KFX"][i], b.files[0].size));
		return [
			...mine,
			mk("The Pragmatic Programmer: From Journeyman to Master, 20th Anniversary Edition (Extended Notes and Marginalia)", ["David Thomas", "Andrew Hunt"], "AZW3", 3_400_000),
			mk("Die Verwandlung", ["Franz Kafka"], "MOBI", 310_000),
			mk("Dune", ["Frank Herbert"], "KFX", 2_100_000),
			mk("Pride and Prejudice", ["Jane Austen"], "AZW3", 1_250_000),
			mk("Sapiens", ["Yuval Noah Harari"], "AZW3", 5_800_000),
			mk("Clean Architecture", ["Robert C. Martin"], "PDF", 8_900_000),
			mk("Reisetagebuch_2024_final_v3_korrigiert_mit_allen_bildern_und_karten_komprimiert", [], "PDF", 22_000_000),
			mk("Meditations", ["Marcus Aurelius"], "EPUB", 480_000)
		];
	};
	return {
		library: p.noLibrary ? null : lib,
		recents: [
			lib,
			{ path: "/home/reader/Comics", name: "Comics", bookCount: 412 },
			{ path: "/mnt/nas/Reference Shelf", name: "Reference Shelf", bookCount: 96 }
		],
		books,
		nextId: books.length + 1,
		saved: [
			{ name: "Unread fantasy", query: 'tags:"=Fantasy"' },
			{ name: "Highly rated", query: 'rating:">=4"' }
		],
		progress,
		reading: new Map(),
		annotations: [],
		deviceFiles: new Map([
			["kobo-libra", dev("kobo")],
			["kindle-scribe", kindleFiles(books)],
			["folder-backup", []]
		]),
		audio: newAudio(books, p.audio)
	};
}

function toStars(v: string) {
	const m = /^(>=|<=|>|<|=)?\s*(\d+)/.exec(v);
	return m ? { op: m[1] ?? "=", n: Number(m[2]) } : null;
}

function tokenize(q: string): string[] {
	const out: string[] = [];
	let cur = "";
	let quote = false;
	for (const ch of q) {
		if (ch === '"') quote = !quote;
		if (/\s/.test(ch) && !quote) {
			if (cur) out.push(cur);
			cur = "";
		} else cur += ch;
	}
	if (cur) out.push(cur);
	return out;
}

interface Hit extends BookSummary {
	comments: string | null;
	hay: string;
	narrators: string[];
	hasEbook: boolean;
	hasAudio: boolean;
}

function fieldValues(b: Hit, field: string): string[] | null {
	switch (field) {
		case "title":
			return [b.title];
		case "author":
		case "authors":
			return b.authors;
		case "series":
			return b.series ? [b.series] : [];
		case "tag":
		case "tags":
			return b.tags;
		case "publisher":
			return b.publisher ? [b.publisher] : [];
		case "language":
		case "languages":
			return b.languages;
		case "format":
		case "formats":
			return b.audio ? [...b.formats, b.audio.format.toUpperCase()] : b.formats;
		case "narrator":
		case "narrators":
			return b.narrators;
		case "comments":
			return b.comments ? [b.comments] : [];
		case "identifiers":
			return Object.entries(b.identifiers).map(([k, v]) => `${k}:${v}`);
		default:
			return null;
	}
}

function matchTerm(b: Hit, term: string): boolean {
	let neg = false;
	if (term.startsWith("-") && term.length > 1) {
		neg = true;
		term = term.slice(1);
	}
	let res: boolean;
	const m = /^(\w+):(.*)$/.exec(term);
	if (m) {
		const field = m[1].toLowerCase();
		let value = m[2];
		if (value.startsWith('"') && value.endsWith('"') && value.length >= 2) value = value.slice(1, -1);
		if (field === "audio" || field === "ebook") {
			res = (field === "audio" ? b.hasAudio : b.hasEbook) === (value.toLowerCase() === "true");
		} else if (field === "duration") {
			const t = /^(>=|<=|>|<|=)?\s*(\d+(?:\.\d+)?)/.exec(value);
			const hours = (b.audio?.durationMs ?? 0) / 3.6e6;
			const n = Number(t?.[2]);
			const op = t?.[1] ?? "=";
			res = !t ? false : op === "=" ? hours === n : op === ">" ? hours > n : op === ">=" ? hours >= n : op === "<" ? hours < n : hours <= n;
		} else if (field === "rating") {
			const t = toStars(value);
			const stars = (b.rating ?? 0) / 2;
			res = !t ? false : t.op === "=" ? stars === t.n : t.op === ">" ? stars > t.n : t.op === ">=" ? stars >= t.n : t.op === "<" ? stars < t.n : stars <= t.n;
		} else {
			const vals = fieldValues(b, field);
			if (!vals) res = b.hay.includes(term.toLowerCase().replace(/"/g, ""));
			else if (field === "identifiers" && value.endsWith(":true")) res = vals.some((v) => v.startsWith(value.slice(0, -4)));
			else if (value.startsWith("=")) res = vals.some((v) => v.toLowerCase() === value.slice(1).toLowerCase());
			else res = vals.some((v) => v.toLowerCase().includes(value.toLowerCase()));
		}
	} else res = b.hay.includes(term.replace(/"/g, "").toLowerCase());
	return neg ? !res : res;
}

function compile(search: string | null | undefined): (b: Hit) => boolean {
	const tokens = tokenize((search ?? "").replace(/[()]/g, " ")).filter((t) => t.toLowerCase() !== "and");
	if (!tokens.length) return () => true;
	const groups: string[][] = [[]];
	for (const t of tokens) {
		if (t.toLowerCase() === "or") groups.push([]);
		else groups[groups.length - 1].push(t);
	}
	return (b) => groups.some((g) => g.every((t) => matchTerm(b, t)));
}

const collator = new Intl.Collator("en", { sensitivity: "base", numeric: true });

function compare(field: SortField): (a: Hit, b: Hit) => number {
	switch (field) {
		case "title":
			return (a, b) => collator.compare(a.sort, b.sort);
		case "author":
			return (a, b) => collator.compare(a.authorSort, b.authorSort) || collator.compare(a.sort, b.sort);
		case "series":
			return (a, b) =>
				a.series === b.series ? a.seriesIndex - b.seriesIndex : !a.series ? 1 : !b.series ? -1 : collator.compare(a.series, b.series);
		case "added":
			return (a, b) => a.timestamp.localeCompare(b.timestamp);
		case "published":
			return (a, b) => (a.pubdate ?? "").localeCompare(b.pubdate ?? "");
		case "modified":
			return (a, b) => a.lastModified.localeCompare(b.lastModified);
		case "rating":
			return (a, b) => (a.rating ?? 0) - (b.rating ?? 0);
		case "size":
			return (a, b) => a.size - b.size;
		case "publisher":
			return (a, b) => collator.compare(a.publisher ?? "￿", b.publisher ?? "￿");
		case "duration":
			return (a, b) => (a.audio?.durationMs ?? 0) - (b.audio?.durationMs ?? 0);
	}
}

function summary(b: MockBook) {
	const { comments: _c, uuid: _u, path: _p, files: _f, hay: _h, ...s } = b;
	return s;
}

function ebookHit(b: MockBook, group?: MockAudio[]): Hit {
	return {
		...summary(b),
		kind: group ? "both" : "ebook",
		audiobookId: group ? group[0].id : null,
		audio: group ? brief(group) : null,
		comments: b.comments,
		hay: group ? `${b.hay}\n${group[0].narrators.join(" ").toLowerCase()}` : b.hay,
		narrators: group ? group[0].narrators : [],
		hasEbook: true,
		hasAudio: !!group
	};
}

function audioHit(a: MockAudio, bookId: number | null): Hit {
	const identifiers: Record<string, string> = {};
	if (a.asin) identifiers.asin = a.asin;
	if (a.isbn) identifiers.isbn = a.isbn;
	return {
		id: -a.id,
		title: a.title,
		sort: sortKey(a.title),
		authors: a.authors,
		authorSort: a.authors.map((n) => n.split(" ").reverse().join(", ")).join(" & "),
		series: a.series,
		seriesIndex: a.seriesIndex ?? 1,
		tags: a.genres,
		publisher: a.publisher,
		languages: a.language ? [a.language] : [],
		rating: null,
		formats: [],
		identifiers,
		hasCover: a.hasCover,
		size: a.size,
		timestamp: a.added,
		pubdate: a.releaseDate,
		lastModified: a.modified,
		kind: bookId === null ? "audiobook" : "both",
		audiobookId: a.id,
		audio: brief([a]),
		pairedBookId: bookId,
		comments: a.description,
		hay: [a.title, a.subtitle ?? "", a.authors.join(" "), a.narrators.join(" "), a.series ?? "", a.genres.join(" "), a.publisher ?? "", a.description ?? "", Object.values(identifiers).join(" ")]
			.join("\n")
			.toLowerCase(),
		narrators: a.narrators,
		hasEbook: bookId !== null,
		hasAudio: true
	};
}

function entries(s: State, scope: Scope): Hit[] {
	const { list, pairs, byBook } = catalog(s);
	const out: Hit[] = [];
	if (scope !== "audiobooks") for (const b of s.books) out.push(ebookHit(b, byBook.get(b.id)));
	if (scope !== "ebooks") {
		for (const a of list) {
			const p = pairs.get(a.id);
			if (scope === "audiobooks" || !p) out.push(audioHit(a, p?.bookId ?? null));
		}
	}
	return out;
}

function plain(h: Hit): BookSummary {
	const { comments: _c, hay: _h, narrators: _n, hasEbook: _e, hasAudio: _a, ...rest } = h;
	return rest;
}

function categories(s: State, search: string | null, scope: Scope): Category[] {
	const match = compile(search);
	const subset = entries(s, scope).filter(match);
	const audible = catalog(s).list.length > 0;
	const build = (key: string, values: (b: Hit) => string[], searchFor: (v: string) => string, label = (v: string) => v) => {
		const counts = new Map<string, number>();
		for (const b of subset) for (const v of values(b)) counts.set(v, (counts.get(v) ?? 0) + 1);
		const items: CategoryItem[] = [...counts]
			.sort((a, b) => collator.compare(a[0], b[0]))
			.map(([v, count], i) => ({ id: i + 1, name: label(v), count, search: searchFor(v) }));
		return { key, items };
	};
	const stars = build("rating", (b) => (b.rating ? [String(b.rating / 2)] : []), (v) => `rating:"=${v}"`, (v) => `${v} ${v === "1" ? "star" : "stars"}`);
	stars.items.reverse();
	return [
		...(audible ? [audioCategory(subset)] : []),
		build("authors", (b) => b.authors, (v) => `authors:"=${v}"`),
		...(audible ? [build("narrators", (b) => b.narrators, (v) => `narrators:"=${v}"`)] : []),
		build("series", (b) => (b.series ? [b.series] : []), (v) => `series:"=${v}"`),
		build("tags", (b) => b.tags, (v) => `tags:"=${v}"`),
		build("publisher", (b) => (b.publisher ? [b.publisher] : []), (v) => `publisher:"=${v}"`),
		build("languages", (b) => b.languages, (v) => `languages:"=${v}"`),
		build("formats", (b) => (b.audio ? [...b.formats, b.audio.format.toUpperCase()] : b.formats), (v) => `formats:"=${v}"`),
		stars,
		build("identifiers", (b) => Object.keys(b.identifiers), (v) => `identifiers:${v}:true`)
	];
}

function audioCategory(subset: Hit[]): Category {
	const total = subset.length;
	const audio = subset.filter((h) => h.hasAudio).length;
	const ebook = subset.filter((h) => h.hasEbook).length;
	const items: CategoryItem[] = [
		{ name: "Has audiobook", count: audio, search: "audio:true" },
		{ name: "Has ebook", count: ebook, search: "ebook:true" },
		{ name: "Both formats", count: audio + ebook - total, search: "audio:true and ebook:true" },
		{ name: "No audiobook", count: total - audio, search: "audio:false" },
		{ name: "No ebook", count: total - ebook, search: "ebook:false" }
	]
		.filter((i) => i.count > 0 && i.count < total)
		.map((i) => ({ id: null, ...i }));
	return { key: "audio", items };
}

function apply(b: MockBook, patch: BookPatch) {
	if (patch.title !== undefined) {
		b.title = patch.title;
		b.sort = patch.titleSort ?? sortKey(patch.title);
	}
	if (patch.titleSort !== undefined) b.sort = patch.titleSort;
	if (patch.authors) b.authors = patch.authors;
	if (patch.authorSort !== undefined) b.authorSort = patch.authorSort;
	if (patch.series !== undefined) b.series = patch.series || null;
	if (patch.seriesIndex !== undefined) b.seriesIndex = patch.seriesIndex;
	if (patch.tags) b.tags = patch.tags;
	if (patch.publisher !== undefined) b.publisher = patch.publisher || null;
	if (patch.pubdate !== undefined) b.pubdate = patch.pubdate || null;
	if (patch.languages) b.languages = patch.languages;
	if (patch.identifiers) b.identifiers = patch.identifiers;
	if (patch.rating !== undefined) b.rating = patch.rating || null;
	if (patch.comments !== undefined) b.comments = patch.comments;
	b.lastModified = new Date().toISOString();
	b.hay = makeHay(b);
}

function find(s: State, id: number) {
	const b = s.books.find((x) => x.id === id);
	if (!b) throw `book ${id} not found`;
	return b;
}

function full(b: MockBook): Book {
	const { hay: _h, ...rest } = b;
	return rest;
}

function fakeResults(q: IdentifyQuery): SourceResult[] {
	const title = q.title || "Untitled";
	const author = q.authors?.[0] || "Unknown";
	return ["Open Library", "Google Books", "WorldCat"].map((source, i) => ({
		source,
		metadata: {
			title: i === 1 ? `${title}: A Novel` : title,
			titleSort: null,
			authors: [author],
			authorSort: null,
			series: null,
			seriesIndex: null,
			tags: ["Fiction", "Literature"],
			publisher: ["Lantern & Quill", "Northgate Press", "Ember House"][i],
			pubdate: `${2008 + i * 3}-05-01T00:00:00+00:00`,
			languages: ["eng"],
			identifiers: { isbn: String(9781000000000 + hashString(title + i) % 999999999) },
			rating: null,
			comments: `<p>A matched record from ${source} for "${title}".</p>`,
			uuid: null
		},
		coverUrl: generatedCover(title, author, hashString(title) + i * 37),
		url: "https://example.com/"
	}));
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function rootInfo(au: AudioMock, r: AudioRoot): AudioRoot {
	const own = au.books.filter((a) => a.rootId === r.id);
	return { ...r, audiobooks: own.length, missing: own.filter((a) => a.state === "missing").length };
}

function audioStatus(s: State) {
	const roots = s.audio.roots.map((r) => rootInfo(s.audio, r));
	return {
		roots,
		audiobooks: roots.filter((r) => r.enabled).reduce((n, r) => n + r.audiobooks, 0),
		issues: s.audio.issues
	};
}

function startScan(s: State, rootIds: number[] | null, onEvent: (e: JobEvent) => void) {
	const au = s.audio;
	const ids = rootIds ?? au.roots.filter((r) => r.enabled).map((r) => r.id);
	const id = ++au.nextJob;
	const job = { cancelled: false };
	au.jobs.set(id, job);
	void simulateScan(au, ids, job, onEvent).finally(() => au.jobs.delete(id));
	return id;
}

async function simulateScan(au: AudioMock, ids: number[], job: { cancelled: boolean }, onEvent: (e: JobEvent) => void) {
	const total: ScanSummary = { audiobooks: 0, added: 0, updated: 0, unchanged: 0, missing: 0, issues: 0, complete: true };
	const progress = (phase: "walking" | "reading", done: number, count: number, current: string) =>
		onEvent({ event: "progress", phase, done, total: count, current });
	for (const id of ids) {
		const root = au.roots.find((r) => r.id === id);
		if (!root) continue;
		if (/fail/i.test(root.path)) return onEvent({ event: "failed", message: `Could not read ${root.path}: permission denied` });
		const offline = /offline/i.test(root.path);
		const claim = offline ? [] : au.books.filter((a) => a.rootId === null || a.rootId === id);
		const files = claim.reduce((n, a) => n + a.fileCount, 0);
		for (let i = 1; i <= 8; i++) {
			if (job.cancelled) return onEvent({ event: "cancelled" });
			progress("walking", Math.round((files * i) / 8), 0, `${root.path}/${claim[Math.floor(((i - 1) * claim.length) / 8)]?.dir ?? ""}`);
			await sleep(75);
		}
		let added = 0;
		for (const [i, a] of claim.entries()) {
			if (job.cancelled) return onEvent({ event: "cancelled" });
			if (a.rootId === null) added++;
			a.rootId = id;
			a.state = a.willBeMissing ? "missing" : "present";
			progress("reading", i + 1, claim.length, `${root.path}/${a.dir}`);
			await sleep(Math.max(4, 1200 / claim.length));
		}
		const missing = claim.filter((a) => a.state === "missing").length;
		root.lastScan = new Date().toISOString();
		root.lastScanOk = !offline;
		total.audiobooks += claim.length;
		total.added += added;
		total.unchanged += claim.length - added - missing;
		total.missing += missing;
		total.complete &&= !offline;
	}
	if (total.audiobooks) {
		au.issues = sampleIssues(au.roots.find((r) => r.id === ids[0])?.path ?? "");
		total.issues = au.issues.length;
	}
	onEvent({ event: "done", result: total });
}

const tokens = (t: string) => new Set(t.toLowerCase().replace(/[^\p{L}\p{N}\s]/gu, " ").split(/\s+/).filter(Boolean));

function candidatesFor(s: State, a: MockAudio) {
	const ta = tokens(a.title);
	const out: { book: BookSummary; score: number }[] = [];
	for (const b of s.books) {
		if (s.audio.links.some((l) => l.audiobookId === a.id && l.bookId === b.id && l.state === "rejected")) continue;
		const tb = tokens(b.title);
		let hit = 0;
		for (const t of ta) if (tb.has(t)) hit++;
		let score = hit / (ta.size + tb.size - hit);
		if (a.authors.some((x) => b.authors.includes(x))) score = Math.min(1, score + 0.15);
		if (score >= 0.5) out.push({ book: summary(b), score });
	}
	return out.sort((x, y) => y.score - x.score).slice(0, 5);
}

function audiobookInfo(s: State, id: number): AudiobookInfo {
	const a = s.audio.books.find((x) => x.id === id);
	if (!a) throw `audiobook ${id} not found`;
	const { auto: _a, willBeMissing: _w, ...row } = a;
	const root = s.audio.roots.find((r) => r.id === a.rootId);
	const base = root ? (root.inLibrary ? `${s.library?.path}/${root.path}` : root.path) : null;
	const byId = new Map(s.books.map((b) => [b.id, b]));
	const pair = pairOf(s.audio, a, byId);
	const book = pair && byId.get(pair.bookId);
	return {
		...row,
		location: base && a.dir ? `${base}/${a.dir}` : base,
		link: book && pair ? { book: summary(book), manual: pair.manual } : null,
		candidates: book ? [] : candidatesFor(s, a)
	};
}

function transferPlan(s: State, ids: number[], destRootId: number | null, destFolder: string | null): TransferPlan {
	const au = s.audio;
	const root = destRootId === null ? null : au.roots.find((r) => r.id === destRootId);
	if (destRootId !== null && !root) throw `location ${destRootId} not found`;
	if (root && !root.managed) throw "Shelfmark is not allowed to add files to this location";
	if (!root && !destFolder) throw "No destination was chosen";
	const destPath = root ? (root.inLibrary ? `${s.library?.path}/${root.path}` : root.path) : (destFolder as string);
	const taken = new Set<string>();
	const items = ids.map((id): TransferPlanItem => {
		const a = au.books.find((x) => x.id === id);
		if (!a) throw `audiobook ${id} not found`;
		const files = a.files.map((f) => ({ src: `${a.dir}/${f.relPath}`, dest: `${a.dir}/${f.relPath}`, size: f.size }));
		const dir = a.dir ?? a.title;
		let skip: string | null = null;
		if (a.rootId === null || a.state !== "present") skip = "files are not available";
		else if (root && a.rootId === root.id) skip = "already in this location";
		else if (!taken.add(dir.toLowerCase())) skip = "another selected audiobook uses the same folder name";
		else if (a.id % 9 === 0) skip = `folder already exists: ${dir}`;
		return { audiobookId: id, title: a.title, destDir: dir, key: dir, files, bytes: files.reduce((n, f) => n + f.size, 0), skip };
	});
	const totalBytes = items.filter((i) => !i.skip).reduce((n, i) => n + i.bytes, 0);
	const freeBytes = root?.inLibrary ? null : /full/i.test(destPath) ? 50e6 : 120e9;
	return { items, totalBytes, freeBytes, enoughSpace: freeBytes === null || freeBytes > totalBytes, destPath };
}

function startTransfer(s: State, a: Record<string, any>) {
	const au = s.audio;
	const mode = a.mode as "copy" | "move";
	if (mode === "move" && a.destRootId === null) throw "moving is only possible between audiobook locations";
	const plan = transferPlan(s, a.ids, a.destRootId, a.destFolder);
	const id = ++au.nextJob;
	const job = { cancelled: false };
	au.jobs.set(id, job);
	void simulateTransfer(au, plan, mode, a.destRootId, job, a.onEvent).finally(() => au.jobs.delete(id));
	return id;
}

async function simulateTransfer(
	au: AudioMock,
	plan: TransferPlan,
	mode: "copy" | "move",
	destRootId: number | null,
	job: { cancelled: boolean },
	onEvent: (e: JobEvent<TransferResult>) => void
) {
	if (!plan.enoughSpace) return onEvent({ event: "failed", message: "not enough free space at the destination" });
	const result: TransferResult = { done: 0, skipped: [], failed: [], bytes: 0, cancelled: false };
	const todo = plan.items.filter((i) => !i.skip);
	const steps = Math.max(2, Math.min(14, Math.round(80 / Math.max(1, todo.length))));
	const progress = (done: number, current: string) => onEvent({ event: "progress", phase: "copying", done, total: plan.totalBytes, current });
	let done = 0;
	for (const item of plan.items) {
		if (item.skip) {
			result.skipped.push({ audiobookId: item.audiobookId, title: item.title, message: item.skip });
			continue;
		}
		const failing = /fail/i.test(item.title);
		let stopped = false;
		for (let k = 1; k <= steps; k++) {
			if (job.cancelled) {
				stopped = true;
				break;
			}
			progress(done + (item.bytes * k) / steps, item.title);
			await sleep(35);
			if (failing && k === Math.ceil(steps / 2)) break;
		}
		if (stopped) {
			result.cancelled = true;
			break;
		}
		done += item.bytes;
		if (failing) {
			result.failed.push({ audiobookId: item.audiobookId, title: item.title, message: `copy does not match the original: ${plan.destPath}/${item.destDir}` });
			continue;
		}
		result.done++;
		result.bytes += item.bytes;
		const book = au.books.find((x) => x.id === item.audiobookId);
		if (mode === "move" && destRootId !== null && book) book.rootId = destRootId;
	}
	if (mode === "copy" && result.done > 0 && destRootId !== null) {
		for (let i = 1; i <= 6; i++) {
			onEvent({ event: "progress", phase: i < 4 ? "walking" : "reading", done: i, total: i < 4 ? 0 : 6, current: plan.destPath });
			await sleep(60);
		}
	}
	onEvent({ event: "done", result });
}

function audioFile(s: State, id: number) {
	const a = s.audio.books.find((x) => x.id === id);
	if (!a) throw `audiobook ${id} not found`;
	if (a.rootId === null) throw "This audiobook has not been downloaded";
	if (a.state === "missing") throw `File is not reachable: ${a.dir}/${a.files[0].relPath}`;
}

export async function mockInvoke(cmd: string, args: Record<string, unknown> = {}): Promise<unknown> {
	const s = (state ??= init());
	const a = args as Record<string, any>;
	switch (cmd) {
		case "current_library":
			return s.library;
		case "recent_libraries":
			return s.recents;
		case "open_library":
		case "create_library": {
			await sleep(150);
			const path = a.path as string;
			const create = cmd === "create_library";
			const existing = s.recents.find((l) => l.path === path);
			if (path === "/home/reader/Calibre Library" && existing && !create) {
				s.books = generate(params().count, 20260101);
			} else {
				s.books = create ? [] : generate(240, hashString(path));
				s.progress = {};
			}
			const info: LibraryInfo = { path, name: baseName(path), bookCount: s.books.length };
			if (s.library) audioByPath.set(s.library.path, s.audio);
			s.audio = (!create && audioByPath.get(path)) || newAudio(s.books, false);
			s.library = info;
			s.recents = [info, ...s.recents.filter((l) => l.path !== path)];
			s.nextId = s.books.length + 1;
			return info;
		}
		case "list_books": {
			await sleep(25);
			const q = a.query as ListQuery;
			const match = compile(q.search);
			const out = entries(s, q.scope ?? "ebooks").filter(match);
			const cmp = compare(q.sortBy ?? "added");
			out.sort(q.descending ? (x, y) => cmp(y, x) : cmp);
			return out.map(plain);
		}
		case "get_book":
			return full(find(s, a.id));
		case "get_categories":
			await sleep(20);
			return categories(s, a.search, a.scope ?? "ebooks");
		case "completions": {
			const set = new Set<string>();
			for (const b of s.books) {
				const f = a.field as string;
				if (f === "authors") b.authors.forEach((x) => set.add(x));
				else if (f === "tags") b.tags.forEach((x) => set.add(x));
				else if (f === "languages") b.languages.forEach((x) => set.add(x));
				else if (f === "series" && b.series) set.add(b.series);
				else if (f === "publisher" && b.publisher) set.add(b.publisher);
			}
			return [...set].sort(collator.compare);
		}
		case "saved_searches":
			return s.saved;
		case "set_saved_searches":
			s.saved = a.searches;
			return null;
		case "update_book": {
			const b = find(s, a.id);
			apply(b, a.patch);
			return full(b);
		}
		case "bulk_update": {
			const p = a.patch as BulkPatch;
			(a.ids as number[]).forEach((id, i) => {
				const b = find(s, id);
				const patch: BookPatch = {};
				if (p.authors) patch.authors = p.authors;
				if (p.series !== undefined) {
					patch.series = p.series;
					if (p.seriesStart !== undefined) patch.seriesIndex = p.seriesStart + i;
				}
				let tags = p.clearTags ? [] : [...b.tags];
				tags = tags.filter((t) => !p.removeTags?.includes(t));
				for (const t of p.addTags ?? []) if (!tags.includes(t)) tags.push(t);
				if (p.clearTags || p.addTags?.length || p.removeTags?.length) patch.tags = tags;
				if (p.publisher !== undefined) patch.publisher = p.publisher;
				if (p.pubdate !== undefined) patch.pubdate = p.pubdate;
				if (p.languages) patch.languages = p.languages;
				if (p.rating !== undefined) patch.rating = p.rating;
				apply(b, patch);
			});
			return null;
		}
		case "set_cover_from_file":
		case "set_cover_from_url": {
			const b = find(s, a.id);
			b.hasCover = true;
			b.lastModified = new Date().toISOString();
			return null;
		}
		case "remove_cover": {
			const b = find(s, a.id);
			b.hasCover = false;
			b.lastModified = new Date().toISOString();
			return null;
		}
		case "embed_metadata":
			await sleep(400);
			return (a.ids as number[]).length;
		case "add_books": {
			await sleep(500);
			const out: AddOutcome[] = [];
			for (const path of a.paths as string[]) {
				const ext = path.split(".").pop()?.toLowerCase() ?? "";
				if (!EBOOK_EXTENSIONS.includes(ext)) {
					out.push({ path, bookId: null, error: `unsupported format .${ext}` });
					continue;
				}
				const title = baseName(path).replace(/\.[^.]+$/, "").replace(/[_-]+/g, " ");
				const id = s.nextId++;
				const stamp = new Date().toISOString();
				const b: MockBook = {
					id, title, sort: sortKey(title), authors: ["Unknown"], authorSort: "Unknown", series: null, seriesIndex: 1, tags: [],
					publisher: null, languages: [], rating: null, formats: [ext.toUpperCase()], identifiers: {}, hasCover: false, size: 800000,
					timestamp: stamp, pubdate: null, lastModified: stamp, comments: null, uuid: crypto.randomUUID(), path: `Unknown/${title} (${id})`,
					files: [{ format: ext.toUpperCase(), fileName: title, size: 800000 }], hay: "", ...EBOOK_ONLY
				};
				b.hay = makeHay(b);
				s.books.push(b);
				out.push({ path, bookId: id, error: null });
			}
			return out;
		}
		case "add_format": {
			const b = find(s, a.id);
			const ext = (a.path as string).split(".").pop()!.toUpperCase();
			if (!b.formats.includes(ext)) {
				b.formats.push(ext);
				b.files.push({ format: ext, fileName: baseName(a.path), size: 1200000 });
				b.size += 1200000;
			}
			return full(b);
		}
		case "remove_format": {
			const b = find(s, a.id);
			b.formats = b.formats.filter((f) => f !== a.format);
			b.files = b.files.filter((f) => f.format !== a.format);
			b.size = b.files.reduce((x, f) => x + f.size, 0);
			return full(b);
		}
		case "delete_books": {
			await sleep(150);
			const ids = new Set(a.ids as number[]);
			s.books = s.books.filter((b) => !ids.has(b.id));
			return null;
		}
		case "show_in_folder":
		case "open_externally":
			console.info("[mock]", cmd, args);
			return null;
		case "best_metadata": {
			await sleep(400);
			const b = find(s, a.id);
			return b.id % 4 === 0 ? null : fakeResults({ title: b.title, authors: b.authors, isbn: null })[0];
		}
		case "identify":
			await sleep(700);
			return fakeResults(a.query);
		case "detect_devices":
			await sleep(300);
			return [
				{
					id: "kobo-libra", name: "Kobo Libra Colour", kind: "kobo", root: "/media/reader/KOBOeReader", booksDir: "/media/reader/KOBOeReader/Books",
					totalBytes: 32e9, freeBytes: 21.4e9, supportedFormats: ["EPUB", "KEPUB", "PDF", "CBZ"],
					info: [
						{ label: "Model", value: "Kobo Libra Colour" },
						{ label: "Firmware", value: "4.40.23081" },
						{ label: "Serial number", value: "N4281A0123456" }
					]
				} satisfies Device,
				{
					id: "kindle-scribe", name: "Kindle Scribe", kind: "kindle", root: "/media/Kindle", booksDir: "/media/Kindle/documents",
					totalBytes: 16e9, freeBytes: 11.8e9, supportedFormats: ["AZW3", "MOBI", "PDF", "TXT"],
					info: [
						{ label: "Model", value: "Kindle Scribe (1st generation)" },
						{ label: "Firmware", value: "5.17.3.1" },
						{ label: "Serial number", value: "G0Y1 2345 6789 ABCD" },
						{ label: "File system", value: "exFAT" },
						{ label: "Mount point", value: "/media/Kindle" },
						{ label: "Device UUID", value: "9f2c1d7e-3a4b-4c55-8e0a-6b7d1f2e3a49-long-value-example" }
					]
				} satisfies Device,
				{
					id: "folder-backup", name: "Backup folder", kind: "folder", root: "/mnt/nas/ebook-backup", booksDir: "/mnt/nas/ebook-backup",
					totalBytes: 500e9, freeBytes: 120e9, supportedFormats: ["EPUB", "PDF", "MOBI", "AZW3"], info: []
				} satisfies Device,
				{
					id: "folder-usb-full", name: "USB stick (almost full)", kind: "folder", root: "/media/reader/USB-full", booksDir: "/media/reader/USB-full",
					totalBytes: 16e9, freeBytes: 50e6, supportedFormats: ["EPUB", "PDF"], info: []
				} satisfies Device
			];
		case "add_folder_device": {
			const id = `folder-${hashString(a.path)}`;
			s.deviceFiles.set(id, []);
			return { id, name: baseName(a.path), kind: "folder", root: a.path, booksDir: a.path, totalBytes: 500e9, freeBytes: 120e9, supportedFormats: ["EPUB", "PDF", "MOBI", "AZW3"], info: [] } satisfies Device;
		}
		case "device_books":
			return s.deviceFiles.get(a.deviceId) ?? [];
		case "send_to_device": {
			await sleep(500);
			const list = s.deviceFiles.get(a.deviceId) ?? [];
			const kindle = a.deviceId === "kindle-scribe";
			const out: SendOutcome[] = (a.ids as number[]).map((id) => {
				const b = find(s, id);
				if (b.formats.length === 1 && b.formats[0] === "FB2") return { bookId: id, book: null, converted: false, error: "no supported format for this device" };
				const converted = kindle && !b.formats.includes("AZW3") && !b.formats.includes("MOBI") && !b.formats.includes("PDF");
				const format = kindle ? (converted ? "AZW3" : b.formats.includes("PDF") ? "PDF" : b.formats[0]) : "EPUB";
				const ext = format.toLowerCase();
				const dir = kindle ? "/media/Kindle/documents" : "/media/reader/KOBOeReader/Books";
				const book: DeviceBook = {
					path: `${dir}/${b.title}.${ext}`, relPath: `${dir.split("/").pop()}/${b.title}.${ext}`, title: b.title, authors: b.authors,
					format, size: b.files[0].size, modified: new Date().toISOString(), uuid: b.uuid
				};
				const at = list.findIndex((x) => x.path === book.path);
				if (at >= 0) list[at] = book;
				else list.push(book);
				return { bookId: id, book, converted, error: null };
			});
			s.deviceFiles.set(a.deviceId, list);
			return out;
		}
		case "import_from_device": {
			await sleep(900);
			const out: AddOutcome[] = [];
			for (const path of a.paths as string[]) {
				const db = (s.deviceFiles.get(a.deviceId) ?? []).find((x) => x.path === path);
				if (!db || /Kafka|Sapiens/.test(db.title)) {
					out.push({ path, bookId: null, error: db ? "file is DRM-protected and cannot be read" : "file not found on device" });
					continue;
				}
				const id = s.nextId++;
				const stamp = new Date().toISOString();
				const authors = db.authors.length ? db.authors : ["Unknown"];
				const b: MockBook = {
					id, title: db.title, sort: sortKey(db.title), authors, authorSort: authors[0], series: null, seriesIndex: 1, tags: [],
					publisher: null, languages: [], rating: null, formats: [db.format], identifiers: {}, hasCover: false, size: db.size,
					timestamp: stamp, pubdate: null, lastModified: stamp, comments: null, uuid: crypto.randomUUID(), path: `${authors[0]}/${db.title} (${id})`,
					files: [{ format: db.format, fileName: db.title, size: db.size }], hay: "", ...EBOOK_ONLY
				};
				b.hay = makeHay(b);
				s.books.push(b);
				out.push({ path, bookId: id, error: null });
			}
			return out;
		}
		case "delete_device_book":
			s.deviceFiles.set(a.deviceId, (s.deviceFiles.get(a.deviceId) ?? []).filter((b) => b.path !== a.path));
			return null;
		case "reading_progress":
			return s.progress;
		case "reading_state":
			return s.reading.get(a.id) ?? null;
		case "set_reading_state": {
			const st = a.state as ReadingState;
			s.reading.set(st.bookId, st);
			s.progress[st.bookId] = st.progress;
			return null;
		}
		case "annotations":
			return s.annotations.filter((x) => x.bookId === a.id);
		case "save_annotation": {
			const an = a.annotation as Annotation;
			const i = s.annotations.findIndex((x) => x.id === an.id);
			if (i >= 0) s.annotations[i] = an;
			else s.annotations.push(an);
			return null;
		}
		case "delete_annotation":
			s.annotations = s.annotations.filter((x) => x.id !== a.id);
			return null;
		case "audio_status":
			return audioStatus(s);
		case "audio_add_root": {
			await sleep(100);
			const inLibrary = a.inLibrary as boolean;
			const path = inLibrary ? "Audiobooks" : (a.path as string | null);
			if (!path) throw "No folder was chosen";
			if (s.audio.roots.some((r) => r.path === path)) throw `Already added: ${path}`;
			const root: AudioRoot = {
				id: s.audio.nextRoot++,
				path,
				inLibrary,
				managed: inLibrary,
				layout: inLibrary ? "author-title" : "libation",
				label: null,
				enabled: true,
				lastScan: null,
				lastScanOk: null,
				audiobooks: 0,
				missing: 0
			};
			s.audio.roots.push(root);
			return root;
		}
		case "audio_update_root": {
			const root = s.audio.roots.find((r) => r.id === a.id);
			if (!root) throw `location ${a.id} not found`;
			const p = a.patch as RootPatch;
			if (p.path !== undefined) root.path = p.path;
			if (p.managed !== undefined) root.managed = p.managed;
			if (p.layout !== undefined) root.layout = p.layout;
			if (p.label !== undefined) root.label = p.label.trim() || null;
			if (p.enabled !== undefined) root.enabled = p.enabled;
			return rootInfo(s.audio, root);
		}
		case "audio_forget_missing": {
			const gone = new Set(s.audio.books.filter((x) => x.rootId === a.id && x.state === "missing").map((x) => x.id));
			s.audio.books = s.audio.books.filter((x) => !gone.has(x.id));
			s.audio.links = s.audio.links.filter((l) => !gone.has(l.audiobookId));
			return gone.size;
		}
		case "audio_remove_root":
			for (const x of s.audio.books) if (x.rootId === a.id) x.rootId = null;
			s.audio.roots = s.audio.roots.filter((r) => r.id !== a.id);
			return null;
		case "audio_scan":
			return startScan(s, a.rootIds, a.onEvent);
		case "job_cancel": {
			const job = s.audio.jobs.get(a.id);
			if (job) job.cancelled = true;
			return null;
		}
		case "audiobook":
			return audiobookInfo(s, a.id);
		case "audio_link":
			find(s, a.bookId);
			s.audio.links = s.audio.links.filter((l) => l.audiobookId !== a.audiobookId);
			s.audio.links.push({ audiobookId: a.audiobookId, bookId: a.bookId, state: "manual" });
			return null;
		case "audio_unlink":
			s.audio.links = s.audio.links.filter((l) => !(l.audiobookId === a.audiobookId && l.bookId === a.bookId));
			s.audio.links.push({ audiobookId: a.audiobookId, bookId: a.bookId, state: "rejected" });
			return null;
		case "audio_reset_link":
			s.audio.links = s.audio.links.filter((l) => l.audiobookId !== a.audiobookId);
			return null;
		case "audio_transfer_plan":
			await sleep(120);
			return transferPlan(s, a.ids, a.destRootId ?? null, a.destFolder ?? null);
		case "audio_transfer":
			return startTransfer(s, a);
		case "audio_show_in_folder":
		case "audio_open":
			audioFile(s, a.id);
			console.info("[mock]", cmd, args);
			return null;
		default:
			throw `no mock for ${cmd}`;
	}
}
