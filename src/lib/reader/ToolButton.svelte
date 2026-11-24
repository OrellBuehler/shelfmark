<script lang="ts">
	import type { Snippet } from "svelte";
	import { Button } from "#lib/components/ui/button/index.js";
	import * as Tooltip from "#lib/components/ui/tooltip/index.js";

	let {
		label,
		shortcut,
		pressed,
		class: className,
		onclick,
		children
	}: {
		label: string;
		shortcut?: string;
		pressed?: boolean;
		class?: string;
		onclick: () => void;
		children: Snippet;
	} = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="icon-sm"
				class={className}
				aria-label={label}
				aria-pressed={pressed}
				{onclick}
			>
				{@render children()}
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content side="bottom" sideOffset={6}>
		{label}
		{#if shortcut}<span class="opacity-60">{shortcut}</span>{/if}
	</Tooltip.Content>
</Tooltip.Root>
