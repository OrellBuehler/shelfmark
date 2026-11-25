<script lang="ts">
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import { untrack } from "svelte";
	import { api, type BookSummary } from "#lib/api";
	import * as Dialog from "#lib/components/ui/dialog";
	import { Input } from "#lib/components/ui/input";
	import { lib } from "#lib/stores/library.svelte";
	import Cover from "./Cover.svelte";

	let {
		open = $bindable(false),
		audiobookId,
		query = "",
		onlinked
	}: { open: boolean; audiobookId: number; query?: string; onlinked?: () => void } = $props();

	let q = $state("");
	let results = $state.raw<BookSummary[]>([]);
	let loading = $state(false);
	let token = 0;

	$effect(() => {
		if (open) untrack(() => (q = query));
	});

	$effect(() => {
		if (!open) return;
		const text = q.trim();
		const mine = ++token;
		loading = true;
		const timer = setTimeout(() => {
			api.listBooks({ search: text || null, sortBy: "title", descending: false, scope: "ebooks" }).then(
				(list) => {
					if (mine !== token) return;
					results = list.slice(0, 50);
					loading = false;
				},
				(e) => {
					if (mine !== token) return;
					loading = false;
					lib.fail("Could not search books", e);
				}
			);
		}, 180);
		return () => clearTimeout(timer);
	});

	async function pick(book: BookSummary) {
		if (!(await lib.linkAudio(audiobookId, book.id))) return;
		open = false;
		onlinked?.();
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[min(80vh,640px)] flex-col gap-4 sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>Link to ebook</Dialog.Title>
			<Dialog.Description>Pick the ebook this audiobook belongs to.</Dialog.Description>
		</Dialog.Header>
		<Input bind:value={q} placeholder="Search your ebooks" aria-label="Search your ebooks" spellcheck={false} autocomplete="off" />
		<div class="thin-scroll -mx-2 min-h-48 flex-1 overflow-y-auto">
			{#if loading && !results.length}
				<div class="flex h-48 items-center justify-center text-muted-foreground"><LoaderCircle class="animate-spin" /></div>
			{:else if !results.length}
				<p class="px-2 py-10 text-center text-sm text-muted-foreground">No ebooks found</p>
			{:else}
				<ul>
					{#each results as b (b.id)}
						<li>
							<button
								type="button"
								class="flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-left outline-none hover:bg-accent focus-visible:bg-accent"
								onclick={() => pick(b)}
							>
								<div class="w-8 shrink-0"><Cover book={b} width={96} /></div>
								<span class="min-w-0 flex-1">
									<span class="block truncate text-sm font-medium">{b.title}</span>
									<span class="block truncate text-xs text-muted-foreground">{b.authors.join(", ") || "Unknown"}</span>
								</span>
								{#if b.audio}
									<span class="shrink-0 rounded bg-secondary px-1.5 py-0.5 text-[10px] font-medium text-secondary-foreground">Has audiobook</span>
								{/if}
							</button>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
