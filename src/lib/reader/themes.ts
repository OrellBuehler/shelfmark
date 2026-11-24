import type { FontChoice, Prefs, ThemeName } from "./prefs.svelte";

export interface Palette {
	bg: string;
	fg: string;
	link: string;
	muted: string;
	selection: string;
	highlightBlend: string;
	highlightOpacity: string;
}

export const palettes: Record<ThemeName, Palette> = {
	light: {
		bg: "#fbfaf7",
		fg: "#1f1c18",
		link: "#2455a4",
		muted: "#7a746b",
		selection: "rgba(70, 130, 220, 0.28)",
		highlightBlend: "multiply",
		highlightOpacity: "0.45"
	},
	sepia: {
		bg: "#f4ecd9",
		fg: "#4b3a28",
		link: "#8a4b1c",
		muted: "#8f7b62",
		selection: "rgba(180, 110, 40, 0.28)",
		highlightBlend: "multiply",
		highlightOpacity: "0.5"
	},
	dark: {
		bg: "#1b1a18",
		fg: "#d8d4cb",
		link: "#8db4f0",
		muted: "#8d887e",
		selection: "rgba(110, 160, 240, 0.35)",
		highlightBlend: "normal",
		highlightOpacity: "0.38"
	}
};

export const highlightColors = {
	yellow: "#f2c94c",
	green: "#6fcf97",
	blue: "#56a4f5",
	pink: "#f27ca5",
	purple: "#b58cf0"
} as const;

export type HighlightColor = keyof typeof highlightColors;

export function colorValue(name: string | null | undefined) {
	return highlightColors[(name ?? "yellow") as HighlightColor] ?? highlightColors.yellow;
}

const fonts: Record<Exclude<FontChoice, "publisher">, string> = {
	serif: `"Iowan Old Style", "Charter", "Source Serif 4", "Palatino Linotype", "Book Antiqua", Georgia, "Noto Serif", serif`,
	sans: `"Inter Variable", Inter, -apple-system, "Segoe UI", "Helvetica Neue", "Noto Sans", sans-serif`,
	dyslexic: `"OpenDyslexic", "Lexend", "Atkinson Hyperlegible", Verdana, "Trebuchet MS", "Noto Sans", sans-serif`
};

const textSelector = `body :not(pre):not(code):not(kbd):not(samp):not(tt):not(svg, svg *)`;

export function buildCss(p: Prefs, theme: ThemeName) {
	const c = palettes[theme];
	const dark = theme !== "light";
	const family = p.font === "publisher" ? "" : fonts[p.font];
	const dyslexic = p.font === "dyslexic";
	const spacing = dyslexic ? Math.max(p.lineHeight, 1.75) : p.lineHeight;

	return `
@namespace epub "http://www.idpf.org/2007/ops";
html {
	color-scheme: ${theme === "dark" ? "dark" : "light"};
	background: ${c.bg} !important;
	color: ${c.fg};
	font-size: ${p.fontSize}px !important;
	-webkit-text-size-adjust: 100%;
	text-rendering: optimizeLegibility;
	-webkit-font-smoothing: antialiased;
	font-kerning: normal;
	font-variant-ligatures: common-ligatures;
}
body {
	background: transparent !important;
	color: ${c.fg};
	font-size: 1em !important;
	${family ? `font-family: ${family} !important;` : ""}
}
${family ? `${textSelector} { font-family: ${family} !important; }` : ""}
${dyslexic ? `body { letter-spacing: 0.04em; word-spacing: 0.12em; }` : ""}
${
	dark
		? `${textSelector}:not(a) { color: inherit !important; background-color: transparent !important; border-color: ${c.muted}; }
body hr { border-color: ${c.muted} !important; }
img:not([src$=".svg"]) { filter: brightness(${theme === "dark" ? 0.88 : 0.97}); }`
		: ""
}
a:link, a:visited, a:link *, a:visited * { color: ${c.link} !important; }
::selection { background: ${c.selection}; }
:where(body, p, li, blockquote, dd, dt, div, td, th, figcaption) { line-height: ${spacing} !important; }
p, li, blockquote, dd {
	text-align: ${p.justify ? "justify" : "start"};
	-webkit-hyphens: ${p.hyphenate ? "auto" : "manual"};
	hyphens: ${p.hyphenate ? "auto" : "manual"};
	-webkit-hyphenate-limit-before: 3;
	-webkit-hyphenate-limit-after: 2;
	hanging-punctuation: allow-end last;
	widows: 2;
	orphans: 2;
}
[align="left"] { text-align: left; }
[align="right"] { text-align: right; }
[align="center"] { text-align: center; }
[align="justify"] { text-align: justify; }
pre { white-space: pre-wrap !important; }
aside[epub|type~="endnote"],
aside[epub|type~="footnote"],
aside[epub|type~="note"],
aside[epub|type~="rearnote"] { display: none; }
`;
}
