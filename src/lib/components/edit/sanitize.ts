const ALLOWED = new Set([
	"p", "br", "b", "strong", "i", "em", "u", "s", "strike", "del", "sub", "sup", "ul", "ol", "li",
	"a", "h1", "h2", "h3", "h4", "h5", "h6", "blockquote", "div", "span", "hr", "code", "pre"
]);
const DROP = new Set([
	"script", "style", "iframe", "object", "embed", "link", "meta", "form", "input", "button",
	"textarea", "select", "svg", "math", "template", "noscript", "title", "head", "img", "video", "audio"
]);

function clean(node: Node) {
	for (const child of Array.from(node.childNodes)) {
		if (child.nodeType === Node.COMMENT_NODE) {
			child.remove();
		} else if (child.nodeType === Node.ELEMENT_NODE) {
			const el = child as Element;
			const tag = el.tagName.toLowerCase();
			if (DROP.has(tag)) {
				el.remove();
				continue;
			}
			clean(el);
			if (!ALLOWED.has(tag)) {
				el.replaceWith(...Array.from(el.childNodes));
				continue;
			}
			const href = tag === "a" ? (el.getAttribute("href") ?? "").trim() : "";
			if (tag === "a" && !/^(https?:|mailto:)/i.test(href)) {
				el.replaceWith(...Array.from(el.childNodes));
				continue;
			}
			for (const attr of Array.from(el.attributes)) el.removeAttribute(attr.name);
			if (tag === "a") {
				el.setAttribute("href", href);
				el.setAttribute("rel", "noopener noreferrer");
			}
		}
	}
}

export function sanitizeHtml(html: string | null | undefined): string {
	if (!html) return "";
	const doc = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
	clean(doc.body);
	return normalizeHtml(doc.body.innerHTML);
}

export function normalizeHtml(html: string): string {
	const t = html.trim();
	const text = t.replace(/<[^>]*>/g, "").replace(/&nbsp;/g, " ").trim();
	if (!text && !/<(hr|li)\b/i.test(t)) return "";
	return t;
}

export function htmlToText(html: string): string {
	if (!html) return "";
	const spaced = html.replace(/<(\/(p|li|div|h[1-6]|blockquote)|br\s*\/?)>/gi, " $&");
	const doc = new DOMParser().parseFromString(spaced, "text/html");
	return (doc.body.textContent ?? "").replace(/\s+/g, " ").trim();
}
