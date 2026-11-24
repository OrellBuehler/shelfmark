<script lang="ts">
	import ALargeSmall from "@lucide/svelte/icons/a-large-small";
	import { Button } from "#lib/components/ui/button/index.js";
	import { Label } from "#lib/components/ui/label/index.js";
	import * as Popover from "#lib/components/ui/popover/index.js";
	import * as Tooltip from "#lib/components/ui/tooltip/index.js";
	import { Slider } from "#lib/components/ui/slider/index.js";
	import { Switch } from "#lib/components/ui/switch/index.js";
	import { cn } from "#lib/utils.js";
	import { limits, prefs, type FontChoice, type ThemeChoice } from "./prefs.svelte";
	import type { Reader } from "./reader.svelte";
	import { palettes } from "./themes";

	let { reader }: { reader: Reader } = $props();

	const p = $derived(prefs.value);

	const themes: { id: ThemeChoice; label: string }[] = [
		{ id: "auto", label: "Auto" },
		{ id: "light", label: "Light" },
		{ id: "sepia", label: "Sepia" },
		{ id: "dark", label: "Dark" }
	];

	const fonts: { id: FontChoice; label: string; family: string }[] = [
		{ id: "publisher", label: "Original", family: "inherit" },
		{ id: "serif", label: "Serif", family: "Georgia, 'Noto Serif', serif" },
		{ id: "sans", label: "Sans", family: "'Inter Variable', system-ui, sans-serif" },
		{ id: "dyslexic", label: "Dyslexia-friendly", family: "Verdana, 'Trebuchet MS', sans-serif" }
	];

	function swatch(id: ThemeChoice) {
		if (id === "auto") return null;
		return palettes[id];
	}
</script>

<Popover.Root bind:open={reader.appearanceOpen}>
	<Tooltip.Root disabled={reader.appearanceOpen}>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<span {...props}>
					<Popover.Trigger>
						{#snippet child({ props })}
							<Button {...props} variant="ghost" size="icon-sm" aria-label="Appearance">
								<ALargeSmall />
							</Button>
						{/snippet}
					</Popover.Trigger>
				</span>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content side="bottom" sideOffset={6}>Appearance</Tooltip.Content>
	</Tooltip.Root>
	<Popover.Content
		align="end"
		sideOffset={8}
		class="max-h-[min(78vh,680px)] w-[min(344px,calc(100vw-1.5rem))] gap-5 overflow-y-auto p-4"
	>
		<div class="grid grid-cols-4 gap-2" role="radiogroup" aria-label="Theme">
			{#each themes as t (t.id)}
				{@const c = swatch(t.id)}
				<button
					type="button"
					role="radio"
					aria-checked={p.theme === t.id}
					onclick={() => prefs.set("theme", t.id)}
					class={cn(
						"flex h-14 flex-col items-center justify-center gap-0.5 rounded-lg border text-xs transition-all outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
						p.theme === t.id
							? "border-foreground/70 ring-1 ring-foreground/40"
							: "border-border hover:border-foreground/30"
					)}
					style={c ? `background:${c.bg};color:${c.fg}` : ""}
				>
					{#if c}
						<span class="font-serif text-base leading-none">Aa</span>
					{:else}
						<span
							class="size-5 rounded-full border"
							style="background:linear-gradient(135deg,#fbfaf7 50%,#1b1a18 50%)"
						></span>
					{/if}
					<span class={c ? "opacity-70" : "text-muted-foreground"}>{t.label}</span>
				</button>
			{/each}
		</div>

		{#if reader.fixed}
			<div class="grid gap-2">
				<Label class="text-muted-foreground text-xs">Zoom</Label>
				<div class="bg-muted grid grid-cols-2 gap-0.5 rounded-lg p-0.5">
					{#each [["fit-page", "Fit page"], ["fit-width", "Fit width"]] as [id, label] (id)}
						<button
							type="button"
							onclick={() => prefs.set("zoom", id as "fit-page" | "fit-width")}
							class={cn(
								"rounded-md py-1.5 text-sm transition-colors",
								p.zoom === id ? "bg-background shadow-sm" : "text-muted-foreground hover:text-foreground"
							)}>{label}</button
						>
					{/each}
				</div>
			</div>
		{:else}
			<div class="grid gap-2">
				<div class="flex items-center gap-3">
					<span class="text-muted-foreground text-sm">A</span>
					<Slider
						type="single"
						value={p.fontSize}
						min={limits.fontSize.min}
						max={limits.fontSize.max}
						step={limits.fontSize.step}
						onValueChange={(v) => prefs.set("fontSize", v)}
						aria-label="Font size"
					/>
					<span class="text-lg leading-none">A</span>
					<span class="text-muted-foreground w-9 text-right text-xs tabular-nums">{p.fontSize}px</span>
				</div>
			</div>

			<div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Font">
				{#each fonts as f (f.id)}
					<button
						type="button"
						role="radio"
						aria-checked={p.font === f.id}
						onclick={() => prefs.set("font", f.id)}
						class={cn(
							"rounded-lg border px-3 py-2 text-left text-sm transition-all outline-none focus-visible:ring-3 focus-visible:ring-ring/50",
							p.font === f.id
								? "border-foreground/70 ring-1 ring-foreground/40"
								: "border-border hover:border-foreground/30"
						)}
						style:font-family={f.family}>{f.label}</button
					>
				{/each}
			</div>

			<div class="grid gap-4">
				<div class="grid gap-2">
					<div class="flex justify-between">
						<Label class="text-muted-foreground text-xs">Line spacing</Label>
						<span class="text-muted-foreground text-xs tabular-nums">{p.lineHeight.toFixed(2)}</span>
					</div>
					<Slider
						type="single"
						value={p.lineHeight}
						min={limits.lineHeight.min}
						max={limits.lineHeight.max}
						step={limits.lineHeight.step}
						onValueChange={(v) => prefs.set("lineHeight", v)}
						aria-label="Line spacing"
					/>
				</div>
				<div class="grid gap-2">
					<div class="flex justify-between">
						<Label class="text-muted-foreground text-xs">Margins</Label>
						<span class="text-muted-foreground text-xs tabular-nums">{p.margin}%</span>
					</div>
					<Slider
						type="single"
						value={p.margin}
						min={limits.margin.min}
						max={limits.margin.max}
						step={limits.margin.step}
						onValueChange={(v) => prefs.set("margin", v)}
						aria-label="Margins"
					/>
				</div>
				<div class="grid gap-2">
					<div class="flex justify-between">
						<Label class="text-muted-foreground text-xs">Maximum line width</Label>
						<span class="text-muted-foreground text-xs tabular-nums">{p.lineWidth}px</span>
					</div>
					<Slider
						type="single"
						value={p.lineWidth}
						min={limits.lineWidth.min}
						max={limits.lineWidth.max}
						step={limits.lineWidth.step}
						onValueChange={(v) => prefs.set("lineWidth", v)}
						aria-label="Maximum line width"
					/>
				</div>
			</div>

			<div class="grid gap-3">
				<div class="flex items-center justify-between">
					<Label for="reader-justify" class="text-sm font-normal">Justify text</Label>
					<Switch
						id="reader-justify"
						checked={p.justify}
						onCheckedChange={(v) => prefs.set("justify", v)}
					/>
				</div>
				<div class="flex items-center justify-between">
					<Label for="reader-hyphenate" class="text-sm font-normal">Hyphenation</Label>
					<Switch
						id="reader-hyphenate"
						checked={p.hyphenate}
						onCheckedChange={(v) => prefs.set("hyphenate", v)}
					/>
				</div>
			</div>

			<div class="grid gap-2">
				<Label class="text-muted-foreground text-xs">Layout</Label>
				<div class="bg-muted grid grid-cols-2 gap-0.5 rounded-lg p-0.5">
					{#each [["paginated", "Paginated"], ["scrolled", "Scrolled"]] as [id, label] (id)}
						<button
							type="button"
							onclick={() => prefs.set("flow", id as "paginated" | "scrolled")}
							class={cn(
								"rounded-md py-1.5 text-sm transition-colors",
								p.flow === id ? "bg-background shadow-sm" : "text-muted-foreground hover:text-foreground"
							)}>{label}</button
						>
					{/each}
				</div>
				{#if p.flow === "paginated"}
					<div class="bg-muted grid grid-cols-2 gap-0.5 rounded-lg p-0.5">
						{#each [[1, "One column"], [2, "Auto columns"]] as [id, label] (id)}
							<button
								type="button"
								onclick={() => prefs.set("columns", id as 1 | 2)}
								class={cn(
									"rounded-md py-1.5 text-sm transition-colors",
									p.columns === id ? "bg-background shadow-sm" : "text-muted-foreground hover:text-foreground"
								)}>{label}</button
							>
						{/each}
					</div>
				{/if}
			</div>

			<Button variant="ghost" size="sm" class="text-muted-foreground -mt-1 self-start" onclick={() => prefs.reset()}>
				Reset to defaults
			</Button>
		{/if}
	</Popover.Content>
</Popover.Root>
