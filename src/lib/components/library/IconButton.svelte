<script lang="ts">
	import { mergeProps } from "bits-ui";
	import type { Component } from "svelte";
	import { Button, type ButtonProps } from "#lib/components/ui/button";
	import * as Tooltip from "#lib/components/ui/tooltip";

	let {
		label,
		shortcut,
		icon: Icon,
		side = "bottom",
		variant = "ghost",
		size = "icon-sm",
		children,
		...rest
	}: ButtonProps & {
		label: string;
		shortcut?: string;
		icon?: Component<{ class?: string }>;
		side?: "top" | "bottom" | "left" | "right";
	} = $props();
</script>

<Tooltip.Root>
	<Tooltip.Trigger>
		{#snippet child({ props })}
			<Button {...mergeProps(rest, props)} {variant} {size} aria-label={label}>
				{#if Icon}<Icon />{/if}
				{@render children?.()}
			</Button>
		{/snippet}
	</Tooltip.Trigger>
	<Tooltip.Content {side}>
		{label}
		{#if shortcut}
			<kbd class="rounded bg-background/20 px-1 font-sans text-[10px] font-medium">{shortcut}</kbd>
		{/if}
	</Tooltip.Content>
</Tooltip.Root>
