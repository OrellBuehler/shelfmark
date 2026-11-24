<script lang="ts">
	import CheckIcon from "@lucide/svelte/icons/check";
	import LoaderIcon from "@lucide/svelte/icons/loader-circle";
	import TriangleAlertIcon from "@lucide/svelte/icons/triangle-alert";
	import BooksIcon from "@lucide/svelte/icons/library-big";
	import { untrack } from "svelte";
	import { toast } from "svelte-sonner";
	import { api, type BulkPatch } from "#lib/api";
	import { Button } from "#lib/components/ui/button/index.js";
		import { Input } from "#lib/components/ui/input/index.js";
		import * as Dialog from "#lib/components/ui/dialog/index.js";
	import { cn } from "#lib/utils.js";
	import Autocomplete from "./Autocomplete.svelte";
	import Check from "./Check.svelte";
	import ChipsInput from "./ChipsInput.svelte";
	import Toggle from "./Toggle.svelte";
	import RatingStars from "./RatingStars.svelte";
	import { LANGUAGES, languageName, resolveLanguage } from "./languages";

	let {
		open = $bindable(false),
		bookIds,
		onsaved
	}: { open: boolean; bookIds: number[]; onsaved?: () => void } = $props();

	type Field = "authors" | "series" | "tags" | "publisher" | "pubdate" | "languages" | "rating";

	let on = $state<Record<Field, boolean>>({
		authors: false,
		series: false,
		tags: false,
		publisher: false,
		pubdate: false,
		languages: false,
		rating: false
	});
	let authors = $state<string[]>([]);
	let seriesMode = $state<"set" | "clear">("set");
	let seriesName = $state("");
	let numbering = $state(false);
	let seriesStart = $state<number | null>(1);
	let tagsAdd = $state<string[]>([]);
	let tagsRemove = $state<string[]>([]);
	let tagsClear = $state(false);
	let publisher = $state("");
	let pubdate = $state("");
	let languages = $state<string[]>([]);
	let rating = $state(0);
	let saving = $state(false);

	let completions = $state({
		authors: [] as string[],
		series: [] as string[],
		tags: [] as string[],
		publisher: [] as string[],
		languages: [] as string[]
	});

	const languageOptions = $derived([
		...new Set([...LANGUAGES.map(([c]) => c), ...completions.languages])
	]);

	const count = $derived(bookIds.length);
	const noun = $derived(count === 1 ? "book" : "books");

	function reset() {
		on = { authors: false, series: false, tags: false, publisher: false, pubdate: false, languages: false, rating: false };
		authors = [];
		seriesMode = "set";
		seriesName = "";
		numbering = false;
		seriesStart = 1;
		tagsAdd = [];
		tagsRemove = [];
		tagsClear = false;
		publisher = "";
		pubdate = "";
		languages = [];
		rating = 0;
		saving = false;
	}

	async function loadCompletions() {
		const fields = ["authors", "series", "tags", "publisher", "languages"] as const;
		const res = await Promise.allSettled(fields.map((f) => api.completions(f)));
		const next = { ...completions };
		fields.forEach((f, i) => {
			const r = res[i];
			next[f] = r.status === "fulfilled" ? r.value : [];
		});
		completions = next;
	}

	$effect(() => {
		if (open) {
			untrack(() => {
				reset();
				loadCompletions();
			});
		}
	});

	type Change = { field: Field; text: string; problem?: string };

	const changes = $derived.by<Change[]>(() => {
		const list: Change[] = [];
		const quote = (s: string) => `“${s}”`;
		if (on.authors)
			list.push(
				authors.length
					? { field: "authors", text: `Set authors to ${authors.join(", ")}` }
					: { field: "authors", text: "Set authors", problem: "Add at least one author" }
			);
		if (on.series) {
			if (seriesMode === "clear") list.push({ field: "series", text: "Clear series" });
			else if (!seriesName.trim())
				list.push({ field: "series", text: "Set series", problem: "Enter a series name" });
			else
				list.push({
					field: "series",
					text: `Set series to ${quote(seriesName.trim())}${numbering ? `, numbered from ${seriesStart ?? 1}` : ""}`
				});
		}
		if (on.tags) {
			const parts: string[] = [];
			if (tagsClear) parts.push("Remove all existing tags");
			else if (tagsRemove.length) parts.push(`Remove tags: ${tagsRemove.join(", ")}`);
			if (tagsAdd.length) parts.push(`Add tags: ${tagsAdd.join(", ")}`);
			if (parts.length) for (const text of parts) list.push({ field: "tags", text });
			else list.push({ field: "tags", text: "Change tags", problem: "Choose tags to add or remove" });
		}
		if (on.publisher)
			list.push({
				field: "publisher",
				text: publisher.trim() ? `Set publisher to ${quote(publisher.trim())}` : "Clear publisher"
			});
		if (on.pubdate)
			list.push({ field: "pubdate", text: pubdate ? `Set published date to ${pubdate}` : "Clear published date" });
		if (on.languages)
			list.push({
				field: "languages",
				text: languages.length ? `Set languages to ${languages.map(languageName).join(", ")}` : "Clear languages"
			});
		if (on.rating)
			list.push({
				field: "rating",
				text: rating ? `Set rating to ${rating / 2} ${rating === 2 ? "star" : "stars"}` : "Clear rating"
			});
		return list;
	});

	const hasProblem = $derived(changes.some((c) => c.problem));
	const canApply = $derived(changes.length > 0 && !hasProblem && !saving);

	function buildPatch(): BulkPatch {
		const p: BulkPatch = {};
		if (on.authors) p.authors = authors;
		if (on.series) {
			p.series = seriesMode === "clear" ? "" : seriesName.trim();
			if (seriesMode === "set" && numbering) p.seriesStart = seriesStart ?? 1;
		}
		if (on.tags) {
			if (tagsClear) p.clearTags = true;
			else if (tagsRemove.length) p.removeTags = tagsRemove;
			if (tagsAdd.length) p.addTags = tagsAdd;
		}
		if (on.publisher) p.publisher = publisher.trim();
		if (on.pubdate) p.pubdate = pubdate ? `${pubdate}T00:00:00+00:00` : "";
		if (on.languages) p.languages = languages;
		if (on.rating) p.rating = rating;
		return p;
	}

	async function apply() {
		if (!canApply) return;
		saving = true;
		try {
			await api.bulkUpdate(bookIds, buildPatch());
			toast.success(`Updated ${count} ${noun}`);
			onsaved?.();
			open = false;
		} catch (e) {
			toast.error(`Could not update books: ${e}`);
		} finally {
			saving = false;
		}
	}

	function onkeydown(e: KeyboardEvent) {
		if (open && e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
			e.preventDefault();
			apply();
		}
	}

	const modeBtn = (active: boolean) =>
		cn(
			"rounded px-2.5 py-1 text-xs transition-colors",
			active ? "bg-background font-medium shadow-sm" : "text-muted-foreground hover:text-foreground"
		);
</script>

{#snippet card(field: Field, title: string, body: import("svelte").Snippet)}
	<section
		class={cn(
			"rounded-xl border transition-colors",
			on[field] ? "bg-card border-foreground/15 shadow-xs" : "bg-muted/25 border-transparent"
		)}
	>
		<div class="flex items-center gap-3 px-4 py-3">
			<Toggle id="bulk-{field}" bind:checked={on[field]} label="Change {title.toLowerCase()}" />
			<label
				for="bulk-{field}"
				class={cn("flex-1 cursor-pointer text-sm font-medium", !on[field] && "text-muted-foreground")}
			>
				{title}
			</label>
			{#if !on[field]}
				<span class="text-muted-foreground text-xs">Unchanged</span>
			{/if}
		</div>
		{#if on[field]}
			<div class="space-y-3 px-4 pb-4">{@render body()}</div>
		{/if}
	</section>
{/snippet}

{#snippet authorsBody()}
	<ChipsInput bind:value={authors} options={completions.authors} separators="&;" placeholder="Replace authors with…" />
	<p class="text-muted-foreground text-xs">Replaces the authors of every selected book.</p>
{/snippet}

{#snippet seriesBody()}
	<div class="bg-muted inline-flex rounded-md p-0.5" role="group" aria-label="Series action">
		<button type="button" class={modeBtn(seriesMode === "set")} aria-pressed={seriesMode === "set"} onclick={() => (seriesMode = "set")}>Set series</button>
		<button type="button" class={modeBtn(seriesMode === "clear")} aria-pressed={seriesMode === "clear"} onclick={() => (seriesMode = "clear")}>Clear series</button>
	</div>
	{#if seriesMode === "set"}
		<Autocomplete bind:value={seriesName} options={completions.series} placeholder="Series name" />
		<div class="flex items-center gap-2.5">
			<Check id="bulk-numbering" bind:checked={numbering} />
			<label for="bulk-numbering" class="text-sm">Number the books in order, starting at</label>
			<Input type="number" step="1" min="0" bind:value={seriesStart} disabled={!numbering} class="h-8 w-20" />
		</div>
	{:else}
		<p class="text-muted-foreground text-xs">Removes the series and numbering from every selected book.</p>
	{/if}
{/snippet}

{#snippet tagsBody()}
	<div>
		<p class="text-muted-foreground mb-1.5 text-xs font-medium">Add</p>
		<ChipsInput bind:value={tagsAdd} options={completions.tags} placeholder="Tags to add" />
	</div>
	<div class={cn(tagsClear && "pointer-events-none opacity-50")}>
		<p class="text-muted-foreground mb-1.5 text-xs font-medium">Remove</p>
		<ChipsInput bind:value={tagsRemove} options={completions.tags} placeholder="Tags to remove" />
	</div>
	<div class="flex items-center gap-2.5">
		<Check id="bulk-clear-tags" bind:checked={tagsClear} />
		<label for="bulk-clear-tags" class="text-sm">Remove all existing tags first</label>
	</div>
{/snippet}

{#snippet publisherBody()}
	<Autocomplete bind:value={publisher} options={completions.publisher} placeholder="Publisher" />
	<p class="text-muted-foreground text-xs">Leave empty to clear the publisher.</p>
{/snippet}

{#snippet pubdateBody()}
	<Input type="date" bind:value={pubdate} class="w-48" />
	<p class="text-muted-foreground text-xs">Leave empty to clear the published date.</p>
{/snippet}

{#snippet languagesBody()}
	<ChipsInput
		bind:value={languages}
		options={languageOptions}
		labelOf={languageName}
		resolve={resolveLanguage}
		placeholder="Replace languages with…"
	/>
	<p class="text-muted-foreground text-xs">Leave empty to clear the languages.</p>
{/snippet}

{#snippet ratingBody()}
	<RatingStars bind:value={rating} />
	<p class="text-muted-foreground text-xs">No rating clears it. Click the same star again to reset.</p>
{/snippet}

<svelte:window {onkeydown} />

<Dialog.Root bind:open>
	<Dialog.Content
		class="flex h-[min(90vh,760px)] w-[880px] max-w-[calc(100%-2rem)] flex-col gap-0 overflow-hidden p-0 sm:max-w-[880px]"
	>
		<header class="border-b py-3.5 pr-14 pl-6">
			<Dialog.Title class="text-base font-semibold">Editing {count} {noun}</Dialog.Title>
			<Dialog.Description class="text-muted-foreground mt-0.5 text-xs">
				Switch on a field to change it for every selected book. Everything else stays as it is.
			</Dialog.Description>
		</header>

		<div class="grid min-h-0 flex-1 grid-cols-[1fr_17rem]">
			<div class="min-h-0 space-y-2.5 overflow-y-auto p-5">
				{@render card("authors", "Authors", authorsBody)}
				{@render card("series", "Series", seriesBody)}
				{@render card("tags", "Tags", tagsBody)}
				{@render card("publisher", "Publisher", publisherBody)}
				{@render card("pubdate", "Published date", pubdateBody)}
				{@render card("languages", "Languages", languagesBody)}
				{@render card("rating", "Rating", ratingBody)}
			</div>

			<aside class="bg-muted/30 flex min-h-0 flex-col border-l">
				<div class="flex items-center gap-3 border-b px-5 py-4">
					<div class="bg-brand/20 grid size-9 place-items-center rounded-lg">
						<BooksIcon class="size-4.5" />
					</div>
					<div>
						<p class="text-sm leading-tight font-semibold">{count} {noun}</p>
						<p class="text-muted-foreground text-xs">will be updated</p>
					</div>
				</div>
				<div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
					{#if changes.length === 0}
						<p class="text-muted-foreground text-xs leading-relaxed">
							No changes yet. Switch on a field on the left to start.
						</p>
					{:else}
						<ul class="space-y-2.5">
							{#each changes as change, i (i)}
								<li class="flex gap-2 text-xs leading-snug">
									{#if change.problem}
										<TriangleAlertIcon class="text-destructive mt-px size-3.5 shrink-0" />
										<span>
											{change.text}
											<span class="text-destructive block">{change.problem}</span>
										</span>
									{:else}
										<CheckIcon class="text-brand mt-px size-3.5 shrink-0" />
										<span class="break-words">{change.text}</span>
									{/if}
								</li>
							{/each}
						</ul>
					{/if}
				</div>
			</aside>
		</div>

		<footer class="flex items-center justify-end gap-2 border-t px-6 py-3">
			<Button variant="ghost" onclick={() => (open = false)}>Cancel</Button>
			<Button onclick={apply} disabled={!canApply}>
				{#if saving}
					<LoaderIcon class="animate-spin" />
				{/if}
				Apply to {count} {noun}
			</Button>
		</footer>
	</Dialog.Content>
</Dialog.Root>
