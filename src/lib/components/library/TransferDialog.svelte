<script lang="ts">
	import { untrack, type Component } from "svelte";
	import Check from "@lucide/svelte/icons/check";
	import CircleCheck from "@lucide/svelte/icons/circle-check";
	import FolderSearch from "@lucide/svelte/icons/folder-search";
	import HardDrive from "@lucide/svelte/icons/hard-drive";
	import LibraryBig from "@lucide/svelte/icons/library-big";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
	import FolderOpen from "@lucide/svelte/icons/folder-open";
	import { api, type AudioRoot, type Device, type JobProgress, type TransferPlan, type TransferResult } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import * as Dialog from "#lib/components/ui/dialog";
	import { Progress } from "#lib/components/ui/progress";
	import { baseName, errorMessage, formatSize, plural } from "#lib/format";
	import { audio } from "#lib/stores/audio.svelte";
	import { lib, pickFolder } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";

	const SOURCE_LOOKUP_LIMIT = 30;

	type Dest = { kind: "root"; id: number } | { kind: "folder"; path: string; label: string };

	let { open = $bindable(false), ids }: { open: boolean; ids: number[] } = $props();

	let dest = $state<Dest | null>(null);
	let mode = $state<"copy" | "move">("copy");
	let verify = $state<"quick" | "full">("quick");
	let verifyTouched = false;
	let devices = $state.raw<Device[]>([]);
	let sources = $state.raw<(number | null)[] | null>(null);
	let customFolder = $state<Extract<Dest, { kind: "folder" }> | null>(null);
	let plan = $state.raw<TransferPlan | null>(null);
	let planError = $state<string | null>(null);
	let planning = $state(false);
	let adding = $state(false);
	let step = $state<"setup" | "running" | "done">("setup");
	let progress = $state.raw<JobProgress | null>(null);
	let cancelling = $state(false);
	let result = $state.raw<TransferResult | null>(null);
	let failure = $state<string | null>(null);
	let running: { cancel: () => void } | null = null;
	let planToken = 0;

	const roots = $derived(audio.status.roots);
	const managed = $derived(roots.filter((r) => r.managed && r.enabled));
	const soleSource = $derived(sources && new Set(sources).size === 1 ? sources[0] : undefined);
	const targets = $derived(managed.filter((r) => r.id !== soleSource));
	const folderDevices = $derived(devices.filter((d) => d.kind === "folder"));
	const unmanagedSource = $derived(
		sources ? sources.some((id) => id !== null && !roots.find((r) => r.id === id)?.managed) : roots.some((r) => !r.managed)
	);
	const transferable = $derived(plan ? plan.items.filter((i) => !i.skip) : []);
	const skipped = $derived(plan ? plan.items.filter((i) => i.skip) : []);
	const canStart = $derived(!!dest && !!plan && plan.enoughSpace && transferable.length > 0 && !audio.job && !planning);
	const verb = $derived(mode === "move" ? "Move" : "Copy");

	$effect(() => {
		if (!open) return;
		untrack(() => {
			step = "setup";
			dest = null;
			customFolder = null;
			mode = "copy";
			verify = "quick";
			verifyTouched = false;
			plan = null;
			planError = null;
			result = null;
			failure = null;
			progress = null;
			cancelling = false;
			sources = null;
			void loadDevices();
			void loadSources(ids);
			const first = audio.status.roots.find((r) => r.managed && r.enabled);
			if (first) dest = { kind: "root", id: first.id };
		});
	});

	$effect(() => {
		if (soleSource !== undefined && dest?.kind === "root" && dest.id === soleSource) dest = null;
	});

	$effect(() => {
		if (!open || step !== "setup" || !dest) return;
		const target = dest.kind === "root" ? { destRootId: dest.id } : { destFolder: dest.path };
		const list = ids;
		const mine = ++planToken;
		plan = null;
		planError = null;
		planning = true;
		api.audioTransferPlan(list, target).then(
			(p) => {
				if (mine !== planToken) return;
				plan = p;
				planning = false;
			},
			(e) => {
				if (mine !== planToken) return;
				planError = errorMessage(e);
				planning = false;
			}
		);
	});

	async function loadDevices() {
		try {
			devices = await api.detectDevices();
		} catch {
			devices = [];
		}
	}

	async function loadSources(list: number[]) {
		if (list.length > SOURCE_LOOKUP_LIMIT) return;
		try {
			const found = await Promise.all(list.map((id) => api.audiobook(id).then((a) => a.rootId)));
			if (list === ids) sources = found;
		} catch {
			sources = null;
		}
	}

	function path(r: AudioRoot) {
		if (!r.inLibrary || !lib.info) return r.path;
		return `${lib.info.path}${lib.info.path.includes("\\") ? "\\" : "/"}${r.path}`;
	}

	function deviceFolder(d: Device) {
		return `${d.booksDir}${d.booksDir.includes("\\") ? "\\" : "/"}Audiobooks`;
	}

	function chooseFolder(folder: Extract<Dest, { kind: "folder" }>) {
		dest = folder;
		setMode("copy");
	}

	async function chooseOther() {
		const picked = await pickFolder("Choose a folder to copy the audiobooks to");
		if (!picked) return;
		customFolder = { kind: "folder", path: picked, label: baseName(picked) };
		chooseFolder(customFolder);
	}

	async function addLibraryLocation() {
		adding = true;
		try {
			const root = await api.audioAddRoot(true);
			await audio.refresh();
			dest = { kind: "root", id: root.id };
		} catch (e) {
			lib.fail("Could not add location", e);
		} finally {
			adding = false;
		}
	}

	function setMode(next: "copy" | "move") {
		mode = next;
		if (!verifyTouched) verify = next === "move" ? "full" : "quick";
	}

	function setVerify(next: "quick" | "full") {
		verify = next;
		verifyTouched = true;
	}

	function same(a: Dest | null, b: Dest) {
		return !!a && a.kind === b.kind && (a.kind === "root" ? a.id === (b as typeof a).id : a.path === (b as typeof a).path);
	}

	function start() {
		if (!dest || !canStart) return;
		const options = {
			mode,
			verify,
			...(dest.kind === "root" ? { destRootId: dest.id } : { destFolder: dest.path })
		};
		step = "running";
		progress = null;
		cancelling = false;
		failure = null;
		const job = api.audioTransfer(ids, options, (p) => (progress = p));
		running = job;
		const finish = (end: { event: string; result?: TransferResult; message?: string }) => {
			running = null;
			if (end.event === "done" && end.result) {
				result = end.result;
				step = "done";
			} else if (end.event === "cancelled") {
				result = { done: 0, skipped: [], failed: [], bytes: 0, cancelled: true };
				step = "done";
			} else {
				failure = end.message ?? "The transfer failed";
				step = "done";
			}
			void audio.refresh();
		};
		job.result.then(finish, (e) => finish({ event: "failed", message: errorMessage(e) }));
	}

	function cancel() {
		if (cancelling) return;
		cancelling = true;
		running?.cancel();
	}

	function phase(p: JobProgress) {
		return p.phase === "copying" ? `${mode === "move" ? "Moving" : "Copying"} files` : "Updating the library";
	}

	function summary(r: TransferResult) {
		const what = plural(r.done, "audiobook");
		if (r.cancelled) return r.done ? `Cancelled after ${mode === "move" ? "moving" : "copying"} ${what}` : "Cancelled";
		return `${mode === "move" ? "Moved" : "Copied"} ${what}`;
	}

	const ratio = $derived(progress && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0);
</script>

{#snippet destRow(target: Dest, Icon: Component<{ class?: string }>, title: string, sub: string, note?: string)}
	<button
		type="button"
		role="radio"
		aria-checked={same(dest, target)}
		class={cn(
			"flex w-full items-center gap-3 rounded-lg border bg-card px-3 py-2.5 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring",
			same(dest, target) && "border-brand bg-brand/10"
		)}
		onclick={() => (target.kind === "root" ? (dest = target) : chooseFolder(target))}
	>
		<Icon class="size-4 shrink-0 text-brand" />
		<span class="min-w-0 flex-1">
			<span class="block truncate text-sm font-medium">{title}</span>
			<span class="block truncate font-mono text-[11px] text-muted-foreground">{sub}</span>
		</span>
		{#if note}<span class="shrink-0 text-[11px] text-muted-foreground">{note}</span>{/if}
		{#if same(dest, target)}<Check class="size-4 shrink-0 text-brand" />{/if}
	</button>
{/snippet}

<Dialog.Root bind:open>
	<Dialog.Content
		class="flex max-h-[min(90vh,780px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-xl"
		showCloseButton={step !== "running"}
		interactOutsideBehavior={step === "running" ? "ignore" : "close"}
		escapeKeydownBehavior={step === "running" ? "ignore" : "close"}
	>
		<header class="border-b py-3.5 pr-14 pl-6">
			<Dialog.Title class="text-base font-semibold">Copy or move audiobooks</Dialog.Title>
			<Dialog.Description class="mt-0.5 text-xs text-muted-foreground">{plural(ids.length, "audiobook")} selected</Dialog.Description>
		</header>

		{#if step === "setup"}
			<div class="thin-scroll min-h-0 flex-1 space-y-5 overflow-y-auto p-6">
				<section class="space-y-2">
					<h3 class="text-xs font-medium tracking-wide text-muted-foreground uppercase">Destination</h3>
					<div role="radiogroup" aria-label="Destination" class="space-y-1.5">
						{#each targets as r (r.id)}
							{@render destRow({ kind: "root", id: r.id }, FolderOpen, r.label ?? baseName(r.path), path(r))}
						{/each}
						{#if !managed.length}
							<button
								type="button"
								class="flex w-full items-center gap-3 rounded-lg border bg-card px-3 py-2.5 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
								disabled={adding}
								onclick={addLibraryLocation}
							>
								{#if adding}<LoaderCircle class="size-4 shrink-0 animate-spin" />{:else}<LibraryBig class="size-4 shrink-0 text-brand" />{/if}
								<span class="min-w-0 flex-1">
									<span class="block text-sm font-medium">Store audiobooks inside this library</span>
									<span class="block text-xs text-muted-foreground">Creates an Audiobooks folder in your library folder.</span>
								</span>
							</button>
						{/if}
						{#each folderDevices as d (d.id)}
							{@render destRow({ kind: "folder", path: deviceFolder(d), label: d.name }, HardDrive, d.name, deviceFolder(d), "Copy only")}
						{/each}
						{#if customFolder}
							{@render destRow(customFolder, FolderOpen, customFolder.label, customFolder.path, "Copy only")}
						{/if}
						<button
							type="button"
							class="flex w-full items-center gap-3 rounded-lg border border-dashed px-3 py-2.5 text-left text-sm outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring"
							onclick={chooseOther}
						>
							<FolderSearch class="size-4 shrink-0 text-muted-foreground" />
							Another folder…
						</button>
					</div>
				</section>

				{#if dest}
					<section class="space-y-2.5">
						<h3 class="text-xs font-medium tracking-wide text-muted-foreground uppercase">Plan</h3>
						{#if planning}
							<div class="h-16 animate-pulse rounded-lg bg-muted"></div>
						{:else if planError}
							<p class="rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px] text-destructive">{planError}</p>
						{:else if plan}
							<div class="space-y-1.5 rounded-lg border bg-card px-3 py-2.5 text-[13px]">
								<div class="flex justify-between gap-3">
									<span class="text-muted-foreground">Audiobooks</span>
									<span class="tabular-nums">{transferable.length.toLocaleString()} of {ids.length.toLocaleString()}</span>
								</div>
								<div class="flex justify-between gap-3">
									<span class="text-muted-foreground">Total size</span>
									<span class="tabular-nums">{formatSize(plan.totalBytes)}</span>
								</div>
								<div class="flex justify-between gap-3">
									<span class="text-muted-foreground">Free space</span>
									<span class="tabular-nums">{plan.freeBytes === null ? "Unknown" : formatSize(plan.freeBytes)}</span>
								</div>
								<div class="truncate font-mono text-[11px] text-muted-foreground" title={plan.destPath}>{plan.destPath}</div>
							</div>
							{#if !plan.enoughSpace}
								<p class="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px] text-destructive">
									<TriangleAlert class="mt-0.5 size-4 shrink-0" />
									Not enough free space at the destination. Free up space or choose another destination.
								</p>
							{/if}
							{#if skipped.length}
								<div>
									<p class="mb-1 text-xs text-muted-foreground">Will be skipped ({skipped.length})</p>
									<ul class="thin-scroll max-h-36 space-y-1 overflow-y-auto">
										{#each skipped as item (item.audiobookId)}
											<li class="rounded-md border bg-card px-2.5 py-1.5 text-xs">
												<span class="font-medium">{item.title}</span>
												<span class="text-muted-foreground"> · {item.skip}</span>
											</li>
										{/each}
									</ul>
								</div>
							{/if}
						{/if}
					</section>
				{/if}

				<section class="space-y-2">
					<h3 class="text-xs font-medium tracking-wide text-muted-foreground uppercase">What to do</h3>
					<div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Copy or move">
						<button
							type="button"
							role="radio"
							aria-checked={mode === "copy"}
							class={cn(
								"rounded-lg border bg-card px-3 py-2 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring",
								mode === "copy" && "border-brand bg-brand/10"
							)}
							onclick={() => setMode("copy")}
						>
							<span class="block text-sm font-medium">Copy</span>
							<span class="block text-xs text-muted-foreground">Keep the originals where they are.</span>
						</button>
						<button
							type="button"
							role="radio"
							aria-checked={mode === "move"}
							disabled={dest?.kind === "folder"}
							class={cn(
								"rounded-lg border bg-card px-3 py-2 text-left outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
								mode === "move" && "border-brand bg-brand/10"
							)}
							onclick={() => setMode("move")}
						>
							<span class="block text-sm font-medium">Move</span>
							<span class="block text-xs text-muted-foreground">
								{dest?.kind === "folder" ? "Only between locations." : "Remove the originals afterwards."}
							</span>
						</button>
					</div>
					{#if mode === "move" && unmanagedSource}
						<p class="text-xs text-muted-foreground">
							Moving files out of a folder that another app, such as Libation, manages makes that app download them again.
						</p>
					{/if}
				</section>

				<section class="space-y-2">
					<h3 class="text-xs font-medium tracking-wide text-muted-foreground uppercase">Verification</h3>
					<div class="grid grid-cols-2 gap-2" role="radiogroup" aria-label="Verification">
						{#each [["quick", "Quick"], ["full", "Full"]] as const as [value, label] (value)}
							<button
								type="button"
								role="radio"
								aria-checked={verify === value}
								class={cn(
									"rounded-lg border bg-card px-3 py-1.5 text-left text-sm font-medium outline-none transition-colors hover:bg-accent/50 focus-visible:ring-2 focus-visible:ring-ring",
									verify === value && "border-brand bg-brand/10"
								)}
								onclick={() => setVerify(value)}
							>
								{label}
							</button>
						{/each}
					</div>
					<p class="text-xs text-muted-foreground">
						{verify === "full"
							? "Reads every copied file back and compares it in full. Slower, and the safest choice for a move."
							: "Compares file sizes and the start and end of each file. Fast."}
					</p>
				</section>
			</div>

			<footer class="flex items-center justify-end gap-2 border-t px-6 py-3">
				{#if audio.job}<span class="mr-auto text-xs text-muted-foreground">Wait for the running scan to finish.</span>{/if}
				<Button variant="ghost" onclick={() => (open = false)}>Cancel</Button>
				<Button disabled={!canStart} onclick={start}>{verb}{plan ? ` ${plural(transferable.length, "audiobook")}` : ""}</Button>
			</footer>
		{:else if step === "running"}
			<div class="space-y-3 p-6" role="status" aria-live="polite">
				<div class="flex items-center gap-2 text-sm">
					<LoaderCircle class="size-4 shrink-0 animate-spin" />
					<span class="font-medium">{progress ? phase(progress) : "Starting"}</span>
					{#if progress?.phase === "copying"}
						<span class="ml-auto text-xs text-muted-foreground tabular-nums">
							{formatSize(progress.done)} of {formatSize(progress.total)}
						</span>
					{/if}
				</div>
				<Progress value={ratio} aria-label="Transfer progress" />
				{#if progress?.current && progress.phase === "copying"}
					<p class="truncate text-[13px] text-muted-foreground" title={progress.current}>{progress.current}</p>
				{/if}
			</div>
			<footer class="flex justify-end border-t px-6 py-3">
				<Button variant="outline" disabled={cancelling} onclick={cancel}>{cancelling ? "Cancelling" : "Cancel"}</Button>
			</footer>
		{:else}
			<div class="thin-scroll min-h-0 flex-1 space-y-4 overflow-y-auto p-6">
				{#if failure}
					<p class="flex items-start gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px] text-destructive">
						<TriangleAlert class="mt-0.5 size-4 shrink-0" />
						{failure}
					</p>
				{:else if result}
					<div class="flex items-center gap-2.5">
						{#if result.failed.length || result.cancelled}
							<TriangleAlert class="size-5 shrink-0 text-brand" />
						{:else}
							<CircleCheck class="size-5 shrink-0 text-brand" />
						{/if}
						<div>
							<p class="text-sm font-semibold">{summary(result)}</p>
							<p class="text-xs text-muted-foreground tabular-nums">
								{formatSize(result.bytes)}
								{#if result.skipped.length} · {plural(result.skipped.length, "skipped", "skipped")}{/if}
								{#if result.failed.length} · {plural(result.failed.length, "failed", "failed")}{/if}
							</p>
						</div>
					</div>
					{#if result.failed.length}
						<section>
							<h3 class="mb-1.5 text-xs font-medium tracking-wide text-destructive uppercase">Failed ({result.failed.length})</h3>
							<ul class="thin-scroll max-h-44 space-y-1 overflow-y-auto">
								{#each result.failed as n (n.audiobookId)}
									<li class="rounded-md border border-destructive/30 bg-destructive/5 px-2.5 py-1.5 text-xs">
										<span class="font-medium">{n.title}</span>
										<span class="block text-muted-foreground">{n.message}</span>
									</li>
								{/each}
							</ul>
						</section>
					{/if}
					{#if result.skipped.length}
						<section>
							<h3 class="mb-1.5 text-xs font-medium tracking-wide text-muted-foreground uppercase">Skipped ({result.skipped.length})</h3>
							<ul class="thin-scroll max-h-44 space-y-1 overflow-y-auto">
								{#each result.skipped as n (n.audiobookId)}
									<li class="rounded-md border bg-card px-2.5 py-1.5 text-xs">
										<span class="font-medium">{n.title}</span>
										<span class="text-muted-foreground"> · {n.message}</span>
									</li>
								{/each}
							</ul>
						</section>
					{/if}
				{/if}
			</div>
			<footer class="flex items-center justify-end gap-2 border-t px-6 py-3">
				{#if failure}<Button variant="ghost" onclick={() => (step = "setup")}>Back</Button>{/if}
				<Button onclick={() => (open = false)}>Close</Button>
			</footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>
