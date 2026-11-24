<script lang="ts">
	import ChevronRight from "@lucide/svelte/icons/chevron-right";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import FolderPlus from "@lucide/svelte/icons/folder-plus";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import { Button } from "#lib/components/ui/button";
	import { plural } from "#lib/format";
	import { lib } from "#lib/stores/library.svelte";
	import AppIcon from "./AppIcon.svelte";
</script>

<div class="flex size-full items-center justify-center overflow-auto bg-background p-6">
	<div class="flex w-full max-w-md flex-col items-center gap-8 py-8">
		<div class="flex flex-col items-center gap-4 text-center">
			<AppIcon class="size-20 drop-shadow-md" />
			<div>
				<h1 class="text-2xl font-semibold tracking-tight">Shelfmark</h1>
				<p class="mt-1 text-sm text-muted-foreground">A fast, calm home for your e-books.</p>
			</div>
		</div>

		<div class="flex w-full flex-col gap-2.5">
			<Button size="lg" class="h-11 justify-start gap-2.5 bg-brand px-4 text-sm text-brand-foreground hover:bg-brand/90" disabled={lib.busy > 0} onclick={() => lib.pickAndOpen(false)}>
				{#if lib.busy > 0}<LoaderCircle class="animate-spin" />{:else}<FolderOpen />{/if}
				Open calibre library
			</Button>
			<Button variant="outline" size="lg" class="h-11 justify-start gap-2.5 px-4 text-sm" disabled={lib.busy > 0} onclick={() => lib.pickAndOpen(true)}>
				<FolderPlus />
				Create new library
			</Button>
			<p class="px-1 pt-1 text-center text-xs text-muted-foreground">
				Existing calibre libraries open in place. Nothing is imported or copied.
			</p>
		</div>

		{#if lib.recents.length}
			<section class="w-full">
				<h2 class="mb-2 px-1 text-xs font-medium tracking-wide text-muted-foreground uppercase">Recent</h2>
				<ul class="overflow-hidden rounded-xl border bg-card">
					{#each lib.recents as r (r.path)}
						<li class="border-b last:border-b-0">
							<button
								type="button"
								class="group flex w-full items-center gap-3 px-3.5 py-2.5 text-left outline-none hover:bg-accent focus-visible:bg-accent"
								onclick={() => lib.openLibrary(r.path)}
							>
								<FolderOpen class="size-4 shrink-0 text-brand" />
								<span class="min-w-0 flex-1">
									<span class="block truncate text-sm font-medium">{r.name}</span>
									<span class="block truncate text-xs text-muted-foreground">{r.path}</span>
								</span>
								<span class="shrink-0 text-xs text-muted-foreground tabular-nums">{plural(r.bookCount, "book")}</span>
								<ChevronRight class="size-4 shrink-0 text-muted-foreground opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100" />
							</button>
						</li>
					{/each}
				</ul>
			</section>
		{/if}
	</div>
</div>
