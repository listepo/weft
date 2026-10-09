// Fragments (SPEC §10.7): a screen with uses draws what its hand-expanded copy draws, and hostile
// fragments are diagnostics, never a hang or a crash.
import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse, validate, type Catalog, type Document } from "@weft/core";
import { expectedTree } from "../src/index.ts";
import { b, doc, el, html } from "./helpers.ts";

function withFragments(files: Record<string, string>): Catalog {
  const fragments: Record<string, Document> = {};
  for (const [name, markup] of Object.entries(files)) {
    const { document } = parse(`<fragment weft="0.2">${markup}</fragment>`, {
      catalog: coreCatalog,
    });
    assert.ok(document, name);
    fragments[name] = document;
  }
  return { ...coreCatalog, fragments };
}

const catalog = withFragments({
  crumb: '<param name="go" type="action"/><link id="home" on-press="{$go}">Home</link>',
  header:
    '<param name="title" type="string" required="true"/><param name="back" type="action"/>' +
    '<param name="actions" type="slot"/>' +
    '<stack id="bar" direction="row"><button id="back" on-press="{$back}">Back</button>' +
    '<heading id="title" level="1" text="{$title}"/>' +
    '<use id="crumbs" fragment="crumb" on-go="{$back}"/><outlet name="actions"/></stack>',
});

const withUses = (): Document => {
  const markup =
    '<screen id="root" weft="0.2" label="Test">' +
    '<use id="top" fragment="header" title="{$.title}" on-back="nav.back">' +
    '<slot name="actions"><button id="clear" on-press="cart.clear">Clear</button></slot></use>' +
    '<each id="lines" in="{$.lines}" as="line">' +
    '<use id="row" fragment="header" title="{$line.name}" on-back="line.open"/></each></screen>';
  const { document } = parse(markup, { catalog });
  assert.ok(document);
  return document;
};

// What the two uses mean, written out by hand with instance paths for ids.
const header = (id: string, title: string, back: string, actions: Document["root"][]) =>
  el("stack", `${id}/bar`, { direction: "row" }, [
    el("button", `${id}/back`, {}, ["Back"], { on: { press: back } }),
    el("heading", `${id}/title`, { level: 1, text: b(title) }),
    el("link", `${id}/crumbs/home`, {}, ["Home"], { on: { press: back } }),
    ...actions,
  ]);
const handExpanded = (): Document => {
  const clear = el("button", "clear", {}, ["Clear"], { on: { press: "cart.clear" } });
  const rows = el("each", "lines", { in: b("$.lines"), as: "line" }, [
    header("row", "$line.name", "line.open", []),
  ]);
  return { ...doc(header("top", "$.title", "nav.back", [clear]), rows), weft: "0.2" };
};

const data = { title: "Cart", lines: [{ name: "Tea" }, { name: "Milk" }] };

test("a screen with nested uses has the accessibility tree of its hand-expanded copy", () => {
  assert.deepEqual(validate(withUses(), { catalog, mode: "strict" }), []);
  assert.deepEqual(
    expectedTree(withUses(), { catalog, data }),
    expectedTree(handExpanded(), { catalog: coreCatalog, data }),
  );
});

test("the reference renderer draws a use as its expansion, with instance paths", () => {
  const drawn = html(withUses(), { catalog, data });
  assert.equal(drawn, html(handExpanded(), { data }));
  assert.match(drawn, /data-weft-id="top\/crumbs\/home"/);
  assert.match(drawn, /data-weft-id="row\/title\[1\]"/);
});

test("a cycle of fragments is W805 and draws nothing of the cycle", () => {
  const cyclic = withFragments({
    a: '<stack id="s"><use id="b" fragment="b"/></stack>',
    b: '<use id="a" fragment="a"/>',
  });
  const { document } = parse('<screen id="root" weft="0.2"><use id="x" fragment="a"/></screen>', {
    catalog: cyclic,
  });
  assert.ok(document);
  const codes = validate(document, { catalog: cyclic }).map((d) => d.code);
  assert.deepEqual(codes, ["W805"]);
  assert.match(html(document, { catalog: cyclic }), /data-weft-id="x\/s"/);
  expectedTree(document, { catalog: cyclic });
});
