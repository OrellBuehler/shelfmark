<script lang="ts">
	import ChevronRight from "@lucide/svelte/icons/chevron-right";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import FolderSearch from "@lucide/svelte/icons/folder-search";
	import Headphones from "@lucide/svelte/icons/headphones";
	import LibraryBig from "@lucide/svelte/icons/library-big";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import Plus from "@lucide/svelte/icons/plus";
	import RefreshCw from "@lucide/svelte/icons/refresh-cw";
	import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
	import X from "@lucide/svelte/icons/x";
	import type { AudioRoot } from "#lib/api";
	import { Badge } from "#lib/components/ui/badge";
	import { Button } from "#lib/components/ui/button";
	import * as Dialog from "#lib/components/ui/dialog";
	import * as DropdownMenu from "#lib/components/ui/dropdown-menu";
	import { Progress } from "#lib/components/ui/progress";
	import { Switch } from "#lib/components/ui/switch";
	import { baseName, formatDateTime, plural } from "#lib/format";
	import { audio } from "#lib/stores/audio.svelte";
	import { lib, pickFolder } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";
	import AudibleSettings from "./AudibleSettings.svelte";

	let { open = $bindable(false) }: { open: boolean } = $props();

	let confirming = $state<number | null>(null);
	let forgetting = $state<number | null>(null);
	let issuesOpen = $state(false);
	let adding = $state(false);

	const roots = $derived(audio.status.roots);
	const issues = $derived(audio.status.issues);
	const progress = $derived(audio.job?.progress ?? null);
	const hasLibraryRoot = $derived(roots.some((r) => r.inLibrary));
	const ratio = $derived(progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0);

	$effect(() => {
		if (!open) {
			confirming = null;
			forgetting = null;
		}
	});

	async function addExisting() {
		const path = await pickFolder("Choose your audiobook folder");
		if (!path) return;
		adding = true;
		await audio.addRoot(false, path);
		adding = false;
	}

	async function addInLibrary() {
		adding = true;
		await audio.addRoot(true);
		adding = false;
	}

	function fullPath(r: AudioRoot) {
		if (!r.inLibrary || !lib.info) return r.path;
		return `${lib.info.path}${lib.info.path.includes("\\") ? "\\" : "/"}${r.path}`;
	}

	function phase(p: NonNullable<typeof progress>) {
		if (p.phase === "walking") return "Looking for audiobooks";
		if (p.phase === "reading") return "Reading tags";
		return "Finishing up";
	}

	async function remove(r: AudioRoot) {
		confirming = null;
		await audio.removeRoot(r.id);
	}

	async function forget(r: AudioRoot) {
		forgetting = null;
		await audio.forgetMissing(r.id);
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="flex max-h-[min(88vh,760px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
		<header class="border-b py-3.5 pr-14 pl-6">
			<Dialog.Title class="text-base font-semibold">Settings</Dialog.Title>
			<Dialog.Description class="mt-0.5 text-xs text-muted-foreground">Preferences for this library.</Dialog.Description>
		</header>

		<div class="thin-scroll min-h-0 flex-1 space-y-4 overflow-y-auto p-6">
			<div class="flex items-center gap-2">
				<Headphones class="size-4 text-brand" />
				<h2 class="text-sm font-semibold">Audiobooks</h2>
			</div>

			{#if !roots.length}
				<p class="text-[13px] text-muted-foreground">
					Add a place where your audiobooks live. They show up next to your ebooks, and Shelfmark pairs each one with its ebook.
				</p>
				<div class="grid gap-3 sm:grid-cols-2">
					<button
						type="button"
						class="flex flex-col items-start gap-2 rounded-xl border bg-card p-4 text-left outline-none transition-colors hover:border-brand hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
						disabled={adding}
						onclick={addExisting}
					>
						<span class="flex size-9 items-center justify-center rounded-lg bg-brand/20"><FolderSearch class="size-4.5" /></span>
						<span class="text-sm font-semibold">Use an existing folder</span>
						<span class="text-xs text-muted-foreground">
							Index the audiobooks where they are. Your files stay in place and are never changed.
						</span>
					</button>
					<button
						type="button"
						class="flex flex-col items-start gap-2 rounded-xl border bg-card p-4 text-left outline-none transition-colors hover:border-brand hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
						disabled={adding}
						onclick={addInLibrary}
					>
						<span class="flex size-9 items-center justify-center rounded-lg bg-brand/20"><LibraryBig class="size-4.5" /></span>
						<span class="text-sm font-semibold">Store audiobooks inside this library</span>
						<span class="text-xs text-muted-foreground">
							Creates an Audiobooks folder in your library folder. Shelfmark may add files there.
						</span>
					</button>
				</div>
			{:else}
				<div class="flex flex-wrap items-center gap-2">
					<DropdownMenu.Root>
						<DropdownMenu.Trigger>
							{#snippet child({ props })}
								<Button {...props} variant="outline" size="sm" disabled={adding}><Plus />Add location</Button>
							{/snippet}
						</DropdownMenu.Trigger>
						<DropdownMenu.Content align="start" class="w-72">
							<DropdownMenu.Item onSelect={addExisting} class="items-start gap-2">
								<FolderSearch class="mt-0.5" />
								<span>
									<span class="block text-sm font-medium">Use an existing folder</span>
									<span class="block text-xs text-muted-foreground">Files stay where they are</span>
								</span>
							</DropdownMenu.Item>
							<DropdownMenu.Item onSelect={addInLibrary} disabled={hasLibraryRoot} class="items-start gap-2">
								<LibraryBig class="mt-0.5" />
								<span>
									<span class="block text-sm font-medium">Store audiobooks inside this library</span>
									<span class="block text-xs text-muted-foreground">
										{hasLibraryRoot ? "Already added" : "Creates an Audiobooks folder"}
									</span>
								</span>
							</DropdownMenu.Item>
						</DropdownMenu.Content>
					</DropdownMenu.Root>
					<Button variant="outline" size="sm" disabled={!!audio.job} onclick={() => audio.scan()}>
						<RefreshCw />Scan all
					</Button>
					<span class="ml-auto text-xs text-muted-foreground tabular-nums">{plural(audio.status.audiobooks, "audiobook")}</span>
				</div>
			{/if}

			{#if audio.job}
				<div class="space-y-2 rounded-lg border bg-muted/40 p-3" role="status" aria-live="polite">
					<div class="flex items-center gap-2 text-[13px]">
						<LoaderCircle class="size-3.5 shrink-0 animate-spin" />
						<span class="font-medium">{progress ? phase(progress) : "Starting scan"}</span>
						{#if progress && progress.total}
							<span class="text-xs text-muted-foreground tabular-nums">{progress.done.toLocaleString()} of {progress.total.toLocaleString()}</span>
						{:else if progress}
							<span class="text-xs text-muted-foreground tabular-nums">{plural(progress.done, "file")} so far</span>
						{/if}
						<Button class="ml-auto" size="xs" variant="outline" disabled={audio.job.cancelling} onclick={() => audio.cancel()}>
							{audio.job.cancelling ? "Cancelling" : "Cancel"}
						</Button>
					</div>
					<Progress value={ratio} class={cn(progress && !progress.total && "animate-pulse")} aria-label="Scan progress" />
					{#if progress?.current}
						<p class="truncate font-mono text-[11px] text-muted-foreground" title={progress.current}>{progress.current}</p>
					{/if}
				</div>
			{/if}

			{#if roots.length}
				<ul class="space-y-2.5">
					{#each roots as r (r.id)}
						<li class={cn("rounded-lg border bg-card p-3.5", !r.enabled && "opacity-80")}>
							<div class="flex items-start gap-3">
								<FolderOpen class="mt-0.5 size-4 shrink-0 text-brand" />
								<div class="min-w-0 flex-1">
									<div class="flex flex-wrap items-center gap-x-2 gap-y-1">
										<span class="truncate text-sm font-medium">{r.label ?? baseName(r.path)}</span>
										{#if r.inLibrary}<Badge variant="secondary">Inside library</Badge>{/if}
									</div>
									<div class="truncate font-mono text-[11px] text-muted-foreground" title={fullPath(r)}>{fullPath(r)}</div>
									<div class="mt-1 flex flex-wrap items-center gap-x-1.5 text-xs text-muted-foreground">
										<span class="tabular-nums">{plural(r.audiobooks, "audiobook")}</span>
										{#if r.missing > 0}
											<span aria-hidden="true">·</span>
											<span class="tabular-nums">{r.missing.toLocaleString()} missing</span>
											<Button
												variant="link"
												size="xs"
												class="h-auto px-0.5 py-0 text-xs"
												onclick={() => (forgetting = forgetting === r.id ? null : r.id)}
											>
												Forget
											</Button>
										{/if}
										<span aria-hidden="true">·</span>
										{#if r.lastScan}
											<span>Scanned {formatDateTime(r.lastScan)}</span>
											{#if r.lastScanOk === false}
												<span class="inline-flex items-center gap-1 text-destructive">
													<TriangleAlert class="size-3" />Did not complete
												</span>
											{:else}
												<span>Completed</span>
											{/if}
										{:else}
											<span>Not scanned yet</span>
										{/if}
									</div>
								</div>
								<div class="flex shrink-0 items-center gap-1">
									<Button
										variant="outline"
										size="xs"
										disabled={!!audio.job || !r.enabled}
										onclick={() => audio.scan([r.id])}
									>
										<RefreshCw />Rescan
									</Button>
									<Button
										variant="ghost"
										size="icon-xs"
										aria-label="Remove location"
										title="Remove location"
										onclick={() => (confirming = confirming === r.id ? null : r.id)}
									>
										<X />
									</Button>
								</div>
							</div>

							<div class="mt-3 grid gap-2 border-t pt-3 sm:grid-cols-2">
								<label class="flex items-center gap-2.5 text-[13px]">
									<Switch
										checked={r.enabled}
										onCheckedChange={(v) => audio.updateRoot(r.id, { enabled: v })}
										aria-label="Include in library"
									/>
									Include in library
								</label>
								<label class="flex items-center gap-2.5 text-[13px]">
									<Switch
										checked={r.managed}
										onCheckedChange={(v) => audio.updateRoot(r.id, { managed: v })}
										aria-label="Shelfmark may add files here"
									/>
									Shelfmark may add files here
								</label>
							</div>

							{#if forgetting === r.id}
								<div class="mt-3 flex flex-wrap items-center gap-2 rounded-md bg-muted p-2.5 text-[13px]">
									<span class="min-w-0 flex-1">
										Forget {plural(r.missing, "missing audiobook")}? They are removed from Shelfmark's list. Your files are never touched.
									</span>
									<Button size="xs" variant="ghost" onclick={() => (forgetting = null)}>Cancel</Button>
									<Button size="xs" onclick={() => forget(r)}>Forget</Button>
								</div>
							{/if}

							{#if confirming === r.id}
								<div class="mt-3 flex flex-wrap items-center gap-2 rounded-md bg-destructive/10 p-2.5 text-[13px]">
									<span class="min-w-0 flex-1">
										Remove this location from Shelfmark? Its audiobooks disappear from the library. Your files are not deleted.
									</span>
									<Button size="xs" variant="ghost" onclick={() => (confirming = null)}>Cancel</Button>
									<Button size="xs" variant="destructive" onclick={() => remove(r)}>Remove</Button>
								</div>
							{/if}
						</li>
					{/each}
				</ul>
			{/if}

			{#if roots.length}
				<p class="text-xs text-muted-foreground">
					To copy or move audiobooks between locations, select them in the library and choose Copy or move… from the right-click menu.
				</p>
			{/if}

			{#if issues.length}
				<section>
					<button
						type="button"
						class="flex items-center gap-1 rounded-sm text-xs font-medium text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
						aria-expanded={issuesOpen}
						onclick={() => (issuesOpen = !issuesOpen)}
					>
						<ChevronRight class={cn("size-3.5 transition-transform", issuesOpen && "rotate-90")} />
						Issues ({issues.length})
					</button>
					{#if issuesOpen}
						<ul class="mt-2 space-y-1.5">
							{#each issues as issue (issue.path + issue.message)}
								<li class="rounded-md border bg-card px-3 py-2">
									<div class="truncate font-mono text-[11px]" title={issue.path}>{issue.path}</div>
									<div class="mt-0.5 text-xs text-muted-foreground">{issue.message}</div>
								</li>
							{/each}
						</ul>
					{/if}
				</section>
			{/if}

			<AudibleSettings />
		</div>
	</Dialog.Content>
</Dialog.Root>
