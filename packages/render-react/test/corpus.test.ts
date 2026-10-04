// Every corpus screen parses, renders on the server and declares a tree, so the corpus is covered
// on machines without Chromium too (the browser test compares the trees).
import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { corpusScreens } from "../src/corpus.ts";
import { expectedTree, renderPage } from "../src/index.ts";

const screens = corpusScreens();

test("the corpus has its twelve screens", () => {
  assert.equal(screens.length, 12);
});

for (const { name, document, diagnostics, data } of screens) {
  test(`corpus ${name} renders on the server and declares a tree`, () => {
    assert.deepEqual(diagnostics, []);
    assert.ok(document);
    const page = renderPage(document, { catalog: coreCatalog, data });
    assert.match(page, new RegExp(`<main data-weft-id="${document.root.id}"`));
    const tree = expectedTree(document, { catalog: coreCatalog, data });
    assert.equal(tree.children?.[0]?.role, "main");
  });
}
