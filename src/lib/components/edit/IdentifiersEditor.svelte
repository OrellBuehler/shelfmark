<script lang="ts" module>
	export type IdentifierRow = { key: string; value: string };
</script>

<script lang="ts">
	import PlusIcon from "@lucide/svelte/icons/plus";
	import XIcon from "@lucide/svelte/icons/x";
	import { Button } from "#lib/components/ui/button/index.js";

	let { rows = $bindable([]) }: { rows?: IdentifierRow[] } = $props();

	const keys = ["isbn", "amazon", "google", "goodreads", "doi", "mobi-asin", "oclc", "lccn", "issn", "openlibrary"];
	const listId = `idkeys-${Math.random().toString(36).slice(2, 8)}`;

	const field =
		"dark:bg-input/30 border-input focus-visible:border-ring focus-visible:ring-ring/50 h-8 min-w-0 rounded-md border bg-transparent px-2 text-sm shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:ring-3";

	function add() {
		rows = [...rows, { key: "", value: "" }];
	}
</script>

<div class="space-y-1.5">
	<datalist id={listId}>
		{#each keys as k (k)}
			<option value={k}></option>
		{/each}
	</datalist>
	{#each rows as row, i (i)}
		<div class="flex items-center gap-1.5">
			<input
				class="{field} w-28 font-mono text-xs"
				list={listId}
				placeholder="isbn"
				aria-label="Identifier type"
				bind:value={row.key}
			/>
			<input
				class="{field} flex-1"
				placeholder="Value"
				aria-label="Identifier value"
				bind:value={row.value}
			/>
			<Button
				variant="ghost"
				size="icon-sm"
				class="text-muted-foreground"
				aria-label="Remove identifier"
				onclick={() => (rows = rows.filter((_, j) => j !== i))}
			>
				<XIcon />
			</Button>
		</div>
	{/each}
	<Button variant="ghost" size="xs" class="text-muted-foreground" onclick={add}>
		<PlusIcon />
		Add identifier
	</Button>
</div>
