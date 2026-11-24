export type ThemeChoice = "auto" | "light" | "sepia" | "dark";
export type ThemeName = "light" | "sepia" | "dark";
export type FontChoice = "publisher" | "serif" | "sans" | "dyslexic";
export type Flow = "paginated" | "scrolled";
export type Zoom = "fit-page" | "fit-width";

export interface Prefs {
	theme: ThemeChoice;
	font: FontChoice;
	fontSize: number;
	lineHeight: number;
	margin: number;
	lineWidth: number;
	justify: boolean;
	hyphenate: boolean;
	flow: Flow;
	columns: 1 | 2;
	zoom: Zoom;
}

export const defaultPrefs: Prefs = {
	theme: "auto",
	font: "serif",
	fontSize: 19,
	lineHeight: 1.6,
	margin: 7,
	lineWidth: 680,
	justify: true,
	hyphenate: true,
	flow: "paginated",
	columns: 2,
	zoom: "fit-page"
};

export const limits = {
	fontSize: { min: 12, max: 36, step: 1 },
	lineHeight: { min: 1.2, max: 2.2, step: 0.05 },
	margin: { min: 2, max: 16, step: 1 },
	lineWidth: { min: 440, max: 1100, step: 20 }
};

const KEY = "shelfmark.reader.prefs";

function load(): Prefs {
	try {
		const raw = localStorage.getItem(KEY);
		if (raw) return { ...defaultPrefs, ...JSON.parse(raw) };
	} catch {
		// storage unavailable
	}
	return { ...defaultPrefs };
}

class PrefsStore {
	value = $state<Prefs>(load());

	set<K extends keyof Prefs>(key: K, v: Prefs[K]) {
		this.value[key] = v;
		try {
			localStorage.setItem(KEY, JSON.stringify(this.value));
		} catch {
			// storage unavailable
		}
	}

	reset() {
		this.value = { ...defaultPrefs, theme: this.value.theme };
		try {
			localStorage.setItem(KEY, JSON.stringify(this.value));
		} catch {
			// storage unavailable
		}
	}
}

export const prefs = new PrefsStore();
