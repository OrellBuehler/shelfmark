export const LANGUAGES: [string, string][] = [
	["eng", "English"],
	["deu", "German"],
	["fra", "French"],
	["spa", "Spanish"],
	["ita", "Italian"],
	["por", "Portuguese"],
	["nld", "Dutch"],
	["swe", "Swedish"],
	["nor", "Norwegian"],
	["dan", "Danish"],
	["fin", "Finnish"],
	["isl", "Icelandic"],
	["pol", "Polish"],
	["ces", "Czech"],
	["slk", "Slovak"],
	["hun", "Hungarian"],
	["ron", "Romanian"],
	["bul", "Bulgarian"],
	["rus", "Russian"],
	["ukr", "Ukrainian"],
	["ell", "Greek"],
	["tur", "Turkish"],
	["heb", "Hebrew"],
	["ara", "Arabic"],
	["fas", "Persian"],
	["hin", "Hindi"],
	["ben", "Bengali"],
	["tha", "Thai"],
	["vie", "Vietnamese"],
	["ind", "Indonesian"],
	["zho", "Chinese"],
	["jpn", "Japanese"],
	["kor", "Korean"],
	["lat", "Latin"],
	["cat", "Catalan"],
	["eus", "Basque"],
	["glg", "Galician"],
	["hrv", "Croatian"],
	["srp", "Serbian"],
	["slv", "Slovenian"],
	["lit", "Lithuanian"],
	["lav", "Latvian"],
	["est", "Estonian"],
	["gle", "Irish"],
	["cym", "Welsh"],
	["afr", "Afrikaans"],
	["swa", "Swahili"],
	["gsw", "Swiss German"],
	["roh", "Romansh"],
	["epo", "Esperanto"]
];

const TWO_TO_THREE: Record<string, string> = {
	en: "eng", de: "deu", fr: "fra", es: "spa", it: "ita", pt: "por", nl: "nld", sv: "swe", no: "nor",
	da: "dan", fi: "fin", is: "isl", pl: "pol", cs: "ces", sk: "slk", hu: "hun", ro: "ron", bg: "bul",
	ru: "rus", uk: "ukr", el: "ell", tr: "tur", he: "heb", ar: "ara", fa: "fas", hi: "hin", bn: "ben",
	th: "tha", vi: "vie", id: "ind", zh: "zho", ja: "jpn", ko: "kor", la: "lat", ca: "cat", eu: "eus",
	gl: "glg", hr: "hrv", sr: "srp", sl: "slv", lt: "lit", lv: "lav", et: "est", ga: "gle", cy: "cym",
	af: "afr", sw: "swa", rm: "roh", eo: "epo"
};

const names = new Map(LANGUAGES);

export function languageName(code: string): string {
	return names.get(code.toLowerCase()) ?? code;
}

export function resolveLanguage(text: string): string | null {
	const t = text.trim().toLowerCase();
	if (!t) return null;
	for (const [code, name] of LANGUAGES) if (name.toLowerCase() === t || code === t) return code;
	if (TWO_TO_THREE[t]) return TWO_TO_THREE[t];
	const starts = LANGUAGES.filter(([, name]) => name.toLowerCase().startsWith(t));
	if (starts.length === 1) return starts[0][0];
	if (/^[a-z]{3}$/.test(t)) return t;
	return null;
}
