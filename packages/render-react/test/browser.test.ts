// Renders every fixture to a static page in Chromium and checks that the accessibility snapshot
// equals the tree the document declares. Skips when the Chromium binary is not installed, so CI
// does not depend on a browser download (`pnpm exec playwright install chromium`).
import assert from "node:assert/strict";
import { after, before, describe, test } from "node:test";
import { chromium, type Browser } from "playwright";
import { coreCatalog } from "@weft/catalog";
import { diffAria, expectedTree, parseAriaSnapshot, renderPage } from "../src/index.ts";
import { fixtures } from "./fixtures.ts";

let browser: Browser | undefined;
let skipReason = "";

describe("browser accessibility snapshot", () => {
  before(async () => {
    try {
      browser = await chromium.launch();
    } catch (error) {
      skipReason = `Chromium is not available (run \`pnpm exec playwright install chromium\`): ${
        String(error).split("\n")[0]
      }`;
    }
  });
  after(async () => {
    await browser?.close();
  });

  for (const { name, document, data } of fixtures()) {
    test(`fixture ${name} matches expectedTree`, async (t) => {
      if (!browser) {
        t.skip(skipReason);
        return;
      }
      const page = await browser.newPage();
      try {
        await page.setContent(renderPage(document, { catalog: coreCatalog, data }));
        const actual = parseAriaSnapshot(await page.locator("body").ariaSnapshot());
        const expected = expectedTree(document, { catalog: coreCatalog, data });
        assert.deepEqual(diffAria(expected, actual), []);
      } finally {
        await page.close();
      }
    });
  }
});
