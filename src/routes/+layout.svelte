<script lang="ts">
	import "../app.css";
	import { onMount } from "svelte";
	import { ModeWatcher } from "mode-watcher";
	import { Toaster } from "$lib/components/ui/sonner";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import UpdateDialog from "$lib/components/update/UpdateDialog.svelte";
	import { updater } from "$lib/updater.svelte";

	let { children } = $props();

	onMount(() => void updater.init());
</script>

<ModeWatcher />
<Tooltip.Provider delayDuration={500} skipDelayDuration={150}>
	{@render children()}
	<UpdateDialog />
</Tooltip.Provider>
<Toaster position="bottom-right" closeButton />
