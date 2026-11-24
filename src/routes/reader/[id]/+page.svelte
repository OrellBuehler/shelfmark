<script lang="ts">
	import { page } from "$app/state";
	import ReaderView from "$lib/reader/ReaderView.svelte";

	const format = $derived(page.url.searchParams.get("format")?.toUpperCase() ?? null);
	const src = $derived(page.url.searchParams.get("src"));
</script>

{#key `${page.params.id}/${format}`}
	<ReaderView id={Number(page.params.id)} {format} {src} />
{/key}
