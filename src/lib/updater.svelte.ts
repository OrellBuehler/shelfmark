import { toast } from "svelte-sonner";
import type { Update } from "@tauri-apps/plugin-updater";
import { inTauri } from "./api";

export const RELEASES_URL = "https://github.com/OrellBuehler/shelfmark/releases/latest";

const mockUpdate =
	!inTauri && typeof location !== "undefined" && new URLSearchParams(location.search).has("update");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const message = (e: unknown) => (typeof e === "string" ? e : ((e as Error)?.message ?? String(e)));

class Updater {
	current = $state("");
	portable = $state(false);
	available = $state<{ version: string; notes: string } | null>(null);
	checking = $state(false);
	phase = $state<"idle" | "downloading" | "installing" | "failed">("idle");
	progress = $state<number | null>(null);
	error = $state("");
	open = $state(false);
	#update: Update | null = null;
	#started = false;

	get busy() {
		return this.phase === "downloading" || this.phase === "installing";
	}

	async init() {
		if (this.#started) return;
		this.#started = true;
		if (inTauri) {
			const [{ getVersion }, { invoke }] = await Promise.all([
				import("@tauri-apps/api/app"),
				import("@tauri-apps/api/core")
			]);
			this.current = await getVersion();
			this.portable = await invoke<boolean>("is_portable");
			if (import.meta.env.DEV) return;
		} else {
			this.current = "1.0.0";
			if (!mockUpdate) return;
		}
		setTimeout(() => void this.check(false), 3000);
	}

	async check(manual: boolean) {
		if (this.checking || this.busy) return;
		if (this.available) {
			this.open = true;
			return;
		}
		this.checking = true;
		try {
			if (inTauri) {
				const { check } = await import("@tauri-apps/plugin-updater");
				this.#update = await check();
				this.available = this.#update
					? { version: this.#update.version, notes: this.#update.body ?? "" }
					: null;
			} else {
				await sleep(600);
				this.available = mockUpdate
					? { version: "1.1.0", notes: "- Automatic updates\n- Faster cover loading\n- Fixes for PDF metadata" }
					: null;
			}
			if (!this.available) {
				if (manual) toast.success(`Shelfmark ${this.current} is up to date`);
			} else if (manual) {
				this.open = true;
			} else {
				toast(`Shelfmark ${this.available.version} is available`, {
					duration: 15000,
					action: { label: "View", onClick: () => (this.open = true) }
				});
			}
		} catch (e) {
			if (manual) toast.error(`Could not check for updates: ${message(e)}`);
		} finally {
			this.checking = false;
		}
	}

	async install() {
		if (this.busy || !this.available) return;
		this.phase = "downloading";
		this.progress = null;
		this.error = "";
		try {
			if (!this.#update) {
				for (let i = 0; i <= 20; i++) {
					this.progress = i / 20;
					await sleep(120);
				}
				this.phase = "installing";
				await sleep(1200);
				throw "Updates cannot be installed in the browser preview";
			}
			let total = 0;
			let done = 0;
			await this.#update.downloadAndInstall((ev) => {
				if (ev.event === "Started") total = ev.data.contentLength ?? 0;
				else if (ev.event === "Progress") {
					done += ev.data.chunkLength;
					this.progress = total ? Math.min(1, done / total) : null;
				} else this.phase = "installing";
			});
			const { invoke } = await import("@tauri-apps/api/core");
			await invoke("restart");
		} catch (e) {
			this.phase = "failed";
			this.error = message(e);
		}
	}
}

export const updater = new Updater();
