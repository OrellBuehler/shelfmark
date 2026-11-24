<script lang="ts">
	import { coverSrc, placeholderHue } from "#lib/covers";
	import type { BookSummary } from "#lib/api";
	import { cn } from "#lib/utils";

	let {
		book,
		width = 300,
		eager = false,
		class: className
	}: {
		book: Pick<BookSummary, "id" | "lastModified" | "title" | "authors" | "hasCover">;
		width?: number;
		eager?: boolean;
		class?: string;
	} = $props();

	let failed = $state(false);
	let loaded = $state(false);
	const hue = $derived(placeholderHue(book.title));
	const src = $derived(coverSrc(book, width));
	const showImage = $derived(book.hasCover && !failed);

	$effect(() => {
		void src;
		failed = false;
		loaded = false;
	});
</script>

<div
	class={cn("@container relative aspect-[2/3] w-full overflow-hidden rounded-[3px] bg-muted", className)}
	style={showImage ? undefined : `--h:${hue}`}
>
	{#if showImage}
		<img
			{src}
			alt=""
			loading={eager ? "eager" : "lazy"}
			decoding="async"
			draggable="false"
			class="size-full object-cover transition-opacity duration-200 {loaded ? 'opacity-100' : 'opacity-0'}"
			onload={() => (loaded = true)}
			onerror={() => (failed = true)}
		/>
	{:else}
		<div
			class="absolute inset-0 flex flex-col justify-between p-[9cqw] pb-[17cqw] pl-[13cqw] text-white"
			style="background: linear-gradient(160deg, oklch(0.5 0.075 var(--h)), oklch(0.36 0.07 calc(var(--h) + 25)))"
		>
			<span class="absolute inset-y-0 left-0 w-[4cqw] bg-black/20"></span>
			<span class="line-clamp-4 font-serif text-[11cqw] leading-[1.15] font-bold wrap-break-word">{book.title}</span>
			<span class="line-clamp-2 text-[6.5cqw] tracking-wider uppercase opacity-80">{book.authors[0] ?? ""}</span>
		</div>
	{/if}
	<div class="pointer-events-none absolute inset-0 rounded-[3px] ring-1 ring-black/10 ring-inset dark:ring-white/10"></div>
</div>
