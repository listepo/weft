#!/usr/bin/env node
// Builds the plugin into `dist/`: `code.js` for Figma's main thread and `ui.html` for its UI
// iframe. Run with `node build.ts [outDir]`; the tests build into a temp folder. The build itself
// is shared with the Penpot plugin (`@weft/design-plugin`).
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildDesignPlugin } from "@weft/design-plugin/build";

const PLUGIN = dirname(fileURLToPath(import.meta.url));

export function buildPlugin(outDir: string): Promise<void> {
  return buildDesignPlugin({
    root: PLUGIN,
    outDir,
    sandbox: { entry: "src/code.ts", file: "code.js" },
    ui: { entry: "src/ui.ts", tool: "Figma", layer: "frame" },
  });
}

if (import.meta.main) {
  await buildPlugin(process.argv[2] ?? join(PLUGIN, "dist"));
}
