#!/usr/bin/env node
// Builds the plugin into `dist/`: `plugin.js` for Penpot's sandbox and `ui.html` for its UI
// iframe, next to a copy of `manifest.json`, so `dist/` is the folder a host serves. Run with
// `node build.ts [outDir]`; the tests build into a temp folder. The build itself is shared with
// the Figma plugin (`@weft/design-plugin`).
import { copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { buildDesignPlugin } from "@weft/design-plugin/build";

const PLUGIN = dirname(fileURLToPath(import.meta.url));

export async function buildPlugin(outDir: string): Promise<void> {
  await buildDesignPlugin({
    root: PLUGIN,
    outDir,
    sandbox: { entry: "src/plugin.ts", file: "plugin.js" },
    ui: { entry: "src/ui.ts", tool: "Penpot", layer: "board" },
  });
  copyFileSync(join(PLUGIN, "manifest.json"), join(outDir, "manifest.json"));
}

if (import.meta.main) {
  await buildPlugin(process.argv[2] ?? join(PLUGIN, "dist"));
}
