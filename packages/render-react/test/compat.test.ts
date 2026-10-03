// SPEC §8 on the render side: documents from the future and from other vendors still render.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { coreCatalog } from "@weft/catalog";
import { hasErrors, parse } from "@weft/core";
import { expectedTree } from "../src/index.ts";
import { dom } from "./helpers.ts";

const read = (name: string) =>
  readFileSync(new URL(`../../../compat/${name}.weft`, import.meta.url), "utf8");

function load(name: string) {
  const { document, diagnostics } = parse(read(name), { catalog: coreCatalog });
  assert.ok(document);
  assert.equal(hasErrors(diagnostics), false);
  return document;
}

const readable = [
  "unknown-element",
  "unknown-attribute",
  "unknown-with-children",
  "unknown-in-restricted-parent",
  "extensions",
];

const tree = (name: string) => expectedTree(load(name), { catalog: coreCatalog }).children?.[0];

for (const name of readable) {
  test(`${name}: renders without throwing`, () => {
    assert.doesNotThrow(() => dom(load(name)));
  });
}

test("an unknown element renders as a group between its siblings", () => {
  const v = dom(load("unknown-element"));
  assert.equal(v.byId("holo").attribs["role"], "group");
  assert.equal(v.byId("holo").attribs["intensity"], undefined, "unknown attributes are ignored");
  assert.equal(v.text(v.byId("st")), "BeforeAfter");
  assert.deepEqual(tree("unknown-element")?.children, [
    { role: "text", name: "Before" },
    { role: "group", name: "" },
    { role: "text", name: "After" },
  ]);
});

test("an unknown attribute on a known element does not change the element", () => {
  const v = dom(load("unknown-attribute"));
  assert.equal(v.byId("go").name, "button");
  assert.equal(v.byId("go").attribs["glow"], undefined);
  assert.deepEqual(tree("unknown-attribute")?.children, [{ role: "button", name: "Go" }]);
});

test("an unknown element keeps its known children and slot content", () => {
  const v = dom(load("unknown-with-children"));
  assert.equal(v.byId("car").attribs["role"], "group");
  for (const id of ["intro", "next", "prev"]) assert.ok(v.has(id), id);
  assert.equal(v.byId("next").name, "button");
  assert.equal(v.text(v.byId("car")), "Pick oneNextPrevious");
  assert.deepEqual(tree("unknown-with-children")?.children, [
    {
      role: "group",
      name: "",
      children: [
        { role: "text", name: "Pick one" },
        { role: "button", name: "Next" },
        { role: "button", name: "Previous" },
      ],
    },
  ]);
});

test("an unknown element inside a restricted parent renders with its label and items", () => {
  const v = dom(load("unknown-in-restricted-parent"));
  assert.equal(v.byId("b1").attribs["role"], "group");
  assert.equal(v.byId("b1").attribs["aria-label"], "New");
  assert.ok(v.has("i1") && v.has("i2"));
  assert.deepEqual(tree("unknown-in-restricted-parent")?.children, [
    {
      role: "list",
      name: "",
      children: [
        { role: "listitem", name: "", children: [{ role: "text", name: "One" }] },
        {
          role: "group",
          name: "New",
          children: [{ role: "listitem", name: "", children: [{ role: "text", name: "Two" }] }],
        },
      ],
    },
  ]);
});

test("an extension element renders with its own role; extension attributes are ignored", () => {
  const v = dom(load("extensions"));
  assert.equal(v.byId("chart").attribs["role"], "img");
  assert.equal(v.byId("chart").attribs["aria-label"], "Sales");
  assert.equal(v.byId("chart").attribs["x-acme-kind"], undefined);
  assert.equal(v.byId("go").attribs["x-acme-density"], undefined);
  assert.ok(v.has("caption") && v.has("legend-btn"));
  assert.deepEqual(tree("extensions")?.children, [
    {
      role: "img",
      name: "Sales",
      children: [
        { role: "text", name: "Sales per month" },
        { role: "button", name: "Legend" },
      ],
    },
    { role: "button", name: "Go" },
  ]);
});
