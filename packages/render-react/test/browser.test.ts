// Renders every fixture and every corpus screen to a static page in Chromium and checks that the
// accessibility snapshot equals the tree the document declares. Skips when the Chromium binary is not installed, so CI
// does not depend on a browser download (`pnpm exec playwright install chromium`).
import assert from "node:assert/strict";
import { after, before, describe, test } from "node:test";
import { chromium, type Browser } from "playwright";
import { coreCatalog } from "@weft/catalog";
import type { Document } from "@weft/core";
import { corpusScreens } from "../src/corpus.ts";
import { diffAria, expectedTree, parseAriaSnapshot, renderPage } from "../src/index.ts";
import { fixtures } from "./fixtures.ts";

const screens = corpusScreens();
const screen = (name: string) => screens.find((s) => s.name === name);

// The corpus data shows each screen in one state; these show the states its documents declare
// but its data never reaches.
const variants = [
  { name: "search-results", state: "without results", data: { results: [] } },
  { name: "confirm-dialog", state: "with the dialog open", data: { confirmOpen: true } },
];

const cases: { title: string; document: Document | undefined; data: unknown }[] = [
  ...fixtures().map((f) => ({ ...f, title: `fixture ${f.name}` })),
  ...screens.map((s) => ({ ...s, title: `corpus ${s.name}` })),
  ...variants.map((v) => ({
    title: `corpus ${v.name} ${v.state}`,
    document: screen(v.name)?.document,
    data: { ...(screen(v.name)?.data as object), ...v.data },
  })),
];

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

  for (const { title, document, data } of cases) {
    test(`${title} matches expectedTree`, async (t) => {
      if (!browser) {
        t.skip(skipReason);
        return;
      }
      assert.ok(document, "the document parses");
      const page = await browser.newPage();
      // Documents may name remote images; the test must not depend on (or reach) the network.
      await page.route("**/*", (route) => route.abort());
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
