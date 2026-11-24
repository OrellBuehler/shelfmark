import { toast } from "svelte-sonner";
import { inTauri } from "./api";

export async function openLink(url: string) {
	if (!/^(https?:|mailto:)/i.test(url)) return;
	if (!inTauri) {
		window.open(url, "_blank", "noopener,noreferrer");
		return;
	}
	try {
		const { openUrl } = await import("@tauri-apps/plugin-opener");
		await openUrl(url);
	} catch (e) {
		toast.error(`Could not open link: ${typeof e === "string" ? e : (e as Error).message}`);
	}
}
