#!/usr/bin/env node
// Bundles the scripts and the MCP server that the plugins share, and writes the result into every
// plugin folder. A host copies only the plugin's own folder into its cache (Claude Code, Cursor),
// without the workspace packages or their dependencies, so each plugin carries a `dist/` and a copy
// of AGENT-SPEC.md. Both come from one build here: edit the sources, never the copies. Run with
// `node build.ts`; the tests rebuild into a temp folder and compare.
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";

const SHARED = dirname(fileURLToPath(import.meta.url));
const REPOSITORY = join(SHARED, "../..");

/** Where each plugin keeps its copy of the bundle and of the authoring guide its spec skill reads. */
export const PLUGINS = {
  "claude-code": { skill: "skills/spec" },
} as const;

export type PluginName = keyof typeof PLUGINS;

export const pluginDir = (name: PluginName): string => join(REPOSITORY, "plugins", name);
export const distDir = (name: PluginName): string => join(pluginDir(name), "dist");
export const specCopy = (name: PluginName): string =>
  join(pluginDir(name), PLUGINS[name].skill, "AGENT-SPEC.md");

// `import.meta.main` (Node 24.2) is how an entry knows it was started as a program. Older Node
// leaves it undefined, so an entry would end without a word; this says why instead.
const NODE_GUARD = `if (import.meta.main === undefined) {
  process.stderr.write("weft: Node 24.2 or later is required, this is " + process.version + "\\n");
  process.exit(2);
}`;

export const ENTRIES = {
  import: join(SHARED, "scripts/import.ts"),
  export: join(SHARED, "scripts/export.ts"),
  render: join(SHARED, "scripts/render.ts"),
  server: join(REPOSITORY, "packages/mcp/src/server.ts"),
};

/**
 * The WebAssembly core (`moon run root:wasm`). `@weft/core` reads it with
 * `new URL("../wasm/weft_bg.wasm", import.meta.url)` from the shared chunk, which is built into
 * `dist/chunks/`, so the copy lands in `dist/wasm/`. A bundler does not follow that URL.
 */
export const WASM = join(REPOSITORY, "packages/core/wasm/weft_bg.wasm");

/** Builds the bundles and the WebAssembly module into `outDir`. */
export async function buildBundle(outDir: string): Promise<void> {
  if (!existsSync(WASM)) throw new Error(`${WASM} is missing: run \`moon run root:wasm\` first`);
  rmSync(outDir, { recursive: true, force: true });
  await build({
    root: SHARED,
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
}

/**
 * Builds once and replaces every plugin's `dist/` and spec copy with the result. The spec copy is a
 * real file because a link out of the plugin folder would not survive the host's cache.
 */
export async function writeBundles(): Promise<void> {
  const scratch = mkdtempSync(join(tmpdir(), "weft-bundle-"));
  try {
    const built = join(scratch, "dist");
    await buildBundle(built);
    for (const name of Object.keys(PLUGINS) as PluginName[]) {
      rmSync(distDir(name), { recursive: true, force: true });
      cpSync(built, distDir(name), { recursive: true });
      mkdirSync(dirname(specCopy(name)), { recursive: true });
      copyFileSync(join(REPOSITORY, "AGENT-SPEC.md"), specCopy(name));
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

if (import.meta.main) {
  await writeBundles();
}
