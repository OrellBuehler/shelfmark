const READ_ORDER = ["EPUB", "KEPUB", "AZW3", "MOBI", "PDF", "CBZ", "FB2"];

export const EBOOK_EXTENSIONS = [
	"epub",
	"kepub",
	"azw",
	"azw3",
	"mobi",
	"pdf",
	"cbz",
	"cbr",
	"fb2",
	"txt",
	"rtf",
	"docx",
	"djvu",
	"lit",
	"htmlz",
	"txtz"
];

export function pickReadFormat(formats: string[]): string | null {
	const upper = formats.map((f) => f.toUpperCase());
	for (const f of READ_ORDER) {
		const i = upper.indexOf(f);
		if (i >= 0) return formats[i];
	}
	return null;
}

export function formatSize(bytes: number): string {
	if (bytes < 1024) return `${bytes} B`;
	const units = ["KB", "MB", "GB", "TB"];
	let v = bytes / 1024;
	let i = 0;
	while (v >= 1024 && i < units.length - 1) {
		v /= 1024;
		i++;
	}
	return `${v >= 100 || i === 0 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

function parseDate(iso: string | null | undefined): Date | null {
	if (!iso) return null;
	const d = new Date(iso);
	if (isNaN(d.getTime()) || d.getUTCFullYear() < 1000) return null;
	return d;
}

export function formatDate(iso: string | null | undefined, style: "full" | "month" = "full"): string {
	const d = parseDate(iso);
	if (!d) return "";
	return d.toLocaleDateString(undefined, {
		year: "numeric",
		month: "short",
		day: style === "full" ? "numeric" : undefined,
		timeZone: "UTC"
	});
}

export function formatDateTime(iso: string | null | undefined): string {
	const d = parseDate(iso);
	if (!d) return "";
	return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

export function plural(n: number, one: string, many = one + "s"): string {
	return `${n.toLocaleString()} ${n === 1 ? one : many}`;
}

export function formatDuration(ms: number): string {
	const total = Math.round(ms / 60000);
	if (total < 1) return ms > 0 ? "< 1 min" : "";
	const h = Math.floor(total / 60);
	const m = total % 60;
	if (!h) return `${m} min`;
	return m ? `${h} h ${m} min` : `${h} h`;
}

export function formatClock(ms: number): string {
	const total = Math.floor(ms / 1000);
	const h = Math.floor(total / 3600);
	const m = Math.floor((total % 3600) / 60);
	const sec = String(total % 60).padStart(2, "0");
	return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

export function errorMessage(e: unknown): string {
	if (typeof e === "string") return e;
	if (e instanceof Error) return e.message;
	return String(e);
}

export function baseName(path: string): string {
	return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

const BIBLIOGRAPHIC: Record<string, string> = {
	ger: "deu",
	fre: "fra",
	dut: "nld",
	chi: "zho",
	cze: "ces",
	gre: "ell",
	ice: "isl",
	per: "fas",
	rum: "ron",
	slo: "slk",
	wel: "cym",
	baq: "eus",
	arm: "hye",
	geo: "kat",
	mac: "mkd",
	may: "msa",
	bur: "mya",
	alb: "sqi"
};

let displayNames: Intl.DisplayNames | null = null;

export function languageName(code: string): string {
	displayNames ??= new Intl.DisplayNames(["en"], { type: "language" });
	const c = BIBLIOGRAPHIC[code.toLowerCase()] ?? code;
	try {
		const name = displayNames.of(c);
		return name && name !== c ? name : code;
	} catch {
		return code;
	}
}

export interface IdentifierLink {
	label: string;
	value: string;
	urls: { name: string; url: string }[];
}

export function identifierLinks(ids: Record<string, string>): IdentifierLink[] {
	return Object.entries(ids).map(([type, value]) => {
		const t = type.toLowerCase();
		const v = encodeURIComponent(value);
		const urls: { name: string; url: string }[] = [];
		let label = type.toUpperCase();
		if (t === "isbn") {
			urls.push({ name: "Open Library", url: `https://openlibrary.org/isbn/${v}` });
			urls.push({ name: "WorldCat", url: `https://search.worldcat.org/search?q=bn:${v}` });
		} else if (t === "amazon" || t === "asin") {
			label = "Amazon";
			urls.push({ name: "Amazon", url: `https://www.amazon.com/dp/${v}` });
		} else if (t === "google") {
			label = "Google Books";
			urls.push({ name: "Google Books", url: `https://books.google.com/books?id=${v}` });
		} else if (t === "goodreads") {
			label = "Goodreads";
			urls.push({ name: "Goodreads", url: `https://www.goodreads.com/book/show/${v}` });
		} else if (t === "doi") {
			label = "DOI";
			urls.push({ name: "doi.org", url: `https://doi.org/${encodeURI(value)}` });
		}
		return { label, value, urls };
	});
}

export function hashString(s: string): number {
	let h = 2166136261;
	for (let i = 0; i < s.length; i++) {
		h ^= s.charCodeAt(i);
		h = Math.imul(h, 16777619);
	}
	return h >>> 0;
}

export function isEditable(target: EventTarget | null): boolean {
	if (!(target instanceof HTMLElement)) return false;
	return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}
