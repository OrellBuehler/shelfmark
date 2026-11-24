<script lang="ts">
	import ChevronRight from "@lucide/svelte/icons/chevron-right";
	import type { TocItem } from "foliate-js/view.js";
	import { tick } from "svelte";
	import { SvelteSet } from "svelte/reactivity";
	import { cn } from "$lib/utils.js";
	import type { Reader } from "./reader.svelte";

	let { reader }: { reader: Reader } = $props();

	const open = new SvelteSet<string>();
	let list = $state<HTMLElement>();

	function pathTo(items: TocItem[], href: string | null, prefix = ""): string[] | null {
		for (const [i, item] of items.entries()) {
			const key = `${prefix}${i}`;
			if (item.href === href) return [key];
			const sub = item.subitems?.length ? pathTo(item.subitems, href, `${key}.`) : null;
			if (sub) return [key, ...sub];
		}
		return null;
	}

	const currentPath = $derived(pathTo(reader.toc, reader.place?.tocHref ?? null));
	const currentKey = $derived(currentPath?.at(-1) ?? null);

	$effect(() => {
		for (const key of currentPath?.slice(0, -1) ?? []) open.add(key);
	});

	$effect(() => {
		if (reader.panel !== "toc") return;
		void tick().then(() =>
			setTimeout(() => list?.querySelector("[data-current=true]")?.scrollIntoView({ block: "center" }), 60)
		);
	});

	function toggle(key: string) {
		if (open.has(key)) open.delete(key);
		else open.add(key);
	}
</script>

{#snippet nodes(items: TocItem[], depth: number, prefix: string)}
	{#each items as item, i (`${prefix}${i}`)}
		{@const key = `${prefix}${i}`}
		{@const children = item.subitems ?? []}
		<li>
			<div
				class={cn(
					"group/row flex items-start rounded-md transition-colors",
					key === currentKey ? "bg-accent text-accent-foreground" : "hover:bg-muted/70"
				)}
				data-current={key === currentKey}
				style:padding-left="{depth * 14 + 4}px"
			>
				{#if children.length}
					<button
						type="button"
						class="text-muted-foreground hover:text-foreground mt-1.5 flex size-6 shrink-0 items-center justify-center rounded"
						aria-label={open.has(key) ? "Collapse" : "Expand"}
						aria-expanded={open.has(key)}
						onclick={() => toggle(key)}
					>
						<ChevronRight class={cn("size-4 transition-transform", open.has(key) && "rotate-90")} />
					</button>
				{:else}
					<span class="size-6 shrink-0"></span>
				{/if}
				<button
					type="button"
					class={cn(
						"min-w-0 flex-1 py-2 pr-3 text-left text-sm leading-snug",
						depth === 0 && "font-medium",
						key === currentKey && "font-semibold"
					)}
					onclick={() => reader.goToToc(item.href)}
				>
					{item.label?.trim() || "Untitled"}
				</button>
			</div>
			{#if children.length && open.has(key)}
				<ul>{@render nodes(children, depth + 1, `${key}.`)}</ul>
			{/if}
		</li>
	{/each}
{/snippet}

<div bind:this={list} class="h-full overflow-y-auto px-2 py-3">
	{#if reader.toc.length}
		<ul class="space-y-px">{@render nodes(reader.toc, 0, "")}</ul>
	{:else}
		<p class="text-muted-foreground px-3 py-8 text-center text-sm">This book has no table of contents.</p>
	{/if}
</div>
