// Builds a Weft design-tool plugin into two files: a script for the tool's sandbox and a page for
// its UI iframe. Both Figma and Penpot split a plugin this way, and both sandboxes lack
// WebAssembly, so the two plugins share this build and differ only in their entries.
//
// - The sandbox gets one classic script in which the core's WebAssembly loader is replaced by a
//   stub (`no-wasm.ts`): the sandbox runs no modules and no WebAssembly.
// - The UI is one HTML page with the core and its module bytes inlined (`wasm-inline.ts`), so it
//   needs no files beside it and no network.
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build, type Plugin } from "vite";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPOSITORY = join(HERE, "../../..");
const LOADER = join(REPOSITORY, "packages/core/src/wasm.ts");
// The web module (`@weft/core/web`) holds the HTML and JSX importers and generators, which a
// design-tool plugin never calls; both halves get the stub, so neither carries nor fetches it.
const WEB_LOADER = join(REPOSITORY, "packages/core/src/web.ts");
const NO_WASM = join(HERE, "no-wasm.ts");
const GLUE = join(REPOSITORY, "packages/core/wasm/weft.js");
const MODULE = join(REPOSITORY, "packages/core/wasm/weft_bg.wasm");
const TOKENS = join(REPOSITORY, "packages/catalog/tokens/default.tokens.json");

/** Resolves `from` to `to` for every importer except `to` itself, which imports the original. */
export function redirect(from: string, to: string): Plugin {
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
 * `wasm-inline.ts` hands the glue the bytes. A changed glue fails the build here, not in the tool.
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

export type PluginBuild = {
  /** The plugin's folder; entries are relative to it. */
  root: string;
  outDir: string;
  /** The sandbox entry and the file name it is written to. */
  sandbox: { entry: string; file: string };
  /** The UI entry, and how the page names the tool and the layer it builds. */
  ui: { entry: string; tool: string; layer: string };
};

export async function buildDesignPlugin(options: PluginBuild): Promise<void> {
  const shared = {
    root: options.root,
    configFile: false as const,
    logLevel: "warn" as const,
    build: { emptyOutDir: false, minify: true, sourcemap: false, reportCompressedSize: false },
  };
  await build({
    ...shared,
    plugins: [redirect(LOADER, NO_WASM), redirect(WEB_LOADER, NO_WASM)],
    build: {
      ...shared.build,
      outDir: options.outDir,
      emptyOutDir: true,
      target: "es2020",
      lib: {
        entry: options.sandbox.entry,
        formats: ["iife"],
        name: "weft",
        fileName: () => options.sandbox.file,
      },
      // The workspace packages re-export importers and renderers the plugin never calls
      // (from-aria pulls in React through render-react); none run code on import, so unused
      // modules go.
      rolldownOptions: { treeshake: { moduleSideEffects: false } },
    },
  });

  const ui = join(options.outDir, "ui");
  await build({
    ...shared,
    plugins: [
      redirect(GLUE, join(HERE, "wasm-inline.ts")),
      redirect(WEB_LOADER, NO_WASM),
      dropWasmUrls([LOADER, GLUE]),
    ],
    define: {
      WEFT_DEFAULT_TOKENS: readFileSync(TOKENS, "utf8"),
      WEFT_WASM_BASE64: JSON.stringify(readFileSync(MODULE).toString("base64")),
    },
    build: {
      ...shared.build,
      outDir: ui,
      // The core's loader awaits the module at the top level, which needs a module script.
      target: "es2022",
      lib: { entry: options.ui.entry, formats: ["es"], fileName: () => "ui.js" },
      rolldownOptions: { treeshake: { moduleSideEffects: false } },
    },
  });
  // `</script` inside the code would end the inline script early.
  const script = readFileSync(join(ui, "ui.js"), "utf8").replaceAll("</script", "<\\/script");
  const page = readFileSync(join(HERE, "ui.html"), "utf8")
    .replaceAll("{{tool}}", options.ui.tool)
    .replaceAll("{{layer}}", options.ui.layer)
    .replace("<!-- weft:script -->", () => `<script type="module">\n${script}</script>`);
  writeFileSync(join(options.outDir, "ui.html"), page);
  rmSync(ui, { recursive: true, force: true });
}
