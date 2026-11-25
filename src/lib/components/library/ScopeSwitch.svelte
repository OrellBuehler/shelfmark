<script lang="ts">
	import BookOpen from "@lucide/svelte/icons/book-open";
	import Headphones from "@lucide/svelte/icons/headphones";
	import Layers from "@lucide/svelte/icons/layers";
	import type { Scope } from "#lib/api";
	import { lib } from "#lib/stores/library.svelte";
	import IconButton from "./IconButton.svelte";

	const options = [
		{ value: "all", label: "All", icon: Layers },
		{ value: "ebooks", label: "Ebooks", icon: BookOpen },
		{ value: "audiobooks", label: "Audiobooks", icon: Headphones }
	] satisfies { value: Scope; label: string; icon: typeof Layers }[];
</script>

{#if lib.audioSource}
	<div class="flex items-center rounded-md bg-muted/70 p-0.5" role="group" aria-label="Show">
		{#each options as o (o.value)}
			{@const on = lib.scope === o.value}
			<IconButton
				label={o.label}
				icon={o.icon}
				size="xs"
				variant={on ? "secondary" : "ghost"}
				class={on ? "bg-background shadow-xs hover:bg-background" : ""}
				aria-pressed={on}
				onclick={() => lib.setScope(o.value)}
			>
				<span class="hidden lg:inline">{o.label}</span>
			</IconButton>
		{/each}
	</div>
{/if}
