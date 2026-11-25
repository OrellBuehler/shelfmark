<script lang="ts">
	import Bookmark from "@lucide/svelte/icons/bookmark";
	import Search from "@lucide/svelte/icons/search";
	import Trash2 from "@lucide/svelte/icons/trash";
	import X from "@lucide/svelte/icons/x";
	import { Button } from "#lib/components/ui/button";
	import * as Popover from "#lib/components/ui/popover";
	import { lib } from "#lib/stores/library.svelte";
	import IconButton from "./IconButton.svelte";

	let input = $state<HTMLInputElement | null>(null);
	let focused = $state(false);
	let saveName = $state("");
	let savedOpen = $state(false);

	$effect(() => {
		lib.searchEl = input;
		return () => {
			lib.searchEl = null;
		};
	});

	function onkeydown(e: KeyboardEvent) {
		if (e.key === "Escape") {
			if (lib.text) lib.clearText();
			else input?.blur();
			e.preventDefault();
		} else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
			input?.blur();
			lib.move(0, e.key === "ArrowDown" ? 1 : -1, false);
			e.preventDefault();
		}
	}

	async function save() {
		await lib.saveCurrentSearch(saveName);
		saveName = "";
	}
</script>

<div class="relative flex min-w-0 flex-1 items-center">
	<Search class="pointer-events-none absolute left-2.5 size-4 text-muted-foreground" />
	<input
		bind:this={input}
		value={lib.text}
		oninput={(e) => lib.setText(e.currentTarget.value)}
		onfocus={() => (focused = true)}
		onblur={() => (focused = false)}
		{onkeydown}
		placeholder={lib.scope === "audiobooks" ? "Search audiobooks" : "Search books"}
		aria-label={lib.scope === "audiobooks" ? "Search audiobooks" : "Search books"}
		spellcheck="false"
		autocomplete="off"
		class="h-8 w-full min-w-0 rounded-md border border-input bg-muted/50 pr-16 pl-8 text-sm outline-none select-text placeholder:text-muted-foreground focus-visible:border-ring focus-visible:bg-background focus-visible:ring-3 focus-visible:ring-ring/30 dark:bg-input/30"
	/>
	<div class="absolute right-1 flex items-center gap-0.5">
		{#if lib.text}
			<IconButton label="Clear search" shortcut="Esc" icon={X} size="icon-xs" onclick={() => lib.clearText()} />
		{:else if !focused}
			<kbd class="mr-1.5 rounded border bg-background px-1.5 text-[10px] font-medium text-muted-foreground">/</kbd>
		{/if}
		<Popover.Root bind:open={savedOpen}>
			<Popover.Trigger>
				{#snippet child({ props })}
					<IconButton {...props} label="Saved searches" icon={Bookmark} size="icon-xs" />
				{/snippet}
			</Popover.Trigger>
			<Popover.Content align="end" class="w-80 gap-3 p-3">
				<div class="text-xs font-medium text-muted-foreground">Saved searches</div>
				{#if lib.savedSearches.length}
					<ul class="-mx-1 max-h-60 overflow-y-auto">
						{#each lib.savedSearches as s (s.name)}
							<li class="group flex items-center gap-1 rounded-md hover:bg-accent">
								<button
									type="button"
									class="min-w-0 flex-1 px-2 py-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
									onclick={() => {
										lib.setQuery(s.query);
										savedOpen = false;
									}}
								>
									<div class="truncate text-sm">{s.name}</div>
									<div class="truncate font-mono text-[11px] text-muted-foreground">{s.query}</div>
								</button>
								<IconButton
									label="Delete saved search"
									icon={Trash2}
									size="icon-xs"
									class="mr-1 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100"
									onclick={() => lib.deleteSavedSearch(s.name)}
								/>
							</li>
						{/each}
					</ul>
				{:else}
					<p class="text-sm text-muted-foreground">Nothing saved yet.</p>
				{/if}
				<form
					class="flex gap-2 border-t pt-3"
					onsubmit={(e) => {
						e.preventDefault();
						void save();
					}}
				>
					<input
						bind:value={saveName}
						placeholder={lib.query ? "Name this search" : "Search for something first"}
						disabled={!lib.query}
						aria-label="Saved search name"
						class="h-8 min-w-0 flex-1 rounded-md border border-input bg-transparent px-2 text-sm outline-none select-text focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30 disabled:opacity-50"
					/>
					<Button type="submit" size="sm" disabled={!lib.query || !saveName.trim()}>Save</Button>
				</form>
			</Popover.Content>
		</Popover.Root>
	</div>
</div>
