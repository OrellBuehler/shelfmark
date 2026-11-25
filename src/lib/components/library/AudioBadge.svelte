<script lang="ts">
	import BookHeadphones from "@lucide/svelte/icons/book-headphones";
	import HeadphoneOff from "@lucide/svelte/icons/headphone-off";
	import Headphones from "@lucide/svelte/icons/headphones";
	import type { BookSummary } from "#lib/api";
	import { cn } from "#lib/utils";

	let {
		book,
		overlay = false,
		class: className
	}: { book: Pick<BookSummary, "kind" | "audio">; overlay?: boolean; class?: string } = $props();

	const only = $derived(book.kind === "audiobook");
	const missing = $derived(book.audio?.state === "missing");
	const title = $derived(
		missing ? "Audiobook files are missing" : only ? "Audiobook only" : "Ebook and audiobook"
	);
	const tone = $derived(
		missing
			? overlay
				? "bg-black/45 text-white/55"
				: "bg-muted text-muted-foreground"
			: only
				? "bg-brand text-brand-foreground"
				: overlay
					? "bg-white/90 text-neutral-900 shadow-sm"
					: "bg-secondary text-secondary-foreground"
	);
</script>

{#if book.audio}
	<span
		class={cn("inline-flex size-5 shrink-0 items-center justify-center rounded-full backdrop-blur-sm", tone, className)}
		{title}
		role="img"
		aria-label={title}
	>
		{#if missing}
			<HeadphoneOff class="size-3" />
		{:else if only}
			<Headphones class="size-3" />
		{:else}
			<BookHeadphones class="size-3" />
		{/if}
	</span>
{/if}
