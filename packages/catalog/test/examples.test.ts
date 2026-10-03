import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { parse, type Child } from "@weft/core";
import { coreCatalog } from "../src/core.ts";

// Validity of every example in strict mode is checked by bench/test/conformance.test.ts, together
// with the corpus; this file checks that the examples cover the catalog.
const dir = new URL("../examples/", import.meta.url);
const files = readdirSync(dir).filter((f) => f.endsWith(".weft"));
const kinds = Object.keys(coreCatalog.components);

function kindsIn(children: readonly Child[] | undefined, out: Set<string>): Set<string> {
  for (const c of children ?? []) {
    if (typeof c === "string") continue;
    out.add(c.kind);
    kindsIn(c.children, out);
    for (const list of Object.values(c.slots ?? {})) kindsIn(list, out);
  }
  return out;
}

test("every catalog kind has exactly one example", () => {
  assert.deepEqual(files.map((f) => f.slice(0, -5)).sort(), [...kinds].sort());
});

for (const file of files) {
  test(`example ${file} uses its own kind`, () => {
    const { document } = parse(readFileSync(new URL(file, dir), "utf8"));
    assert.ok(document, file);
    assert.ok(kindsIn([document.root], new Set()).has(file.slice(0, -5)));
  });
}
