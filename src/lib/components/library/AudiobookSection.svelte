<script lang="ts">
	import ArrowRightLeft from "@lucide/svelte/icons/arrow-right-left";
	import Cloud from "@lucide/svelte/icons/cloud";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import Play from "@lucide/svelte/icons/play";
	import Unlink from "@lucide/svelte/icons/unlink";
	import type { BookSummary } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import { formatDuration, formatSize, plural } from "#lib/format";
	import { lib } from "#lib/stores/library.svelte";

	let { summary }: { summary: BookSummary } = $props();

	const a = $derived(summary.audio);
	const playable = $derived(a?.state === "present");
	const remote = $derived(a?.state === "remote");
</script>

{#if a}
	<section>
		<h3 class="mb-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Audiobook</h3>
		<div class="space-y-3 rounded-md border bg-card p-3">
			<dl class="grid grid-cols-[88px_1fr] gap-x-3 gap-y-1.5 text-[13px]">
				{#if a.narrators.length}
					<dt class="text-muted-foreground">Narrator</dt>
					<dd class="min-w-0">
						{#each a.narrators as n, i (n)}
							{#if i > 0}<span class="mr-1">,</span>{/if}<button
								type="button"
								class="rounded-sm underline-offset-2 outline-none hover:underline focus-visible:ring-2 focus-visible:ring-ring"
								onclick={() => lib.addFilter(`narrators:"=${n}"`)}>{n}</button
							>
						{/each}
					</dd>
				{/if}
				<dt class="text-muted-foreground">Length</dt>
				<dd class="tabular-nums">{formatDuration(a.durationMs)}</dd>
				{#if !remote}
					<dt class="text-muted-foreground">Format</dt>
					<dd class="tabular-nums"><span class="uppercase">{a.format}</span> · {formatSize(a.size)}</dd>
					<dt class="text-muted-foreground">Files</dt>
					<dd class="tabular-nums">{a.fileCount.toLocaleString()}</dd>
				{/if}
			</dl>
			{#if a.state === "missing"}
				<p class="text-xs text-muted-foreground">
					The audio files were not found at the last scan. Connect the drive or folder and scan again.
				</p>
			{:else if remote}
				<p class="flex items-center gap-2 text-[13px]">
					<Cloud class="size-4 shrink-0 text-sky-600 dark:text-sky-400" />In your Audible library, not downloaded
				</p>
			{/if}
			{#if a.count > 1}
				<p class="text-xs text-muted-foreground">{plural(a.count, "audiobook")} are linked to this ebook. The first is shown.</p>
			{/if}
			<div class="flex gap-2">
				{#if remote}
					<Button size="sm" class="flex-1" disabled={!a.asin} onclick={() => lib.openOnAudible(summary)}>
						<ExternalLink />Open on Audible
					</Button>
				{:else}
					<Button size="sm" class="flex-1" disabled={!playable} onclick={() => lib.playAudio(summary)}><Play />Play</Button>
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Show audiobook in folder"
						title="Show audiobook in folder"
						disabled={!playable}
						onclick={() => lib.showAudioInFolder(summary)}
					>
						<FolderOpen />
					</Button>
					<Button
						variant="outline"
						size="icon-sm"
						aria-label="Copy or move audiobook"
						title="Copy or move…"
						disabled={summary.audiobookId === null}
						onclick={() => summary.audiobookId !== null && lib.openTransfer([summary.audiobookId])}
					>
						<ArrowRightLeft />
					</Button>
				{/if}
				<Button
					variant="outline"
					size="sm"
					disabled={summary.audiobookId === null}
					onclick={() => summary.audiobookId !== null && lib.unlinkAudio(summary.audiobookId, summary.id)}
				>
					<Unlink />Unlink
				</Button>
			</div>
		</div>
	</section>
{/if}
