<script lang="ts">
	import Cloud from "@lucide/svelte/icons/cloud";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import RefreshCw from "@lucide/svelte/icons/refresh-cw";
	import TriangleAlert from "@lucide/svelte/icons/triangle-alert";
	import type { JobProgress } from "#lib/api";
	import { Button } from "#lib/components/ui/button";
	import { Progress } from "#lib/components/ui/progress";
	import { formatDateTime, plural } from "#lib/format";
	import { audible } from "#lib/stores/audible.svelte";
	import { pickFile } from "#lib/stores/library.svelte";
	import { cn } from "#lib/utils";

	const MARKETPLACES: Record<string, string> = {
		us: "United States",
		uk: "United Kingdom",
		germany: "Germany",
		france: "France",
		canada: "Canada",
		australia: "Australia",
		italy: "Italy",
		spain: "Spain",
		india: "India",
		japan: "Japan",
		brazil: "Brazil"
	};

	let confirming = $state(false);

	const status = $derived(audible.status);
	const progress = $derived(audible.job?.progress ?? null);
	const ratio = $derived(progress?.phase === "covers" && progress.total ? Math.min(100, (progress.done / progress.total) * 100) : 0);

	function marketplace(locale: string) {
		const key = locale.replace(/^pre-amazon - /, "");
		const name = MARKETPLACES[key] ?? key;
		return locale.startsWith("pre-amazon") ? `${name} (pre-Amazon)` : name;
	}

	function phase(p: JobProgress) {
		return p.phase === "covers" ? "Fetching covers" : "Reading your Audible library";
	}

	async function chooseFile() {
		const path = await pickFile("Choose Libation's AccountsSettings.json", ["json"]);
		if (path) await audible.connect(path);
	}

	async function disconnect() {
		confirming = false;
		await audible.disconnect();
	}
</script>

<section class="space-y-3 rounded-lg border bg-card p-3.5" aria-label="Audible">
	<div class="flex items-center gap-2">
		<Cloud class="size-4 text-sky-600 dark:text-sky-400" />
		<h3 class="text-sm font-semibold">Audible</h3>
		{#if audible.connected}
			<span class="ml-auto text-xs text-muted-foreground tabular-nums">{status.titles.toLocaleString()} not downloaded</span>
		{/if}
	</div>

	{#if !audible.connected}
		<p class="text-[13px] text-muted-foreground">
			{#if status.detected}
				Shelfmark can read your Audible library through Libation's sign-in. It only reads; downloads still happen in Libation.
			{:else}
				Libation was not found on this computer. If you use Libation, choose its AccountsSettings.json file and Shelfmark can read your Audible library through it. It only reads; downloads still happen in Libation.
			{/if}
		</p>
		{#if status.detected}
			<p class="truncate font-mono text-[11px] text-muted-foreground" title={status.detected}>{status.detected}</p>
		{/if}
		<div class="flex flex-wrap gap-2">
			{#if status.detected}
				<Button size="sm" onclick={() => audible.connect()}>Connect</Button>
			{/if}
			<Button size="sm" variant="outline" onclick={chooseFile}>Choose file…</Button>
		</div>
	{:else}
		{#if status.source}
			<p class="truncate font-mono text-[11px] text-muted-foreground" title={status.source}>{status.source}</p>
		{/if}
		{#if status.error}
			<p class="flex items-start gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-[13px] text-destructive">
				<TriangleAlert class="mt-0.5 size-4 shrink-0" />
				The sign-in file can no longer be read: {status.error}
			</p>
		{/if}
		{#if status.accounts.length}
			<ul class="space-y-1.5">
				{#each status.accounts as account (account.key)}
					<li class="rounded-md border px-3 py-2">
						<div class="flex items-baseline gap-2">
							<span class="min-w-0 truncate text-sm font-medium">{account.name}</span>
							<span class="shrink-0 text-xs text-muted-foreground">{marketplace(account.locale)}</span>
						</div>
						{#if account.problem}
							<p class="mt-0.5 flex items-start gap-1.5 text-xs text-muted-foreground">
								<TriangleAlert class="mt-px size-3 shrink-0" />{account.problem}
							</p>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}

		{#if audible.job}
			<div class="space-y-2 rounded-lg border bg-muted/40 p-3" role="status" aria-live="polite">
				<div class="flex items-center gap-2 text-[13px]">
					<LoaderCircle class="size-3.5 shrink-0 animate-spin" />
					<span class="font-medium">{progress ? phase(progress) : "Starting sync"}</span>
					{#if progress?.phase === "library"}
						<span class="text-xs text-muted-foreground tabular-nums">{plural(progress.done, "title")} so far</span>
					{:else if progress?.phase === "covers" && progress.total}
						<span class="text-xs text-muted-foreground tabular-nums">{progress.done.toLocaleString()} of {progress.total.toLocaleString()}</span>
					{/if}
					<Button class="ml-auto" size="xs" variant="outline" disabled={audible.job.cancelling} onclick={() => audible.cancel()}>
						{audible.job.cancelling ? "Cancelling" : "Cancel"}
					</Button>
				</div>
				<Progress value={ratio} class={cn(progress?.phase !== "covers" && "animate-pulse")} aria-label="Audible sync progress" />
				{#if progress?.current}
					<p class="truncate text-xs text-muted-foreground">{progress.current}</p>
				{/if}
			</div>
		{/if}

		<div class="flex flex-wrap items-center gap-2">
			<Button size="sm" variant="outline" disabled={!!audible.job || !!status.error} onclick={() => audible.sync()}>
				<RefreshCw />Sync now
			</Button>
			<Button size="sm" variant="ghost" disabled={!!audible.job} onclick={() => (confirming = !confirming)}>Disconnect</Button>
			<span class="ml-auto text-xs text-muted-foreground">
				{status.lastSync ? `Last synced ${formatDateTime(status.lastSync)}` : "Not synced yet"}
			</span>
		</div>

		{#if confirming}
			<div class="flex flex-wrap items-center gap-2 rounded-md bg-destructive/10 p-2.5 text-[13px]">
				<span class="min-w-0 flex-1">
					Disconnect from Audible? Only the {plural(status.titles, "not-downloaded title")} are removed from Shelfmark. Your downloaded audiobooks, your files and Libation are not touched.
				</span>
				<Button size="xs" variant="ghost" onclick={() => (confirming = false)}>Cancel</Button>
				<Button size="xs" variant="destructive" onclick={disconnect}>Disconnect</Button>
			</div>
		{/if}
	{/if}
</section>
