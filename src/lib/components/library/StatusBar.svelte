<script lang="ts">
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import { formatSize, plural } from "#lib/format";
	import { audio } from "#lib/stores/audio.svelte";
	import { lib } from "#lib/stores/library.svelte";
	import { updater } from "#lib/updater.svelte";

	const active = $derived(lib.busy > 0 || lib.loading);
</script>

<footer class="flex h-7 shrink-0 items-center gap-3 border-t bg-sidebar px-3 text-xs text-muted-foreground" aria-live="polite">
	<span class="tabular-nums">
		{#if audio.hasAudio}
			{#if lib.scope !== "audiobooks"}{plural(lib.counts.books, "book")}{/if}{#if lib.scope === "all"}{" · "}{/if}{#if lib.scope !== "ebooks"}{plural(lib.counts.audiobooks, "audiobook")}{/if}
		{:else}
			{plural(lib.books.length, "book")}
		{/if}{#if lib.query && lib.totalBooks > lib.books.length && lib.scope !== "all"}<span class="ml-1 opacity-70">of {lib.totalBooks.toLocaleString()}</span>{/if}
	</span>
	{#if lib.selected.size}
		<span class="text-foreground tabular-nums">{lib.selected.size.toLocaleString()} selected · {formatSize(lib.selectedSize)}</span>
	{/if}
	<span class="ml-auto flex min-w-0 items-center gap-2">
		{#if audio.job}
			{@const p = audio.job.progress}
			<button
				type="button"
				class="flex items-center gap-1.5 rounded px-1 text-foreground tabular-nums hover:underline focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
				onclick={() => (lib.settingsOpen = true)}
			>
				<LoaderCircle class="size-3 animate-spin" />
				Scanning audiobooks{#if p?.phase === "reading" && p.total}{" "}{p.done.toLocaleString()} of {p.total.toLocaleString()}{/if}
			</button>
		{/if}
		{#if active}
			<span class="flex items-center gap-1.5 text-foreground">
				<LoaderCircle class="size-3 animate-spin" />
				{lib.busy > 0 ? lib.busyLabel : "Loading"}
			</span>
		{/if}
		{#if lib.info}
			<span class="truncate" title={lib.info.path}>{lib.info.path}</span>
		{/if}
		{#if updater.current}
			<button
				type="button"
				class="flex shrink-0 items-center gap-1.5 rounded px-1 tabular-nums hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
				title={updater.available ? `Update to ${updater.available.version}` : "Check for updates"}
				disabled={updater.checking}
				onclick={() => updater.check(true)}
			>
				{#if updater.available}
					<span class="size-1.5 rounded-full bg-brand"></span>
					<span class="text-foreground">Update available</span>
				{:else}
					v{updater.current}
				{/if}
			</button>
		{/if}
	</span>
</footer>
