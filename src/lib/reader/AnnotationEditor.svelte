<script lang="ts">
	import Trash2 from "@lucide/svelte/icons/trash-2";
	import { onDestroy, untrack } from "svelte";
	import { Button } from "$lib/components/ui/button/index.js";
	import { cn } from "$lib/utils.js";
	import { floatAt } from "./float";
	import type { Reader } from "./reader.svelte";
	import { highlightColors } from "./themes";

	let { reader }: { reader: Reader } = $props();

	let vw = $state(1000);
	let vh = $state(800);
	let h = $state(200);
	let area = $state<HTMLTextAreaElement>();

	const editor = $derived(reader.editor);
	const annotation = $derived(reader.annotations.find((a) => a.id === editor?.id) ?? null);
	const pos = $derived(editor ? floatAt(editor.box, 300, h, vw, vh) : null);

	let draft = $state("");
	let loadedId = "";
	let loadedNote: string | null = null;

	function commit() {
		if (!loadedId) return;
		const note = draft.trim() || null;
		if (note === loadedNote) return;
		loadedNote = note;
		void reader.updateAnnotation(loadedId, { note });
	}

	$effect(() => {
		if (annotation && annotation.id !== loadedId) {
			const next = annotation;
			untrack(() => {
				commit();
				loadedId = next.id;
				loadedNote = next.note ?? null;
				draft = loadedNote ?? "";
			});
			setTimeout(() => area?.focus(), 60);
		}
	});

	$effect(() => {
		if (!editor)
			untrack(() => {
				commit();
				loadedId = "";
			});
	});

	onDestroy(commit);
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

{#if editor && annotation && pos}
	<div
		role="dialog"
		aria-label="Highlight"
		class="bg-popover text-popover-foreground ring-foreground/10 animate-in fade-in-0 zoom-in-95 fixed z-40 w-[300px] max-w-[calc(100vw-1.5rem)] rounded-xl p-3 shadow-lg ring-1 duration-100"
		style:left="{pos.x}px"
		style:top="{pos.y}px"
		bind:offsetHeight={h}
	>
		<div class="flex items-center gap-1.5">
			{#each Object.entries(highlightColors) as [name, value] (name)}
				<button
					type="button"
					class={cn(
						"size-6 rounded-full ring-1 ring-black/10 transition-transform hover:scale-110",
						(annotation.color ?? "yellow") === name && "ring-foreground/70 ring-2 ring-offset-2 ring-offset-popover"
					)}
					style:background={value}
					aria-label="Colour {name}"
					onclick={() => reader.updateAnnotation(annotation.id, { color: name })}
				></button>
			{/each}
			<span class="flex-1"></span>
			<Button
				variant="ghost"
				size="icon-sm"
				class="text-muted-foreground hover:text-destructive"
				aria-label="Delete highlight"
				title="Delete highlight"
				onclick={() => reader.removeAnnotation(annotation.id)}
			>
				<Trash2 />
			</Button>
		</div>
		{#if annotation.text}
			<p
				class="text-muted-foreground mt-2.5 line-clamp-2 border-l-2 pl-2 text-xs leading-snug italic"
				style:border-color={highlightColors[(annotation.color ?? "yellow") as keyof typeof highlightColors]}
			>
				{annotation.text}
			</p>
		{/if}
		<textarea
			bind:this={area}
			bind:value={draft}
			rows="3"
			placeholder="Add a note…"
			aria-label="Note"
			onblur={commit}
			onkeydown={(e) => {
				if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
					commit();
					reader.editor = null;
				}
			}}
			class="border-input bg-background focus-visible:ring-ring/40 placeholder:text-muted-foreground mt-2.5 w-full resize-none rounded-md border p-2 text-sm outline-none focus-visible:ring-3"
		></textarea>
		<div class="mt-2 flex justify-end">
			<Button
				size="sm"
				variant="secondary"
				onclick={() => {
					commit();
					reader.editor = null;
				}}>Done</Button
			>
		</div>
	</div>
{/if}
