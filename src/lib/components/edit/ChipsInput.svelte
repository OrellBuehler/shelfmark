<script lang="ts">
	import XIcon from "@lucide/svelte/icons/x";
	import { cn } from "$lib/utils.js";
	import Autocomplete from "./Autocomplete.svelte";

	let {
		value = $bindable([]),
		options = [],
		labelOf,
		resolve,
		separators = ",",
		placeholder = "",
		id,
		class: className,
		disabled = false
	}: {
		value?: string[];
		options?: string[];
		labelOf?: (v: string) => string;
		resolve?: (text: string) => string | null;
		separators?: string;
		placeholder?: string;
		id?: string;
		class?: string;
		disabled?: boolean;
	} = $props();

	let draft = $state("");
	let input = $state<HTMLInputElement | null>(null);
	let wrapper: HTMLDivElement;

	function add(text: string): boolean {
		const v = resolve ? resolve(text) : text.trim();
		if (!v) return false;
		if (!value.some((x) => x.toLowerCase() === v.toLowerCase())) value = [...value, v];
		return true;
	}

	function commit() {
		if (draft.trim() && add(draft)) draft = "";
	}

	function remove(i: number) {
		value = value.filter((_, j) => j !== i);
		input?.focus();
	}

	function onType() {
		if (!separators) return;
		const parts = draft.split(new RegExp(`[${separators.replace(/[\]\\^-]/g, "\\$&")}]`));
		if (parts.length < 2) return;
		const last = parts.pop() as string;
		for (const p of parts) add(p);
		draft = last.trimStart();
	}

	function onfocusout(e: FocusEvent) {
		if (!wrapper.contains(e.relatedTarget as Node | null)) commit();
	}

	function onkeydowncapture(e: KeyboardEvent) {
		if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) commit();
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
	bind:this={wrapper}
	data-anchor
	class={cn(
		"dark:bg-input/30 border-input focus-within:border-ring focus-within:ring-ring/50 relative flex min-h-9 w-full cursor-text flex-wrap items-center gap-1.5 rounded-md border bg-transparent px-1.5 py-1 shadow-xs transition-[color,box-shadow] focus-within:ring-3",
		disabled && "pointer-events-none opacity-50",
		className
	)}
	onclick={() => input?.focus()}
	{onfocusout}
	{onkeydowncapture}
>
	{#each value as chip, i (chip)}
		<span
			class="bg-secondary text-secondary-foreground inline-flex max-w-full items-center gap-0.5 rounded-md py-0.5 pr-0.5 pl-2 text-xs font-medium"
		>
			<span class="truncate">{labelOf ? labelOf(chip) : chip}</span>
			<button
				type="button"
				tabindex="-1"
				aria-label="Remove {labelOf ? labelOf(chip) : chip}"
				class="text-muted-foreground hover:bg-foreground/10 hover:text-foreground grid size-4 shrink-0 place-items-center rounded-sm"
				onclick={(e) => {
					e.stopPropagation();
					remove(i);
				}}
			>
				<XIcon class="size-3" />
			</button>
		</span>
	{/each}
	<Autocomplete
		bind:value={draft}
		bind:ref={input}
		bare
		{id}
		{options}
		{labelOf}
		exclude={value}
		placeholder={value.length ? "" : placeholder}
		onpick={(v) => {
			add(v);
			draft = "";
		}}
		onsubmit={(t) => {
			if (t.trim() && add(t)) draft = "";
		}}
		onbackspaceempty={() => (value = value.slice(0, -1))}
		oninput={onType}
	/>
</div>
