<script lang="ts">
	import "../app.css";
	import { ModeWatcher } from "mode-watcher";
	import { Toaster } from "$lib/components/ui/sonner";
	import * as Tooltip from "$lib/components/ui/tooltip";

	let { children } = $props();
</script>

<ModeWatcher />
<Tooltip.Provider delayDuration={500} skipDelayDuration={150}>
	{@render children()}
</Tooltip.Provider>
<Toaster position="bottom-right" closeButton />
