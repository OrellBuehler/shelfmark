export function humanSize(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
	const units = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
	const v = bytes / 1024 ** i;
	return `${v >= 100 || i === 0 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

const ARTICLES = ["the", "a", "an"];

export function autoTitleSort(title: string): string {
	const t = title.trim();
	const m = /^(\S+)\s+(.+)$/.exec(t);
	if (m && ARTICLES.includes(m[1].toLowerCase())) return `${m[2]}, ${m[1]}`;
	return t;
}

export function autoAuthorSort(authors: string[]): string {
	return authors
		.map((a) => {
			const name = a.trim();
			if (!name || name.includes(",")) return name;
			const parts = name.split(/\s+/);
			if (parts.length < 2) return name;
			const last = parts.pop() as string;
			return `${last}, ${parts.join(" ")}`;
		})
		.filter(Boolean)
		.join(" & ");
}

export function suggest(
	list: string[],
	query: string,
	exclude: string[] = [],
	limit = 8,
	labelOf: (v: string) => string = (v) => v
): string[] {
	const q = query.trim().toLowerCase();
	const skip = new Set(exclude.map((e) => e.toLowerCase()));
	const starts: string[] = [];
	const contains: string[] = [];
	for (const item of list) {
		if (skip.has(item.toLowerCase())) continue;
		const l = labelOf(item).toLowerCase();
		const v = item.toLowerCase();
		if (!q || l.startsWith(q) || v.startsWith(q)) starts.push(item);
		else if (l.includes(q) || v.includes(q)) contains.push(item);
		if (starts.length >= limit) break;
	}
	return [...starts, ...contains].slice(0, limit);
}
