const ALLOWED = new Set([
	"a", "b", "strong", "i", "em", "u", "s", "strike", "del", "ins", "mark", "small", "sub", "sup",
	"p", "br", "hr", "div", "span", "blockquote", "pre", "code", "ul", "ol", "li", "dl", "dt", "dd",
	"h1", "h2", "h3", "h4", "h5", "h6", "table", "thead", "tbody", "tfoot", "tr", "td", "th", "caption"
]);

const DROP = new Set([
	"script", "style", "iframe", "frame", "frameset", "object", "embed", "link", "meta", "base",
	"form", "input", "button", "textarea", "select", "option", "noscript", "template", "svg", "math",
	"img", "picture", "video", "audio", "source", "canvas", "title", "head"
]);

function safeHref(href: string): boolean {
	return /^(https?:|mailto:)/i.test(href.trim());
}

function clean(node: Node, doc: Document) {
	for (const child of Array.from(node.childNodes)) {
		if (child.nodeType === Node.COMMENT_NODE) {
			child.remove();
			continue;
		}
		if (child.nodeType !== Node.ELEMENT_NODE) continue;
		const el = child as Element;
		const tag = el.tagName.toLowerCase();
		if (DROP.has(tag)) {
			el.remove();
			continue;
		}
		clean(el, doc);
		if (!ALLOWED.has(tag)) {
			el.replaceWith(...Array.from(el.childNodes));
			continue;
		}
		for (const attr of Array.from(el.attributes)) {
			const keep = tag === "a" && attr.name === "href" && safeHref(attr.value);
			if (!keep) el.removeAttribute(attr.name);
		}
		if (tag === "a") {
			el.setAttribute("rel", "noopener noreferrer");
			el.setAttribute("target", "_blank");
		}
	}
}

export function sanitizeHtml(html: string): string {
	const doc = new DOMParser().parseFromString(html, "text/html");
	clean(doc.body, doc);
	return doc.body.innerHTML;
}
