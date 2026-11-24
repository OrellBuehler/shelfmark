<script lang="ts">
	import Check from "@lucide/svelte/icons/check";
	import Copy from "@lucide/svelte/icons/copy";
	import MessageSquarePlus from "@lucide/svelte/icons/message-square-plus";
	import { floatAt } from "./float";
	import type { Reader } from "./reader.svelte";
	import { highlightColors } from "./themes";

	let { reader }: { reader: Reader } = $props();

	let vw = $state(1000);
	let vh = $state(800);
	let copied = $state(false);

	const sel = $derived(reader.selection);
	const pos = $derived(sel ? floatAt(sel.box, 236, 40, vw, vh) : null);

	async function copy() {
		if (!sel) return;
		try {
			await navigator.clipboard.writeText(sel.text);
			copied = true;
			setTimeout(() => {
				copied = false;
				reader.clearSelection();
			}, 700);
		} catch (e) {
			console.error(e);
		}
	}
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

{#if sel && pos}
	<div
		role="toolbar"
		aria-label="Selection"
		tabindex="-1"
		class="bg-popover text-popover-foreground ring-foreground/10 animate-in fade-in-0 zoom-in-95 fixed z-40 flex h-10 items-center gap-1 rounded-full px-2 shadow-lg ring-1 duration-100"
		style:left="{pos.x}px"
		style:top="{pos.y}px"
		onmousedown={(e) => e.preventDefault()}
	>
		{#each Object.entries(highlightColors) as [name, value] (name)}
			<button
				type="button"
				class="size-6 rounded-full ring-1 ring-black/10 transition-transform hover:scale-110 focus-visible:ring-2 focus-visible:ring-ring"
				style:background={value}
				aria-label="Highlight {name}"
				title="Highlight {name}"
				onclick={() => reader.createHighlight(name)}
			></button>
		{/each}
		<span class="bg-border mx-1 h-5 w-px"></span>
		<button
			type="button"
			class="hover:bg-muted flex size-7 items-center justify-center rounded-full"
			aria-label="Add note"
			title="Add note"
			onclick={() => reader.createHighlight("yellow", true)}
		>
			<MessageSquarePlus class="size-4" />
		</button>
		<button
			type="button"
			class="hover:bg-muted flex size-7 items-center justify-center rounded-full"
			aria-label="Copy"
			title="Copy"
			onclick={copy}
		>
			{#if copied}<Check class="size-4 text-green-600" />{:else}<Copy class="size-4" />{/if}
		</button>
	</div>
{/if}
