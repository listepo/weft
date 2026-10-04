#!/usr/bin/env node
// Builds the plugin into `dist/`: `code.js` for Figma's main thread and `ui.html` for its UI
// iframe. Run with `node build.ts [outDir]`; the tests build into a temp folder.
//
// The two halves differ because Figma runs them in different places. The main thread is a
// sandbox with no WebAssembly, no fetch and no modules: it gets one classic script in which the
// core's WebAssembly loader is replaced by a stub (`src/no-wasm.ts`). The UI is a browser page
// loaded from one HTML string: it gets the core with its module bytes inlined (`src/wasm-inline.ts`).
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build, type Plugin } from "vite";

const PLUGIN = dirname(fileURLToPath(import.meta.url));
const REPOSITORY = join(PLUGIN, "../..");
const LOADER = join(REPOSITORY, "packages/core/src/wasm.ts");
const GLUE = join(REPOSITORY, "packages/core/wasm/weft.js");
const MODULE = join(REPOSITORY, "packages/core/wasm/weft_bg.wasm");
const TOKENS = join(REPOSITORY, "packages/catalog/tokens/default.tokens.json");

/** Resolves `from` to `to` for every importer except `to` itself, which imports the original. */
function redirect(from: string, to: string): Plugin {
  return {
    name: `weft-redirect-${from}`,
    enforce: "pre",
    async resolveId(source, importer, options) {
      if (importer === to) return null;
      const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
      return resolved?.id === from ? to : null;
    },
  };
}

const WASM_URL = /new URL\((["'])[^"']*weft_bg\.wasm\1, import\.meta\.url\)/g;

/**
 * Drops the loader's and the glue's own references to `weft_bg.wasm`. Vite inlines each one as a
 * data URL, which would put the 1 MB module into the page twice more, and neither is reached:
 * `src/wasm-inline.ts` hands the glue the bytes. A changed glue fails the build here, not in Figma.
 */
function dropWasmUrls(ids: readonly string[]): Plugin {
  return {
    name: "weft-drop-wasm-urls",
    transform(code, id) {
      if (!ids.includes(id)) return null;
      const matches = code.match(WASM_URL)?.length ?? 0;
      if (matches !== 1) throw new Error(`${id}: expected one weft_bg.wasm URL, found ${matches}`);
      return code.replace(WASM_URL, "undefined");
    },
  };
}

const shared = {
  root: PLUGIN,
  configFile: false as const,
  logLevel: "warn" as const,
  build: { emptyOutDir: false, minify: true, sourcemap: false, reportCompressedSize: false },
};

export async function buildPlugin(outDir: string): Promise<void> {
  await build({
    ...shared,
    plugins: [redirect(LOADER, join(PLUGIN, "src/no-wasm.ts"))],
    build: {
      ...shared.build,
      outDir,
      emptyOutDir: true,
      target: "es2020",
      lib: { entry: "src/code.ts", formats: ["iife"], name: "weft", fileName: () => "code.js" },
      // The workspace packages re-export importers and renderers the plugin never calls
      // (from-aria pulls in React through render-react); none run code on import, so unused
      // modules go.
      rolldownOptions: { treeshake: { moduleSideEffects: false } },
    },
  });

  const ui = join(outDir, "ui");
  await build({
    ...shared,
    plugins: [redirect(GLUE, join(PLUGIN, "src/wasm-inline.ts")), dropWasmUrls([LOADER, GLUE])],
    define: {
      WEFT_DEFAULT_TOKENS: readFileSync(TOKENS, "utf8"),
      WEFT_WASM_BASE64: JSON.stringify(readFileSync(MODULE).toString("base64")),
    },
    build: {
      ...shared.build,
      outDir: ui,
      // The core's loader awaits the module at the top level, which needs a module script.
      target: "es2022",
      lib: { entry: "src/ui.ts", formats: ["es"], fileName: () => "ui.js" },
      rolldownOptions: { treeshake: { moduleSideEffects: false } },
    },
  });
  // `</script` inside the code would end the inline script early.
  const script = readFileSync(join(ui, "ui.js"), "utf8").replaceAll("</script", "<\\/script");
  const page = readFileSync(join(PLUGIN, "ui.html"), "utf8").replace(
    "<!-- weft:script -->",
    () => `<script type="module">\n${script}</script>`,
  );
  writeFileSync(join(outDir, "ui.html"), page);
  rmSync(ui, { recursive: true, force: true });
}

if (import.meta.main) {
  await buildPlugin(process.argv[2] ?? join(PLUGIN, "dist"));
}
