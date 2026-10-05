// Stands in for the web screenshots when Playwright's Chromium is not installed (vitest.config.ts).
import { test } from "vitest";

test.skip("web screenshots skipped: Chromium is missing; install it with `pnpm exec playwright install chromium`", () => {});
