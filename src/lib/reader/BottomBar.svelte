<script lang="ts">
	import { Slider } from "$lib/components/ui/slider/index.js";
	import type { Reader } from "./reader.svelte";

	let { reader }: { reader: Reader } = $props();

	let scrub = $state<number | null>(null);
	let interacting = false;
	let lastPointer = false;

	$effect(() => {
		void reader.place;
		scrub = null;
	});

	const place = $derived(reader.place);
	const pages = $derived(place?.page ?? null);
	const value = $derived(scrub ?? (pages ? pages.current : (place?.fraction ?? 0)));
	const percent = $derived(
		pages && scrub !== null
			? Math.round((scrub / pages.total) * 100)
			: Math.round((scrub !== null && !pages ? scrub : (place?.fraction ?? 0)) * 100)
	);
	const ticks = $derived(
		reader.fixed
			? []
			: reader.sectionFractions
					.slice(1, -1)
					.reduce<number[]>(
						(acc, t) => (!acc.length || t - acc[acc.length - 1] >= 0.025 ? [...acc, t] : acc),
						[]
					)
	);

	function duration(minutes: number) {
		const m = Math.max(1, Math.round(minutes));
		if (m < 60) return `${m} min`;
		const h = Math.floor(m / 60);
		const rest = m % 60;
		return rest ? `${h} h ${String(rest).padStart(2, "0")} min` : `${h} h`;
	}

	const left = $derived.by(() => {
		if (!place) return "";
		if (pages) return `Page ${scrub ?? pages.current} of ${pages.total}`;
		return place.chapter;
	});

	const right = $derived.by(() => {
		if (!place) return "";
		const parts = [`${percent}%`];
		if (place.minutesLeft !== null && place.minutesLeft >= 0 && scrub === null)
			parts.push(`${duration(place.minutesLeft)} left`);
		return parts.join("  ·  ");
	});

	function commit(v: number) {
		if (pages) reader.goToPage(v);
		else reader.goToFraction(v);
	}
</script>

<div class="px-4 pt-2 pb-3 sm:px-6">
	<div class="text-muted-foreground mx-auto flex max-w-3xl items-baseline justify-between gap-4 text-xs">
		<span class="truncate">{left}</span>
		<span class="shrink-0 tabular-nums">{right}</span>
	</div>
	<div
		class="relative mx-auto mt-2 max-w-3xl"
		role="presentation"
		onpointerdown={() => {
			interacting = true;
			lastPointer = true;
		}}
		onkeydown={() => {
			interacting = true;
			lastPointer = false;
		}}
	>
		{#each ticks as t (t)}
			<span
				class="bg-foreground/25 pointer-events-none absolute top-1/2 h-2.5 w-px -translate-y-1/2"
				style:left="{t * 100}%"
			></span>
		{/each}
		<Slider
			type="single"
			{value}
			min={pages ? 1 : 0}
			max={pages ? pages.total : 1}
			step={pages ? 1 : 0.0005}
			disabled={!place}
			onValueChange={(v) => {
				if (interacting) scrub = v;
			}}
			onValueCommit={(v) => {
				interacting = false;
				commit(v);
				if (document.activeElement instanceof HTMLElement && lastPointer) document.activeElement.blur();
			}}
			aria-label="Reading position"
			class="[&_[data-slot=slider-thumb]]:size-3.5 [&_[data-slot=slider-track]]:h-1 [&_[data-slot=slider-thumb]]:shadow-md"
		/>
	</div>
</div>
