import { coverUrl, inTauri, type BookSummary } from "./api";
import { hashString } from "./format";

const cache = new Map<string, string>();

function esc(s: string) {
	return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

function wrap(text: string, max: number, lines: number): string[] {
	const out: string[] = [];
	let cur = "";
	for (const w of text.split(/\s+/)) {
		if ((cur + " " + w).trim().length > max && cur) {
			out.push(cur);
			cur = w;
		} else cur = (cur + " " + w).trim();
	}
	if (cur) out.push(cur);
	if (out.length > lines) {
		out.length = lines;
		out[lines - 1] += "…";
	}
	return out;
}

export function generatedCover(title: string, author: string, seed: number): string {
	const key = `${seed}|${title}|${author}`;
	const hit = cache.get(key);
	if (hit) return hit;
	const hue = seed % 360;
	const variant = (seed >>> 9) % 5;
	const bg = `hsl(${hue} 38% 30%)`;
	const bg2 = `hsl(${(hue + 28) % 360} 45% 22%)`;
	const accent = `hsl(${(hue + 160) % 360} 60% 68%)`;
	const light = `hsl(${hue} 40% 92%)`;
	let art = "";
	if (variant === 0) {
		art = `<circle cx="100" cy="190" r="62" fill="${accent}" opacity=".85"/><circle cx="100" cy="190" r="38" fill="${bg2}"/>`;
	} else if (variant === 1) {
		for (let i = 0; i < 9; i++)
			art += `<rect x="${-20 + i * 28}" y="150" width="12" height="200" transform="rotate(25 100 200)" fill="${accent}" opacity="${0.25 + (i % 3) * 0.2}"/>`;
	} else if (variant === 2) {
		art = `<path d="M0 230 Q50 190 100 225 T200 215 V300 H0Z" fill="${accent}" opacity=".7"/><path d="M0 255 Q60 225 110 252 T200 245 V300 H0Z" fill="${bg2}" opacity=".9"/>`;
	} else if (variant === 3) {
		for (let r = 0; r < 4; r++)
			for (let c = 0; c < 7; c++)
				art += `<circle cx="${28 + c * 24}" cy="${168 + r * 22}" r="${2 + ((r + c) % 3)}" fill="${accent}" opacity=".8"/>`;
	} else {
		art = `<rect x="30" y="160" width="140" height="2" fill="${accent}"/><rect x="30" y="168" width="90" height="2" fill="${accent}" opacity=".6"/><polygon points="100,178 148,246 52,246" fill="${accent}" opacity=".85"/>`;
	}
	const lines = wrap(title, 13, 4);
	const tspans = lines
		.map((l, i) => `<tspan x="100" dy="${i === 0 ? 0 : 25}">${esc(l)}</tspan>`)
		.join("");
	const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 300"><defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${bg}"/><stop offset="1" stop-color="${bg2}"/></linearGradient></defs><rect width="200" height="300" fill="url(#g)"/>${art}<rect x="0" y="0" width="7" height="300" fill="#000" opacity=".25"/><text x="100" y="62" text-anchor="middle" font-family="Georgia,'Times New Roman',serif" font-weight="700" font-size="22" fill="${light}">${tspans}</text><text x="100" y="270" text-anchor="middle" font-family="Georgia,serif" font-size="10.5" letter-spacing="1.6" fill="${light}" opacity=".85">${esc(author.toUpperCase().slice(0, 28))}</text></svg>`;
	const uri = `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
	if (cache.size > 4000) cache.clear();
	cache.set(key, uri);
	return uri;
}

export function coverSrc(
	book: Pick<BookSummary, "id" | "lastModified" | "title" | "authors" | "hasCover" | "kind" | "audiobookId" | "pairedBookId">,
	width = 400
): string {
	if (inTauri) return coverUrl(book, width);
	return generatedCover(book.title, book.authors[0] ?? "", hashString(book.title + book.id));
}

export function placeholderHue(title: string): number {
	return hashString(title) % 360;
}
