// The generated component against the reference renderer, on every corpus screen and catalog
// example. Both are server-rendered with the same data and read back with `fromDom`, which keeps
// exactly what the accessibility tree and the document ids carry (roles, names, states, ids,
// nesting, text) and drops what does not matter there (attribute order, event handlers, React's
// text-node markers). Comparing the imported documents is therefore comparing the
// accessibility-relevant structure.
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
import { load, markup } from "./compile.ts";

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

// Known gaps of the reference renderer that the generator does not copy, because it follows
// SPEC §5.1 (another branch is fixing the renderer):
// - text: `text` and `heading` read `value` instead of the `text` prop, and button, link,
//   menu-item, option, column and radio take their text from content only;
// - submit: a `submit` button renders as type="button";
// - empty: a list always shows its `empty` slot, and a table never does.
const GAPS: Record<string, string> = {
  "corpus/data-table": "text",
  "corpus/error-state": "text",
  "corpus/login": "submit",
  "corpus/menu": "text",
  "corpus/profile": "text",
  "corpus/search-results": "text, empty",
  "corpus/signup": "submit",
  "corpus/tabs": "text",
  "corpus/todo-list": "text, submit",
  "corpus/wizard-step": "text",
  "examples/form.weft": "submit",
  "examples/table.weft": "empty",
};

type Node = {
  kind: string;
  id: string;
  props?: Record<string, unknown>;
  children?: unknown[];
  slots?: Record<string, unknown[]>;
};

// Removes what the `text` and `submit` gaps change, so the rest of the structure is still
// compared on the screens they affect: text runs, the `text` prop, labels (a label is imported
// when the name differs from the visible text) and `submit`.
function withoutTextGaps(node: Node): Node {
  const props = { ...node.props };
  delete props["text"];
  delete props["label"];
  delete props["submit"];
  const out: Node = { kind: node.kind, id: node.id, props };
  const kids = (list: unknown[] | undefined) =>
    (list ?? []).filter((c): c is Node => typeof c !== "string").map(withoutTextGaps);
  const children = kids(node.children);
  if (children.length > 0) out.children = children;
  if (node.slots)
    out.slots = Object.fromEntries(Object.entries(node.slots).map(([k, v]) => [k, kids(v)]));
  return out;
}

for (const c of cases()) {
  test(`equivalence: ${c.name}`, async (t) => {
    const source = toJsx(c.document, { catalog: coreCatalog });
    const parsed = parseSync("screen.jsx", source);
    assert.deepEqual(parsed.errors, [], "the output parses without errors");
    const component = await load(source);
    const expected = fromDom(
      renderToStaticMarkup(render(c.document, { catalog: coreCatalog, data: c.data })),
      {
        catalog: coreCatalog,
      },
    );
    const actual = fromDom(markup(component, { data: c.data }), { catalog: coreCatalog });
    const gaps = GAPS[c.name];

    await t.test("same accessible structure", { todo: gaps && `renderer gap: ${gaps}` }, () => {
      assert.deepEqual(actual.document, expected.document);
      assert.deepEqual(actual.losses, expected.losses);
    });
    await t.test(
      "same structure without text",
      { todo: gaps?.includes("empty") ? "renderer gap: empty slot" : undefined },
      () => {
        const root = (d: Document) => withoutTextGaps(d.root as Node);
        assert.deepEqual(root(actual.document), root(expected.document));
      },
    );
  });
}
