<script lang="ts">
	import BoldIcon from "@lucide/svelte/icons/bold";
	import ItalicIcon from "@lucide/svelte/icons/italic";
	import UnderlineIcon from "@lucide/svelte/icons/underline";
	import ListIcon from "@lucide/svelte/icons/list";
	import ListOrderedIcon from "@lucide/svelte/icons/list-ordered";
	import CodeIcon from "@lucide/svelte/icons/code-xml";
	import EraserIcon from "@lucide/svelte/icons/eraser";
	import { cn } from "$lib/utils.js";
	import { normalizeHtml, sanitizeHtml } from "./sanitize";

	let {
		value = $bindable(""),
		id,
		placeholder = "No description"
	}: { value?: string; id?: string; placeholder?: string } = $props();

	let source = $state(false);
	let editor = $state<HTMLDivElement | null>(null);
	let sourceText = $state("");
	let emitted = "";
	let marks = $state({ bold: false, italic: false, underline: false, ul: false, ol: false });

	$effect(() => {
		if (!editor || source) return;
		if (value !== emitted || editor.innerHTML === "") {
			editor.innerHTML = value;
			emitted = value;
		}
	});

	function emit() {
		if (!editor) return;
		emitted = normalizeHtml(editor.innerHTML);
		if (!emitted && editor.innerHTML) editor.innerHTML = "";
		value = emitted;
	}

	function exec(cmd: string, arg?: string) {
		editor?.focus();
		document.execCommand(cmd, false, arg);
		emit();
		readMarks();
	}

	function readMarks() {
		marks = {
			bold: document.queryCommandState("bold"),
			italic: document.queryCommandState("italic"),
			underline: document.queryCommandState("underline"),
			ul: document.queryCommandState("insertUnorderedList"),
			ol: document.queryCommandState("insertOrderedList")
		};
	}

	function onpaste(e: ClipboardEvent) {
		e.preventDefault();
		const html = e.clipboardData?.getData("text/html");
		const text = e.clipboardData?.getData("text/plain") ?? "";
		if (html) document.execCommand("insertHTML", false, sanitizeHtml(html));
		else document.execCommand("insertText", false, text);
		emit();
	}

	function onkeydown(e: KeyboardEvent) {
		if (!(e.ctrlKey || e.metaKey)) return;
		const k = e.key.toLowerCase();
		if (k === "b" || k === "i" || k === "u") {
			e.preventDefault();
			exec(k === "b" ? "bold" : k === "i" ? "italic" : "underline");
		}
	}

	function toggleSource() {
		if (!source) {
			sourceText = value;
			source = true;
		} else {
			value = sanitizeHtml(sourceText);
			emitted = "";
			source = false;
		}
	}

	const tools = [
		{ label: "Bold", icon: BoldIcon, key: "bold", run: () => exec("bold") },
		{ label: "Italic", icon: ItalicIcon, key: "italic", run: () => exec("italic") },
		{ label: "Underline", icon: UnderlineIcon, key: "underline", run: () => exec("underline") },
		{ label: "Bulleted list", icon: ListIcon, key: "ul", run: () => exec("insertUnorderedList") },
		{ label: "Numbered list", icon: ListOrderedIcon, key: "ol", run: () => exec("insertOrderedList") },
		{ label: "Clear formatting", icon: EraserIcon, key: "", run: () => exec("removeFormat") }
	] as const;
</script>

<div
	class="border-input dark:bg-input/30 focus-within:border-ring focus-within:ring-ring/50 overflow-hidden rounded-md border shadow-xs transition-[box-shadow] focus-within:ring-3"
>
	<div class="bg-muted/40 flex items-center gap-0.5 border-b p-1">
		{#each tools as tool (tool.label)}
			{@const Icon = tool.icon}
			<button
				type="button"
				title={tool.label}
				aria-label={tool.label}
				disabled={source}
				aria-pressed={tool.key ? marks[tool.key] : undefined}
				class={cn(
					"text-muted-foreground hover:bg-accent hover:text-foreground grid size-7 place-items-center rounded-md transition-colors disabled:pointer-events-none disabled:opacity-40",
					tool.key && marks[tool.key] && "bg-accent text-foreground"
				)}
				onmousedown={(e) => e.preventDefault()}
				onclick={tool.run}
			>
				<Icon class="size-4" />
			</button>
		{/each}
		<div class="flex-1"></div>
		<button
			type="button"
			aria-pressed={source}
			class={cn(
				"text-muted-foreground hover:bg-accent hover:text-foreground flex h-7 items-center gap-1.5 rounded-md px-2 text-xs font-medium transition-colors",
				source && "bg-accent text-foreground"
			)}
			onclick={toggleSource}
		>
			<CodeIcon class="size-3.5" />
			Source
		</button>
	</div>
	{#if source}
		<textarea
			{id}
			bind:value={sourceText}
			spellcheck="false"
			class="block h-44 w-full resize-y bg-transparent p-3 font-mono text-xs leading-relaxed outline-none"
		></textarea>
	{:else}
		<div
			{id}
			bind:this={editor}
			contenteditable="true"
			role="textbox"
			tabindex="0"
			aria-multiline="true"
			data-placeholder={placeholder}
			class="rich min-h-32 max-h-72 overflow-auto p-3 text-sm leading-relaxed outline-none"
			oninput={() => {
				emit();
				readMarks();
			}}
			onkeyup={readMarks}
			onmouseup={readMarks}
			onfocus={readMarks}
			{onpaste}
			{onkeydown}
		></div>
	{/if}
</div>

<style>
	.rich:empty::before {
		content: attr(data-placeholder);
		color: var(--muted-foreground);
		pointer-events: none;
	}
	.rich :global(p),
	.rich :global(div) {
		margin: 0 0 0.6em;
	}
	.rich :global(ul) {
		list-style: disc;
		padding-left: 1.4em;
		margin: 0 0 0.6em;
	}
	.rich :global(ol) {
		list-style: decimal;
		padding-left: 1.4em;
		margin: 0 0 0.6em;
	}
	.rich :global(a) {
		text-decoration: underline;
		text-decoration-color: var(--brand);
		text-decoration-thickness: 2px;
		text-underline-offset: 2px;
	}
	.rich :global(blockquote) {
		border-left: 2px solid var(--border);
		padding-left: 0.8em;
		color: var(--muted-foreground);
	}
	.rich :global(h1),
	.rich :global(h2),
	.rich :global(h3) {
		font-weight: 600;
		margin: 0.6em 0 0.4em;
	}
</style>
