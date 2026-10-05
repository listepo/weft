// The binding suites (@weft/core and @weft/catalog) on every runtime that loads the core. One
// config for all of them: `WEFT_RUNTIME` picks the leg and the launcher (moon, see
// `root:runtimes`) picks the executable, so Deno and Bun run the very same test files Node does.
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { playwright } from "@vitest/browser-playwright";
import { chromium } from "playwright";
import { defineConfig } from "vitest/config";

export const runtimes = ["node", "deno", "bun", "browser"] as const;
export type Runtime = (typeof runtimes)[number];

const runtime = process.env["WEFT_RUNTIME"] ?? "node";
if (!runtimes.includes(runtime as Runtime)) {
  throw new Error(`WEFT_RUNTIME must be one of ${runtimes.join(", ")}, got "${runtime}"`);
}

// A page has no file system, so the suites that read the corpus, the specification, the fixtures or
// a temporary directory stay on the runtimes that have one. A new suite runs in the browser too
// until it is listed here, so a file-reading test fails loudly instead of silently skipping it.
const needsFileSystem = [
  "packages/core/test/cli.test.ts",
  "packages/core/test/compat.test.ts",
  "packages/core/test/differential.test.ts",
  "packages/core/test/roundtrip.test.ts",
  "packages/catalog/test/catalog-json.test.ts",
  "packages/catalog/test/differential.test.ts",
  "packages/catalog/test/examples.test.ts",
  "packages/catalog/test/node.test.ts",
  "packages/catalog/test/tokens.test.ts",
];

// The engine tests drive the native-addon loader, which only Node and Bun run (Deno and a page always
// use the WebAssembly module), and spawn Node and Bun processes of their own.
const needsAddonRuntime = ["packages/core/test/engines.test.ts"];

const browser = runtime === "browser";
// Unlike the visual suite, which skips its web screenshots, a matrix with a leg missing is not a
// pass, so a machine without Chromium fails with the way to install it.
if (browser && !existsSync(chromium.executablePath())) {
  throw new Error("the browser leg needs Chromium: run `pnpm exec playwright install chromium`");
}

export default defineConfig({
  root: resolve(import.meta.dirname, ".."),
  resolve: {
    alias: browser
      ? { "node:assert/strict": resolve(import.meta.dirname, "assert-browser.ts") }
      : {},
  },
  test: {
    name: runtime,
    include: ["packages/core/test/**/*.test.ts", "packages/catalog/test/**/*.test.ts"],
    exclude: browser
      ? [...needsFileSystem, ...needsAddonRuntime]
      : runtime === "deno"
        ? needsAddonRuntime
        : [],
    setupFiles: [resolve(import.meta.dirname, "setup.ts")],
    provide: { runtime },
    testTimeout: 120_000,
    hookTimeout: 120_000,
    ...(browser && {
      browser: {
        enabled: true,
        headless: true,
        provider: playwright(),
        instances: [{ browser: "chromium" as const }],
        // The suites assert on data, not pixels; the failure screenshots would only litter the tree.
        screenshotFailures: false,
      },
    }),
  },
});
