<script lang="ts">
	import { goto } from "$app/navigation";
	import BookX from "@lucide/svelte/icons/book-x";
	import ChevronDown from "@lucide/svelte/icons/chevron-down";
	import ChevronUp from "@lucide/svelte/icons/chevron-up";
	import ExternalLink from "@lucide/svelte/icons/external-link";
	import LoaderCircle from "@lucide/svelte/icons/loader-circle";
	import Undo2 from "@lucide/svelte/icons/undo-2";
	import X from "@lucide/svelte/icons/x";
	import { mode } from "mode-watcher";
	import { onDestroy, onMount, untrack } from "svelte";
	import { Button } from "#lib/components/ui/button/index.js";
	import { cn } from "#lib/utils.js";
	import AnnotationEditor from "./AnnotationEditor.svelte";
	import BottomBar from "./BottomBar.svelte";
	import { prefs, type ThemeName } from "./prefs.svelte";
	import { Reader } from "./reader.svelte";
	import SelectionToolbar from "./SelectionToolbar.svelte";
	import SidePanel from "./SidePanel.svelte";
	import { palettes } from "./themes";
	import TopBar from "./TopBar.svelte";

	let { id, format, src }: { id: number; format: string | null; src: string | null } = $props();

	const reader = untrack(() => new Reader(id, format, src));
	const root = document.documentElement;
	const systemDark = matchMedia("(prefers-color-scheme: dark)").matches;

	let host = $state<HTMLElement>();
	let message = $state("");

	const appDark = $derived(mode.current ? mode.current === "dark" : systemDark);
	const appTheme = $derived<ThemeName>(appDark ? "dark" : "light");
	const theme = $derived<ThemeName>(prefs.value.theme === "auto" ? appTheme : prefs.value.theme);
	const palette = $derived(palettes[theme]);

	function syncClasses() {
		if (root.classList.contains("dark") !== (theme === "dark"))
			root.classList.toggle("dark", theme === "dark");
		if (root.classList.contains("reader-sepia") !== (theme === "sepia"))
			root.classList.toggle("reader-sepia", theme === "sepia");
	}

	const classObserver = new MutationObserver(syncClasses);

	$effect(() => {
		void theme;
		syncClasses();
	});

	$effect(() => {
		const p = $state.snapshot(prefs.value);
		const t = theme;
		untrack(() => reader.applyPrefs(p, t));
	});

	$effect(() => {
		document.title = reader.title ? `${reader.title} · Shelfmark` : "Shelfmark";
	});

	onMount(() => {
		classObserver.observe(root, { attributes: true, attributeFilter: ["class"] });
		void reader.open(host!);
	});

	onDestroy(() => {
		reader.destroy();
		classObserver.disconnect();
		root.classList.toggle("dark", appDark);
		root.classList.remove("reader-sepia");
		document.title = "Shelfmark";
		if (document.fullscreenElement) void document.exitFullscreen();
	});

	async function back() {
		await reader.flush();
		await goto("/");
	}

	async function external() {
		message = "";
		try {
			await reader.openExternally();
			message = "Opened in the default app.";
		} catch (e) {
			message = typeof e === "string" ? e : "Could not open the file.";
		}
	}

	function flushOnHide() {
		if (document.visibilityState === "hidden") void reader.flush();
	}

	const chromeShown = $derived(reader.chromeVisible || reader.status !== "ready");
	const hits = $derived(reader.searchHits);
	const showSearchPill = $derived(reader.panel === null && reader.search.query && hits.length > 0);
</script>

<svelte:window
	onkeydown={(e) => reader.onKey(e)}
	onpagehide={() => reader.flush()}
	onbeforeunload={() => reader.flush()}
/>
<svelte:document onvisibilitychange={flushOnHide} />

<div
	class="reader-root fixed inset-0 overflow-hidden select-none"
	class:reader-invert={reader.format === "PDF" && theme === "dark"}
	style:background={palette.bg}
	style:color={palette.fg}
	style:--background={palette.bg}
	style:--foreground={palette.fg}
	onmousemove={() => reader.poke()}
	role="presentation"
>
	<div bind:this={host} class="absolute inset-0"></div>

	{#if reader.bookmarkHere && !chromeShown}
		<svg
			class="pointer-events-none absolute top-0 right-6 z-10 h-9 w-5 text-amber-500 drop-shadow-sm"
			viewBox="0 0 20 36"
			aria-hidden="true"
		>
			<path d="M0 0h20v36l-10-8-10 8z" fill="currentColor" />
		</svg>
	{/if}

	{#if reader.status === "ready"}
		<header
			class={cn(
				"bg-background/85 border-border/50 absolute inset-x-0 top-0 z-20 border-b backdrop-blur-md transition-[opacity,transform] duration-300",
				chromeShown ? "translate-y-0 opacity-100" : "pointer-events-none -translate-y-2 opacity-0"
			)}
			inert={!chromeShown}
			role="presentation"
			onpointerenter={() => (reader.chromeHeld = true)}
			onpointerleave={() => {
				reader.chromeHeld = false;
				reader.poke();
			}}
		>
			<TopBar {reader} onback={back} />
		</header>

		<footer
			class={cn(
				"bg-background/85 border-border/50 absolute inset-x-0 bottom-0 z-20 border-t backdrop-blur-md transition-[opacity,transform] duration-300",
				chromeShown ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-2 opacity-0"
			)}
			inert={!chromeShown}
			role="presentation"
			onpointerenter={() => (reader.chromeHeld = true)}
			onpointerleave={() => {
				reader.chromeHeld = false;
				reader.poke();
			}}
		>
			<BottomBar {reader} />
		</footer>

		<div
			class={cn(
				"pointer-events-none absolute inset-x-0 z-20 flex flex-col items-center gap-2 px-4 transition-[bottom] duration-300",
				chromeShown ? "bottom-24" : "bottom-6"
			)}
		>
			{#if showSearchPill}
				<div
					class="bg-popover text-popover-foreground ring-foreground/10 animate-in fade-in-0 slide-in-from-bottom-2 pointer-events-auto flex items-center gap-1 rounded-full py-1 pr-1 pl-4 text-sm shadow-lg ring-1"
				>
					<span class="max-w-40 truncate font-medium">“{reader.search.query.trim()}”</span>
					<span class="text-muted-foreground mx-1 text-xs tabular-nums">
						{reader.search.active >= 0 ? reader.search.active + 1 : "–"} / {hits.length}
					</span>
					<Button
						variant="ghost"
						size="icon-sm"
						class="rounded-full"
						aria-label="Previous result"
						disabled={reader.search.active <= 0}
						onclick={() => reader.goToHit(reader.search.active - 1)}><ChevronUp /></Button
					>
					<Button
						variant="ghost"
						size="icon-sm"
						class="rounded-full"
						aria-label="Next result"
						disabled={reader.search.active >= hits.length - 1}
						onclick={() => reader.goToHit(reader.search.active + 1)}><ChevronDown /></Button
					>
					<Button
						variant="ghost"
						size="icon-sm"
						class="rounded-full"
						aria-label="Clear search"
						onclick={() => reader.clearSearch()}><X /></Button
					>
				</div>
			{/if}
			{#if reader.jumpBack}
				<div
					class="bg-popover text-popover-foreground ring-foreground/10 animate-in fade-in-0 slide-in-from-bottom-2 pointer-events-auto flex items-center rounded-full py-1 pr-1 pl-1 text-sm shadow-lg ring-1"
				>
					<Button variant="ghost" size="sm" class="rounded-full" onclick={() => reader.returnToPrevious()}>
						<Undo2 /> Back to previous position
					</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						class="rounded-full"
						aria-label="Dismiss"
						onclick={() => (reader.jumpBack = false)}><X /></Button
					>
				</div>
			{/if}
		</div>

		<SidePanel {reader} />
		<SelectionToolbar {reader} />
		<AnnotationEditor {reader} />
	{/if}

	{#if reader.status === "loading"}
		<div
			class="absolute inset-0 z-30 flex flex-col items-center justify-center gap-4"
			style:background={palette.bg}
			aria-live="polite"
		>
			<LoaderCircle class="size-6 animate-spin opacity-50" />
			<p class="text-sm opacity-60">{reader.title || "Opening book…"}</p>
		</div>
	{/if}

	{#if reader.status === "error"}
		<div class="absolute inset-0 z-30 flex flex-col items-center justify-center gap-5 px-6 text-center">
			<div class="bg-muted text-muted-foreground flex size-14 items-center justify-center rounded-full">
				<BookX class="size-7" />
			</div>
			<div class="max-w-sm space-y-1.5">
				<h1 class="text-lg font-semibold">
					{reader.errorKind === "missing"
						? "Book file not found"
						: reader.errorKind === "unsupported"
							? "Format not supported"
							: "Couldn’t open this book"}
				</h1>
				<p class="text-muted-foreground text-sm leading-relaxed">{reader.errorMessage}</p>
			</div>
			<div class="flex flex-wrap items-center justify-center gap-2">
				{#if reader.errorKind !== "missing"}
					<Button onclick={external}><ExternalLink /> Open with system app</Button>
				{/if}
				<Button variant={reader.errorKind === "missing" ? "default" : "outline"} onclick={back}>
					Back to library
				</Button>
			</div>
			{#if message}<p class="text-muted-foreground text-xs">{message}</p>{/if}
		</div>
	{/if}

	{#if reader.lightbox}
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<div
			role="presentation"
			class="animate-in fade-in-0 absolute inset-0 z-50 flex items-center justify-center bg-black/85 p-6 duration-150"
			onclick={() => (reader.lightbox = null)}
		>
			<img
				src={reader.lightbox}
				alt=""
				class="max-h-full max-w-full rounded object-contain shadow-2xl"
			/>
			<Button
				variant="secondary"
				size="icon"
				class="absolute top-4 right-4 rounded-full"
				aria-label="Close image"
				onclick={() => (reader.lightbox = null)}><X /></Button
			>
		</div>
	{/if}
</div>

<style>
	:global(html.reader-sepia) {
		--background: #f4ecd9;
		--foreground: #4b3a28;
		--card: #f8f1e0;
		--card-foreground: #4b3a28;
		--popover: #f8f1e0;
		--popover-foreground: #4b3a28;
		--primary: #4b3a28;
		--primary-foreground: #f4ecd9;
		--secondary: #eadfc4;
		--secondary-foreground: #4b3a28;
		--muted: #eadfc4;
		--muted-foreground: #8a7660;
		--accent: #e6d9ba;
		--accent-foreground: #4b3a28;
		--border: #d9ccaa;
		--input: #d9ccaa;
	}
	.reader-invert :global(foliate-view::part(filter)) {
		filter: invert(1) hue-rotate(180deg);
	}
</style>
