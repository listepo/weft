import { defineConfig } from "vitest/config";

// One config for every package: moon runs `vitest run` inside each package, which is the config's root.
// Tests import the TypeScript sources directly (Vite transforms them), so no build step stands between a change and its tests.
export default defineConfig({
  test: {
    // `test/` is the suite when moon runs inside a package. The second glob is the same
    // suites addressed from the repository root (`vitest run packages/<name>/test`).
    include: ["test/**/*.test.ts", "packages/*/test/**/*.test.ts"],
    // The suites ran without a per-test timeout before; the browser and the differential ones are slower than Vitest's 5 s default.
    testTimeout: 120_000,
    hookTimeout: 120_000,
  },
});
