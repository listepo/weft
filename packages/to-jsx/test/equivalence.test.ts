// The generated component against the reference renderer, on every corpus screen and catalog
// example. Both are server-rendered with the same data and read back with `fromDom`, which keeps
// exactly what the accessibility tree and the document ids carry (roles, names, states, ids,
// nesting, text) and drops what does not matter there (attribute order, event handlers, React's
// text-node markers). Comparing the imported documents is therefore comparing the
// accessibility-relevant structure. The markup itself is compared too, because the renderer's
// boxes that the tree does not show (named-slot wrappers, aria-hidden captions) are part of the
// structure a host styles.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { fromDom } from "@weft/from-aria";
import { render } from "@weft/render-react";
import { parseSync } from "oxc-parser";
import { renderToStaticMarkup } from "react-dom/server";
import { toJsx } from "../src/index.ts";
import { load, markup, normalizeMarkup } from "./compile.ts";

const corpus = new URL("../../../corpus/", import.meta.url);
const examples = new URL("../../catalog/examples/", import.meta.url);

type Case = { name: string; document: Document; data: unknown };

function cases(): Case[] {
  const out: Case[] = [];
  for (const entry of readdirSync(corpus, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const dir = new URL(`${entry.name}/`, corpus);
    out.push({
      name: `corpus/${entry.name}`,
      document: read(new URL("screen.weft", dir)),
      data: JSON.parse(readFileSync(new URL("data.json", dir), "utf8")),
    });
  }
  for (const file of readdirSync(examples)
    .filter((f) => f.endsWith(".weft"))
    .sort()) {
    out.push({ name: `examples/${file}`, document: read(new URL(file, examples)), data: {} });
  }
  return out;
}

function read(url: URL): Document {
  const result = parse(readFileSync(url, "utf8"), { catalog: coreCatalog });
  assert.ok(result.document, `${url.pathname} parses`);
  return result.document;
}

for (const c of cases()) {
  test(`equivalence: ${c.name}`, async (t) => {
    const source = toJsx(c.document, { catalog: coreCatalog });
    const parsed = parseSync("screen.jsx", source);
    assert.deepEqual(parsed.errors, [], "the output parses without errors");
    const component = await load(source);
    const reference = renderToStaticMarkup(
      render(c.document, { catalog: coreCatalog, data: c.data }),
    );
    const generated = markup(component, { data: c.data });
    const expected = fromDom(reference, { catalog: coreCatalog });
    const actual = fromDom(generated, { catalog: coreCatalog });
    await t.test("same accessible structure", () => {
      assert.deepEqual(actual.document, expected.document);
      assert.deepEqual(actual.losses, expected.losses);
    });
    await t.test("same markup", () => {
      assert.equal(normalizeMarkup(generated), normalizeMarkup(reference));
    });
  });
}
