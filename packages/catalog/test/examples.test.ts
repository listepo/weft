import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import type { PropDef } from "@weft/core";
import { coreCatalog } from "../src/core.ts";

const dir = new URL("../examples/", import.meta.url);
const TAG =
  /<(\/?)([a-z][a-z0-9]*(?:-[a-z0-9]+)*)((?:\s+[a-z][a-z0-9]*(?:-[a-z0-9]+)*="[^"<]*")*)\s*(\/?)>/g;
const ATTR = /([a-z][a-z0-9-]*)="([^"]*)"/g;

type El = { name: string; attrs: Record<string, string>; parent: El | undefined };

// A deliberately small well-formedness check; the real parser lives in @weft/core.
function scan(source: string): El[] {
  const elements: El[] = [];
  const stack: El[] = [];
  let last = 0;
  for (const m of source.matchAll(TAG)) {
    assert.ok(!source.slice(last, m.index).includes("<"), `stray "<" before offset ${m.index}`);
    last = m.index + m[0].length;
    const [, closing, name, attrText, selfClosing] = m as unknown as [
      string,
      string,
      string,
      string,
      string,
    ];
    if (closing) {
      assert.equal(stack.pop()?.name, name, `mismatched </${name}>`);
      continue;
    }
    const attrs: Record<string, string> = {};
    for (const a of attrText.matchAll(ATTR)) attrs[a[1]!] = a[2]!;
    const el: El = { name, attrs, parent: stack.at(-1) };
    elements.push(el);
    if (!selfClosing) stack.push(el);
  }
  assert.ok(!source.slice(last).includes("<"), "trailing garbage");
  assert.equal(stack.length, 0, "unclosed element");
  return elements;
}

const files = readdirSync(dir).filter((f) => f.endsWith(".weft"));
const kinds = Object.keys(coreCatalog.components);

test("every catalog kind has exactly one example", () => {
  assert.deepEqual(files.map((f) => f.slice(0, -5)).sort(), [...kinds].sort());
});

for (const file of files) {
  test(`example ${file} is well formed and uses the catalog`, () => {
    const elements = scan(readFileSync(new URL(file, dir), "utf8"));
    const root = elements[0];
    assert.equal(root?.name, "screen");
    assert.equal(root.attrs["weft"], "0.1");
    assert.ok(
      elements.some((e) => e.name === file.slice(0, -5)),
      "kind not used in its own example",
    );

    const ids = new Set<string>();
    for (const el of elements) {
      if (el.name === "slot") {
        assert.ok(el.attrs["name"] && !el.attrs["id"]);
        continue;
      }
      const def = coreCatalog.components[el.name];
      assert.ok(def || el.name === "each", `unknown element ${el.name}`);
      const id = el.attrs["id"];
      assert.ok(id && /^[A-Za-z][A-Za-z0-9_-]*$/.test(id), `${el.name} needs a valid id`);
      assert.ok(!ids.has(id), `duplicate id ${id}`);
      ids.add(id);
      if (!def) continue;
      if (def.requiresLabel) assert.ok(el.attrs["label"], `${id} needs label`);
      for (const [name, value] of Object.entries(el.attrs)) {
        if (["id", "label", "hidden", "state"].includes(name)) continue;
        if (name.startsWith("on-")) {
          assert.ok(def.events?.includes(name.slice(3)), `${id}: unknown event ${name}`);
          continue;
        }
        const prop: PropDef | undefined = def.props?.[name];
        assert.ok(prop, `${id}: unknown prop ${name}`);
        if (prop.type === "boolean")
          assert.ok(/^(true|false|\{!?\$[^}]*\})$/.test(value), `${id}.${name}`);
        if (prop.type === "enum" && !value.startsWith("{"))
          assert.ok(prop.values?.includes(value), `${id}.${name}`);
      }
      if (el.attrs["state"]) assert.ok(def.states?.includes(el.attrs["state"]), `${id}: bad state`);
      if (def.allowedParents) {
        let parent = el.parent;
        while (parent?.name === "each") parent = parent.parent;
        assert.ok(parent && def.allowedParents.includes(parent.name), `${id}: bad parent`);
      }
    }
  });
}
