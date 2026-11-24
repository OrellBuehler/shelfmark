import type { Box } from "./reader.svelte";

export function floatAt(box: Box, w: number, h: number, vw: number, vh: number, top = 64) {
	const gap = 10;
	let y = box.top - h - gap;
	if (y < top) y = box.top + box.height + gap;
	if (y + h > vh - 12) y = Math.max(top, vh - h - 12);
	const x = Math.min(Math.max(box.left + box.width / 2 - w / 2, 12), Math.max(12, vw - w - 12));
	return { x, y };
}
