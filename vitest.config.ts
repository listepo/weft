import { defineConfig } from "vitest/config";

// One config for every package: moon runs `vitest run` inside each package, which is the config's root.
// Tests import the TypeScript sources directly (Vite transforms them), so no build step stands between a change and its tests.
export default defineConfig({
  test: {
    include: ["test/**/*.test.ts"],
    // The suites ran without a per-test timeout before; the browser and the differential ones are slower than Vitest's 5 s default.
    testTimeout: 120_000,
    hookTimeout: 120_000,
  },
});
