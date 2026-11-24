<script lang="ts">
	import { cn } from "#lib/utils.js";
	import { suggest } from "./format";

	let {
		value = $bindable(""),
		options = [],
		exclude = [],
		labelOf,
		placeholder = "",
		id,
		class: className,
		bare = false,
		disabled = false,
		ref = $bindable(null),
		onpick,
		onsubmit,
		onbackspaceempty,
		oninput
	}: {
		value?: string;
		options?: string[];
		exclude?: string[];
		labelOf?: (v: string) => string;
		placeholder?: string;
		id?: string;
		class?: string;
		bare?: boolean;
		disabled?: boolean;
		ref?: HTMLInputElement | null;
		onpick?: (v: string) => void;
		onsubmit?: (text: string) => void;
		onbackspaceempty?: () => void;
		oninput?: () => void;
	} = $props();

	let focused = $state(false);
	let dismissed = $state(false);
	let active = $state(-1);
	let up = $state(false);
	const listId = `ac-${Math.random().toString(36).slice(2, 9)}`;

	const items = $derived(
		value.trim() || active >= 0 ? suggest(options, value, exclude, 8, labelOf) : []
	);
	const shown = $derived(focused && !dismissed && items.length > 0);

	$effect(() => {
		if (!shown || !ref) return;
		const anchor = ref.closest<HTMLElement>("[data-anchor]") ?? ref;
		const r = anchor.getBoundingClientRect();
		const below = window.innerHeight - r.bottom;
		up = below < 280 && r.top > below;
	});

	function pick(v: string) {
		if (onpick) onpick(v);
		else value = v;
		dismissed = true;
		active = -1;
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === "ArrowDown") {
			if (!shown && value.trim() === "" && options.length) {
				dismissed = false;
				active = 0;
			} else if (items.length) active = (active + 1) % items.length;
			e.preventDefault();
		} else if (e.key === "ArrowUp" && shown) {
			active = active <= 0 ? items.length - 1 : active - 1;
			e.preventDefault();
		} else if (e.key === "Enter" && !e.ctrlKey && !e.metaKey) {
			if (shown && active >= 0) {
				pick(items[active]);
				e.preventDefault();
			} else if (onsubmit) {
				onsubmit(value);
				e.preventDefault();
			}
		} else if (e.key === "Tab" && shown && active >= 0) {
			pick(items[active]);
			e.preventDefault();
		} else if (e.key === "Escape" && shown) {
			dismissed = true;
			e.preventDefault();
			e.stopPropagation();
		} else if (e.key === "Backspace" && value === "" && onbackspaceempty) {
			onbackspaceempty();
		}
	}
</script>

<div class={bare ? "contents" : "relative"}>
	<input
		bind:this={ref}
		bind:value
		{id}
		{placeholder}
		{disabled}
		type="text"
		autocomplete="off"
		spellcheck="false"
		role="combobox"
		aria-expanded={shown}
		aria-controls={listId}
		aria-autocomplete="list"
		aria-activedescendant={active >= 0 ? `${listId}-${active}` : undefined}
		class={cn(
			bare
				? "min-w-24 flex-1 bg-transparent py-0.5 text-sm outline-none placeholder:text-muted-foreground"
				: "dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-9 w-full min-w-0 rounded-md border bg-transparent px-2.5 py-1 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:ring-3 disabled:opacity-50",
			className
		)}
		onfocus={() => (focused = true)}
		onblur={() => (focused = false)}
		oninput={() => {
			active = -1;
			dismissed = false;
			oninput?.();
		}}
		{onkeydown}
	/>
	{#if shown}
		<ul
			id={listId}
			role="listbox"
			class={cn(
				"bg-popover text-popover-foreground ring-foreground/10 absolute left-0 z-50 max-h-64 w-full min-w-56 overflow-auto rounded-lg p-1 shadow-lg ring-1",
				up ? "bottom-full mb-1" : "top-full mt-1"
			)}
		>
			{#each items as item, i (item)}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<li
					id="{listId}-{i}"
					role="option"
					aria-selected={i === active}
					class={cn(
						"flex cursor-default items-center justify-between gap-3 rounded-md px-2 py-1.5 text-sm",
						i === active && "bg-accent text-accent-foreground"
					)}
					onmousedown={(e) => e.preventDefault()}
					onclick={() => pick(item)}
					onmouseenter={() => (active = i)}
				>
					<span class="truncate">{labelOf ? labelOf(item) : item}</span>
					{#if labelOf && labelOf(item) !== item}
						<span class="text-muted-foreground font-mono text-xs">{item}</span>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</div>
