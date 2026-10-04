import { readFileSync } from "node:fs";
import { defineConfig } from "vite";

// Figma loads `main` as one classic script with no module loader, so the code is a single IIFE.
const tokens = readFileSync(
  new URL("../../packages/catalog/tokens/default.tokens.json", import.meta.url),
  "utf8",
);

export default defineConfig({
  root: import.meta.dirname,
  logLevel: "warn",
  define: { WEFT_DEFAULT_TOKENS: tokens },
  build: {
    target: "es2020",
    outDir: "dist",
    emptyOutDir: true,
    lib: {
      entry: "src/code.ts",
      formats: ["iife"],
      name: "weft",
      fileName: () => "code.js",
    },
    // The workspace packages re-export importers and renderers the plugin never calls (from-aria
    // pulls in React through render-react); none of them run code on import, so unused modules go.
    rolldownOptions: { treeshake: { moduleSideEffects: false } },
  },
});
