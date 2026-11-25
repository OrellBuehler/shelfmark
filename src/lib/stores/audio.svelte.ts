import { toast } from "svelte-sonner";
import { api, type AudioStatus, type JobEnd, type JobProgress, type RootPatch } from "#lib/api";
import { errorMessage, plural } from "#lib/format";

const EMPTY: AudioStatus = { roots: [], audiobooks: 0, issues: [] };

interface ScanJob {
	progress: JobProgress | null;
	cancelling: boolean;
}

function report(end: JobEnd) {
	if (end.event === "failed") return toast.error(`Scan failed: ${end.message}`);
	if (end.event === "cancelled") return toast.info("Scan cancelled");
	const r = end.result;
	if (!r.complete) {
		return toast.warning("Scan incomplete: a location could not be reached", {
			description: `${plural(r.audiobooks, "audiobook")} found. Nothing was marked as missing. Check that the folder or drive is connected and scan again.`,
			duration: 10000
		});
	}
	const parts = [plural(r.audiobooks, "audiobook")];
	if (r.added) parts.push(`${r.added.toLocaleString()} new`);
	if (r.updated) parts.push(`${r.updated.toLocaleString()} updated`);
	if (r.missing) parts.push(`${r.missing.toLocaleString()} missing`);
	toast.success(`Scan finished: ${parts.join(", ")}`, {
		description: r.issues ? `${plural(r.issues, "problem")} found. See Settings for details.` : undefined
	});
}

class AudioState {
	status = $state.raw<AudioStatus>(EMPTY);
	job = $state.raw<ScanJob | null>(null);

	hasLocation = $derived(this.status.roots.length > 0);
	hasAudio = $derived(this.status.audiobooks > 0);

	onChanged: (() => void) | null = null;

	private running: { cancel: () => void } | null = null;
	private token = 0;

	reset() {
		this.token++;
		this.running?.cancel();
		this.running = null;
		this.job = null;
		this.status = EMPTY;
	}

	fail(what: string, e: unknown) {
		toast.error(`${what}: ${errorMessage(e)}`);
	}

	async load() {
		try {
			this.status = await api.audioStatus();
		} catch (e) {
			this.fail("Could not load audiobook locations", e);
		}
	}

	async addRoot(inLibrary: boolean, path?: string) {
		try {
			const root = await api.audioAddRoot(inLibrary, path);
			await this.load();
			this.onChanged?.();
			this.scan([root.id]);
		} catch (e) {
			this.fail("Could not add location", e);
		}
	}

	async updateRoot(id: number, patch: RootPatch) {
		try {
			await api.audioUpdateRoot(id, patch);
			await this.load();
			this.onChanged?.();
		} catch (e) {
			this.fail("Could not update location", e);
			await this.load();
		}
	}

	async removeRoot(id: number) {
		try {
			await api.audioRemoveRoot(id);
			await this.load();
			this.onChanged?.();
			toast.success("Location removed", { description: "Your files were not touched." });
		} catch (e) {
			this.fail("Could not remove location", e);
		}
	}

	scan(rootIds?: number[]) {
		if (this.job) return;
		const token = ++this.token;
		this.job = { progress: null, cancelling: false };
		const job = api.audioScan(rootIds ?? null, (progress) => {
			if (token === this.token && this.job) this.job = { ...this.job, progress };
		});
		this.running = job;
		const finish = async (end: JobEnd) => {
			if (token !== this.token) return;
			this.job = null;
			this.running = null;
			await this.load();
			this.onChanged?.();
			report(end);
		};
		job.result.then(finish, (e) => finish({ event: "failed", message: errorMessage(e) }));
	}

	cancel() {
		if (!this.job || this.job.cancelling) return;
		this.job = { ...this.job, cancelling: true };
		this.running?.cancel();
	}
}

export const audio = new AudioState();
