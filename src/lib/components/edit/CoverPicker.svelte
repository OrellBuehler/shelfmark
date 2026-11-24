<script lang="ts">
	import ImageIcon from "@lucide/svelte/icons/image";
	import ImagePlusIcon from "@lucide/svelte/icons/image-plus";
	import Trash2Icon from "@lucide/svelte/icons/trash-2";
	import XIcon from "@lucide/svelte/icons/x";
	import { open as pickFile } from "@tauri-apps/plugin-dialog";
	import { toast } from "svelte-sonner";
	import { api, coverUrl, inTauri, type Book } from "$lib/api";
	import { Button } from "$lib/components/ui/button/index.js";

	let {
		book,
		pendingUrl = null,
		onchanged,
		ondiscardpending
	}: {
		book: Book;
		pendingUrl?: string | null;
		onchanged: () => void;
		ondiscardpending: () => void;
	} = $props();

	let busy = $state(false);
	let failed = $state<string | null>(null);

	const src = $derived(pendingUrl ?? (book.hasCover && inTauri ? coverUrl(book, 600) : null));
	const hasImage = $derived(!!src && failed !== src);

	async function choose() {
		if (!inTauri) {
			toast.info("Picking a file is only available in the desktop app");
			return;
		}
		const path = await pickFile({
			multiple: false,
			filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "gif"] }]
		});
		if (typeof path !== "string") return;
		busy = true;
		try {
			await api.setCoverFromFile(book.id, path);
			ondiscardpending();
			onchanged();
		} catch (e) {
			toast.error(`Could not set cover: ${e}`);
		} finally {
			busy = false;
		}
	}

	async function remove() {
		busy = true;
		try {
			if (!pendingUrl) await api.removeCover(book.id);
			ondiscardpending();
			onchanged();
		} catch (e) {
			toast.error(`Could not remove cover: ${e}`);
		} finally {
			busy = false;
		}
	}
</script>

<div class="space-y-3">
	<div
		class="bg-muted ring-foreground/10 relative aspect-[2/3] w-full overflow-hidden rounded-lg shadow-md ring-1"
	>
		{#if hasImage}
			<img
				{src}
				alt="Cover of {book.title}"
				class="size-full object-cover"
				onerror={() => (failed = src)}
			/>
		{:else}
			<div class="text-muted-foreground flex size-full flex-col items-center justify-center gap-2">
				<ImageIcon class="size-8 opacity-50" />
				<span class="text-xs">No cover</span>
			</div>
		{/if}
		{#if pendingUrl}
			<div
				class="bg-brand text-brand-foreground absolute inset-x-0 bottom-0 flex items-center justify-between gap-2 px-2 py-1.5 text-xs font-medium"
			>
				<span>New cover, applied on save</span>
				<button
					type="button"
					aria-label="Discard new cover"
					class="hover:bg-black/10 grid size-5 place-items-center rounded"
					onclick={ondiscardpending}
				>
					<XIcon class="size-3.5" />
				</button>
			</div>
		{/if}
	</div>
	<div class="grid gap-1.5">
		<Button variant="outline" size="sm" onclick={choose} disabled={busy}>
			<ImagePlusIcon />
			Choose file…
		</Button>
		<Button
			variant="ghost"
			size="sm"
			class="text-muted-foreground"
			onclick={remove}
			disabled={busy || (!pendingUrl && !book.hasCover)}
		>
			<Trash2Icon />
			Remove
		</Button>
	</div>
</div>
