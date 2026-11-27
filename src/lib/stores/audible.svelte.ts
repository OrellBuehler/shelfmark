import { toast } from "svelte-sonner";
import { api, type AudibleStatus, type AudibleSyncResult, type JobEnd, type JobProgress } from "#lib/api";
import { errorMessage, plural } from "#lib/format";
import { audio } from "#lib/stores/audio.svelte";

const EMPTY: AudibleStatus = { source: null, detected: null, accounts: [], error: null, lastSync: null, titles: 0, notDownloaded: 0, imported: false };

interface SyncJob {
	progress: JobProgress | null;
	cancelling: boolean;
}

function report(end: JobEnd<AudibleSyncResult>) {
	if (end.event === "failed") return toast.error(`Audible sync failed: ${end.message}`);
	if (end.event === "cancelled") return toast.info("Audible sync cancelled");
	const r = end.result;
	const shown = r.failed.slice(0, 3).map((f) => `${f.name}: ${f.message}`);
	if (r.failed.length > shown.length) shown.push(`and ${r.failed.length - shown.length} more`);
	const description = shown.join("\n") || undefined;
	if (!r.accounts && r.failed.length) return toast.error("Audible sync failed", { description, duration: 10000 });
	const parts = [plural(r.titles, "title")];
	if (r.added) parts.push(`${r.added.toLocaleString()} new`);
	if (r.removed) parts.push(`${r.removed.toLocaleString()} removed`);
	const text = `Audible: ${parts.join(", ")}`;
	if (r.failed.length) toast.warning(text, { description, duration: 10000 });
	else toast.success(text);
}

class AudibleState {
	status = $state.raw<AudibleStatus>(EMPTY);
	job = $state.raw<SyncJob | null>(null);

	connected = $derived(this.status.source !== null);
	hasTitles = $derived(this.status.titles > 0);

	private running: { cancel: () => void } | null = null;
	private token = 0;

	reset() {
		this.token++;
		this.running?.cancel();
		this.running = null;
		this.job = null;
		this.status = EMPTY;
	}

	async load() {
		try {
			this.status = await api.audibleStatus();
		} catch (e) {
			toast.error(`Could not load Audible status: ${errorMessage(e)}`);
		}
	}

	async connect(path?: string) {
		try {
			this.status = await api.audibleConnect(path);
		} catch (e) {
			toast.error(`Could not connect to Audible: ${errorMessage(e)}`);
			return;
		}
		this.sync();
	}

	async disconnect() {
		try {
			this.status = await api.audibleDisconnect();
			await audio.refresh();
			toast.success("Disconnected from Audible", {
				description: "Not-downloaded titles were removed from Shelfmark. Your files were not touched."
			});
		} catch (e) {
			toast.error(`Could not disconnect: ${errorMessage(e)}`);
		}
	}

	sync() {
		if (this.job) return;
		const token = ++this.token;
		this.job = { progress: null, cancelling: false };
		const job = api.audibleSync((progress) => {
			if (token === this.token && this.job) this.job = { ...this.job, progress };
		});
		this.running = job;
		const finish = async (end: JobEnd<AudibleSyncResult>) => {
			if (token !== this.token) return;
			this.job = null;
			this.running = null;
			await this.load();
			await audio.refresh();
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

export const audible = new AudibleState();
