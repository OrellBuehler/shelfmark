<script lang="ts">
	import Download from "@lucide/svelte/icons/download";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import { Button } from "#lib/components/ui/button";
	import * as Dialog from "#lib/components/ui/dialog";
	import { Progress } from "#lib/components/ui/progress";
	import { openLink } from "#lib/links";
	import { RELEASES_URL, updater } from "#lib/updater.svelte";
</script>

<Dialog.Root bind:open={() => updater.open, (v) => (updater.open = v || updater.busy)}>
	<Dialog.Content
		showCloseButton={!updater.busy}
		escapeKeydownBehavior={updater.busy ? "ignore" : "close"}
		interactOutsideBehavior={updater.busy ? "ignore" : "close"}
	>
		{#if updater.available}
			<Dialog.Header>
				<Dialog.Title>Shelfmark {updater.available.version} is available</Dialog.Title>
				<Dialog.Description>
					You have {updater.current}.
					{#if updater.portable}
						The portable version can't update itself. Download the new version and replace this file.
					{:else}
						The update is downloaded, installed and Shelfmark restarts.
					{/if}
				</Dialog.Description>
			</Dialog.Header>

			{#if updater.available.notes.trim()}
				<div class="max-h-56 overflow-y-auto rounded-md border bg-muted/40 px-3 py-2 text-[13px] leading-relaxed whitespace-pre-wrap">{updater.available.notes.trim()}</div>
			{/if}

			{#if updater.busy}
				<div class="grid gap-2" aria-live="polite">
					<Progress value={updater.phase === "installing" ? 100 : (updater.progress ?? 0) * 100} />
					<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
						<LoaderCircle class="size-3 animate-spin" />
						{#if updater.phase === "installing"}
							Installing
						{:else if updater.progress === null}
							Downloading
						{:else}
							Downloading {Math.round(updater.progress * 100)}%
						{/if}
					</p>
				</div>
			{:else if updater.phase === "failed"}
				<p class="text-[13px] text-destructive" role="alert">
					The update could not be installed: {updater.error}
				</p>
			{/if}

			<Dialog.Footer>
				{#if !updater.busy}
					<Button variant="ghost" onclick={() => (updater.open = false)}>Later</Button>
				{/if}
				{#if updater.portable || updater.phase === "failed"}
					<Button variant={updater.portable ? "default" : "outline"} onclick={() => openLink(RELEASES_URL)}>
						<ExternalLink />Open download page
					</Button>
				{/if}
				{#if !updater.portable}
					<Button disabled={updater.busy} onclick={() => updater.install()}>
						<Download />{updater.phase === "failed" ? "Try again" : "Install and restart"}
					</Button>
				{/if}
			</Dialog.Footer>
		{/if}
	</Dialog.Content>
</Dialog.Root>
