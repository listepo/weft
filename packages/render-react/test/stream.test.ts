// Streaming (SPEC §6.3): a model writes a screen a chunk at a time, and every prefix of it parses
// into a document the renderer can show.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse } from "@weft/core";
import { CORPUS_DIR } from "../src/corpus.ts";
import { renderPage } from "../src/index.ts";

/** What a cut can explain; the same list as the Rust test of the same name. */
const TAIL = new Set(["W110", "W114", "W115", "W208", "W309", "W314", "W803"]);
const STEP = 11;

const screens = readdirSync(CORPUS_DIR, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => ({
    name: e.name,
    markup: readFileSync(new URL(`${e.name}/screen.weft`, CORPUS_DIR), "utf8"),
    data: JSON.parse(readFileSync(new URL(`${e.name}/data.json`, CORPUS_DIR), "utf8")) as unknown,
  }));

const ids = (html: string) => html.match(/data-weft-id="/g)?.length ?? 0;

for (const { name, markup, data } of screens) {
  test(`corpus ${name} cut at every unit is a valid prefix that renders`, () => {
    const options = { catalog: coreCatalog, mode: "strict", partial: true } as const;
    const whole = renderPage(parse(markup, { catalog: coreCatalog, mode: "strict" }).document!, {
      catalog: coreCatalog,
      data,
    });
    const rootTagEnd = markup.indexOf(">") + 1;
    let shown = 0;
    for (let cut = 0; cut <= markup.length; cut++) {
      const { document, diagnostics, pending } = parse(markup.slice(0, cut), options);
      const at = `${name} cut at ${cut}`;
      assert.deepEqual(diagnostics, [], at);
      for (const d of pending ?? []) assert.ok(TAIL.has(d.code), `${at}: ${d.code}`);
      if (document === undefined) {
        assert.ok(cut < rootTagEnd, `${at}: no document`);
        continue;
      }
      if (cut % STEP !== 0 && cut !== markup.length) continue;
      const page = renderPage(document, { catalog: coreCatalog, data });
      // The prefix only grows: a later chunk never removes an element from the page.
      const count = ids(page);
      assert.ok(count >= shown, `${at}: ${count} elements after ${shown}`);
      shown = count;
      if (cut === markup.length) {
        assert.deepEqual(pending, [], at);
        assert.equal(page, whole, at);
      }
    }
  });
}

test("a partial document renders what has arrived", () => {
  const markup =
    '<screen id="s" weft="0.1"><stack id="a"><text id="one">Hello</text><text id="two">Wor';
  const { document, diagnostics, pending } = parse(markup, {
    catalog: coreCatalog,
    mode: "strict",
    partial: true,
  });
  assert.deepEqual(diagnostics, []);
  assert.deepEqual(
    pending?.map((d) => d.path),
    ["/screen#s", "/screen#s/stack#a", "/screen#s/stack#a/text#two"],
  );
  assert.ok(document);
  const page = renderPage(document, { catalog: coreCatalog });
  assert.match(page, /data-weft-id="one"[^>]*>Hello</);
  assert.match(page, /data-weft-id="two"[^>]*>Wor</);
});

test("without the option a cut is a syntax error and nothing renders", () => {
  const { document, diagnostics, pending } = parse('<screen id="s" weft="0.1"><stack id="a">', {
    catalog: coreCatalog,
  });
  assert.equal(document, undefined);
  assert.equal(pending, undefined);
  assert.deepEqual(
    diagnostics.map((d) => d.code),
    ["W110", "W110"],
  );
});
