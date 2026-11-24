<script lang="ts">
	import FilePlusIcon from "@lucide/svelte/icons/file-plus";
	import Trash2Icon from "@lucide/svelte/icons/trash-2";
	import { open as pickFile } from "@tauri-apps/plugin-dialog";
	import { toast } from "svelte-sonner";
	import { api, inTauri, type Book } from "#lib/api";
	import { Button } from "#lib/components/ui/button/index.js";
	import * as AlertDialog from "#lib/components/ui/alert-dialog/index.js";
	import { humanSize } from "./format";

	let { book, onchanged }: { book: Book; onchanged: (book: Book) => void } = $props();

	let busy = $state(false);
	let confirming = $state<string | null>(null);

	async function add() {
		if (!inTauri) {
			toast.info("Picking a file is only available in the desktop app");
			return;
		}
		const path = await pickFile({
			multiple: false,
			filters: [
				{
					name: "Ebooks",
					extensions: ["epub", "mobi", "azw3", "azw", "pdf", "fb2", "cbz", "djvu", "txt", "docx", "kepub"]
				}
			]
		});
		if (typeof path !== "string") return;
		busy = true;
		try {
			onchanged(await api.addFormat(book.id, path));
			toast.success("Format added");
		} catch (e) {
			toast.error(`Could not add format: ${e}`);
		} finally {
			busy = false;
		}
	}

	async function remove(format: string) {
		busy = true;
		try {
			onchanged(await api.removeFormat(book.id, format));
			toast.success(`Removed ${format.toUpperCase()}`);
		} catch (e) {
			toast.error(`Could not remove format: ${e}`);
		} finally {
			busy = false;
			confirming = null;
		}
	}
</script>

<div class="space-y-2">
	{#if book.files.length === 0}
		<p class="text-muted-foreground text-xs">No formats</p>
	{:else}
		<ul class="divide-border border-border divide-y rounded-lg border">
			{#each book.files as file (file.format)}
				<li class="group flex items-center gap-2 py-1.5 pr-1 pl-2.5 text-xs">
					<span class="bg-secondary text-secondary-foreground rounded px-1.5 py-0.5 font-semibold uppercase">
						{file.format}
					</span>
					<span class="text-muted-foreground flex-1 tabular-nums">{humanSize(file.size)}</span>
					<Button
						variant="ghost"
						size="icon-xs"
						class="text-muted-foreground hover:text-destructive"
						aria-label="Remove {file.format}"
						disabled={busy}
						onclick={() => (confirming = file.format)}
					>
						<Trash2Icon />
					</Button>
				</li>
			{/each}
		</ul>
	{/if}
	<Button variant="outline" size="sm" class="w-full" onclick={add} disabled={busy}>
		<FilePlusIcon />
		Add format…
	</Button>
</div>

<AlertDialog.Root open={confirming !== null} onOpenChange={(v) => !v && (confirming = null)}>
	<AlertDialog.Content size="sm">
		<AlertDialog.Header>
			<AlertDialog.Title>Remove {confirming?.toUpperCase()} file?</AlertDialog.Title>
			<AlertDialog.Description>
				The file is deleted from your library. This cannot be undone.
			</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
			<AlertDialog.Action
				variant="destructive"
				onclick={() => confirming && remove(confirming)}
			>
				Remove
			</AlertDialog.Action>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
