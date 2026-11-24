import { api, inTauri, type Annotation, type BookId, type ReadingState } from "$lib/api";

const stateKey = (id: BookId) => `shelfmark.dev.reader.state.${id}`;
const notesKey = (id: BookId) => `shelfmark.dev.reader.notes.${id}`;

function readLocal<T>(key: string, fallback: T): T {
	try {
		const raw = localStorage.getItem(key);
		return raw ? (JSON.parse(raw) as T) : fallback;
	} catch {
		return fallback;
	}
}

function writeLocal(key: string, value: unknown) {
	try {
		localStorage.setItem(key, JSON.stringify(value));
	} catch {
		// storage unavailable
	}
}

export async function loadState(id: BookId): Promise<ReadingState | null> {
	try {
		return await api.readingState(id);
	} catch (e) {
		if (inTauri) console.error(e);
		return inTauri ? null : readLocal<ReadingState | null>(stateKey(id), null);
	}
}

export async function saveState(state: ReadingState) {
	try {
		await api.setReadingState(state);
	} catch (e) {
		if (inTauri) console.error(e);
		else writeLocal(stateKey(state.bookId), { ...state, updated: new Date().toISOString() });
	}
}

export async function loadAnnotations(id: BookId): Promise<Annotation[]> {
	try {
		return await api.annotations(id);
	} catch (e) {
		if (inTauri) console.error(e);
		return inTauri ? [] : readLocal<Annotation[]>(notesKey(id), []);
	}
}

export async function saveAnnotation(a: Annotation) {
	try {
		await api.saveAnnotation(a);
	} catch (e) {
		if (inTauri) {
			console.error(e);
			return;
		}
		const list = readLocal<Annotation[]>(notesKey(a.bookId), []);
		const i = list.findIndex((x) => x.id === a.id);
		if (i >= 0) list[i] = a;
		else list.push(a);
		writeLocal(notesKey(a.bookId), list);
	}
}

export async function deleteAnnotation(a: Annotation) {
	try {
		await api.deleteAnnotation(a.id);
	} catch (e) {
		if (inTauri) {
			console.error(e);
			return;
		}
		writeLocal(
			notesKey(a.bookId),
			readLocal<Annotation[]>(notesKey(a.bookId), []).filter((x) => x.id !== a.id)
		);
	}
}
