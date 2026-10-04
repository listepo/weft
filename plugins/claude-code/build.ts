#!/usr/bin/env node
// Bundles the plugin's scripts and the MCP server into `dist/`, so the plugin folder alone works:
// Claude Code copies only that folder into its cache, without the workspace packages or their
// dependencies. Run with `node build.ts [outDir]`; the tests rebuild into a temp folder and compare.
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

const PLUGIN = dirname(fileURLToPath(import.meta.url));
const REPOSITORY = join(PLUGIN, "../..");

// `import.meta.main` (Node 24.2) is how an entry knows it was started as a program. Older Node
// leaves it undefined, so an entry would end without a word; this says why instead.
const NODE_GUARD = `if (import.meta.main === undefined) {
  process.stderr.write("weft: Node 24.2 or later is required, this is " + process.version + "\\n");
  process.exit(2);
}`;

export const ENTRIES = {
  import: join(PLUGIN, "scripts/import.ts"),
  export: join(PLUGIN, "scripts/export.ts"),
  render: join(PLUGIN, "scripts/render.ts"),
  server: join(REPOSITORY, "packages/mcp/src/server.ts"),
};

/**
 * The WebAssembly core (`moon run root:wasm`). `@weft/core` reads it with
 * `new URL("../wasm/weft_bg.wasm", import.meta.url)` from the shared chunk, which is built into
 * `dist/chunks/`, so the copy lands in `dist/wasm/`. A bundler does not follow that URL.
 */
export const WASM = join(REPOSITORY, "packages/core/wasm/weft_bg.wasm");

/** The skill reads this copy: a symlink to the repository root would not survive the plugin cache. */
export const SPEC_COPY = join(PLUGIN, "skills/spec/AGENT-SPEC.md");

/** Builds the bundles and the WebAssembly module into `outDir` and the spec copy into `specCopy`. */
export async function buildPlugin(outDir: string, specCopy: string): Promise<void> {
  if (!existsSync(WASM)) throw new Error(`${WASM} is missing: run \`moon run root:wasm\` first`);
  rmSync(outDir, { recursive: true, force: true });
  await build({
    root: PLUGIN,
    configFile: false,
    logLevel: "warn",
    // Vite's `ssr` build targets Node and keeps `node:` built-ins external. `noExternal` bundles
    // every package, because the cache has no `node_modules`.
    ssr: { noExternal: true, target: "node" },
    define: { "process.env.NODE_ENV": JSON.stringify("production") },
    build: {
      ssr: true,
      outDir,
      emptyOutDir: true,
      target: "node24",
      // The bundles are committed, so their size is repository size.
      minify: true,
      sourcemap: false,
      reportCompressedSize: false,
      rolldownOptions: {
        input: ENTRIES,
        output: {
          format: "esm",
          banner: NODE_GUARD,
          entryFileNames: "[name].js",
          // Hash-free names: the bundle is committed, so a rebuild must reproduce it byte for byte.
          chunkFileNames: "chunks/[name].js",
        },
      },
    },
  });
  mkdirSync(join(outDir, "wasm"), { recursive: true });
  copyFileSync(WASM, join(outDir, "wasm/weft_bg.wasm"));
  mkdirSync(dirname(specCopy), { recursive: true });
  copyFileSync(join(REPOSITORY, "AGENT-SPEC.md"), specCopy);
}

if (import.meta.main) {
  await buildPlugin(join(PLUGIN, "dist"), SPEC_COPY);
}
