<script lang="ts">
	import Bookmark from "@lucide/svelte/icons/bookmark";
	import Trash2 from "@lucide/svelte/icons/trash-2";
	import { cn } from "#lib/utils.js";
	import type { Reader } from "./reader.svelte";
	import { colorValue } from "./themes";

	let { reader }: { reader: Reader } = $props();

	let filter = $state<"all" | "highlight" | "bookmark">("all");

	const all = $derived(reader.sortedAnnotations);
	const list = $derived(filter === "all" ? all : all.filter((a) => a.kind === filter));
	const counts = $derived({
		highlight: all.filter((a) => a.kind === "highlight").length,
		bookmark: all.filter((a) => a.kind === "bookmark").length
	});
</script>

<div class="flex h-full min-h-0 flex-col">
	<div class="px-3 pt-3 pb-2">
		<div class="bg-muted grid grid-cols-3 gap-0.5 rounded-lg p-0.5 text-sm">
			{#each [["all", `All ${all.length}`], ["highlight", `Highlights ${counts.highlight}`], ["bookmark", `Bookmarks ${counts.bookmark}`]] as [id, label] (id)}
				<button
					type="button"
					onclick={() => (filter = id as typeof filter)}
					class={cn(
						"truncate rounded-md px-1 py-1 text-xs transition-colors",
						filter === id ? "bg-background shadow-sm" : "text-muted-foreground hover:text-foreground"
					)}>{label}</button
				>
			{/each}
		</div>
	</div>
	<div class="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
		{#if !list.length}
			<p class="text-muted-foreground px-3 py-10 text-center text-sm">
				{#if reader.fixed && !reader.hasText}
					Bookmark pages to find them again here.
				{:else}
					Select text to highlight it and add notes.<br />Press B to bookmark the current page.
				{/if}
			</p>
		{/if}
		<ul class="space-y-1">
			{#each list as a (a.id)}
				<li class="group/note relative">
					<button
						type="button"
						onclick={() => reader.goToAnnotation(a)}
						class="hover:bg-muted/70 flex w-full gap-3 rounded-md px-2 py-2.5 pr-9 text-left"
					>
						<span class="mt-0.5 flex w-1 shrink-0 justify-center">
							{#if a.kind === "highlight"}
								<span
									class="h-full min-h-5 w-1 rounded-full"
									style:background={colorValue(a.color)}
								></span>
							{:else}
								<Bookmark class="size-4 shrink-0 fill-amber-500 text-amber-500" />
							{/if}
						</span>
						<span class="min-w-0 flex-1 pl-1">
							<span class="text-muted-foreground block truncate text-xs">{a.chapter ?? ""}</span>
							{#if a.text}
								<span class="mt-0.5 line-clamp-4 block text-sm leading-snug">{a.text}</span>
							{:else}
								<span class="text-muted-foreground mt-0.5 block text-sm">Bookmark</span>
							{/if}
							{#if a.note}
								<span class="text-muted-foreground mt-1.5 line-clamp-3 block border-l-2 pl-2 text-sm italic">
									{a.note}
								</span>
							{/if}
						</span>
					</button>
					<button
						type="button"
						class="text-muted-foreground hover:text-destructive hover:bg-destructive/10 absolute top-2 right-2 rounded p-1.5 opacity-0 transition-opacity group-hover/note:opacity-100 focus-visible:opacity-100"
						aria-label="Delete"
						onclick={() => reader.removeAnnotation(a.id)}
					>
						<Trash2 class="size-4" />
					</button>
				</li>
			{/each}
		</ul>
	</div>
</div>
