// @ts-nocheck
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, sep } from "node:path";

const DEV_BASE = "/node_modules/foliate-js/vendor/pdfjs/";
const BUILD_BASE = "/foliate-pdfjs/";
const SKIP = new Set(["pdf.mjs"]);

function walk(dir) {
	return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
		e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)]
	);
}

export function foliate() {
	let serve = true;
	let root = process.cwd();
	return {
		name: "shelfmark-foliate",
		enforce: "pre",
		config: () => ({ optimizeDeps: { exclude: ["foliate-js"] } }),
		configResolved(config) {
			serve = config.command === "serve";
			root = config.root;
		},
		transform(code, id) {
			if (!id.split("?")[0].replaceAll("\\", "/").endsWith("foliate-js/pdf.js")) return;
			const base = serve ? DEV_BASE : BUILD_BASE;
			return code.replace(
				/const pdfjsPath = .*\n/,
				`const pdfjsPath = path => new URL(${JSON.stringify(base)} + path, location.origin).toString()\n`
			);
		},
		generateBundle() {
			if (serve) return;
			const dir = join(root, "node_modules/foliate-js/vendor/pdfjs");
			for (const file of walk(dir)) {
				const rel = relative(dir, file).split(sep).join("/");
				if (SKIP.has(rel) || rel.endsWith(".map")) continue;
				this.emitFile({ type: "asset", fileName: BUILD_BASE.slice(1) + rel, source: readFileSync(file) });
			}
		}
	};
}
