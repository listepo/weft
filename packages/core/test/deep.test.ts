// A cycle and a value past the depth limit must come back as W200, the same diagnostic the
// binding writes into crates/weft-binding/tests/fixtures/deep-documents.json. An empty screen
// would be a successful-looking write of input that was never read.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { canonicalize, expand, serialize, stringify, type Document } from "../src/index.ts";
import { catalog } from "./catalog.ts";

const fixture = JSON.parse(
  readFileSync(
    new URL("../../../crates/weft-binding/tests/fixtures/deep-documents.json", import.meta.url),
    "utf8",
  ),
) as {
  reported: { code: string }[];
  expand: { document?: Document; diagnostics: { code: string }[] };
};

function cyclic(): Document {
  const doc = { weft: "0.1", root: { kind: "screen", id: "s" } } as Document & { extra?: unknown };
  doc.extra = doc;
  return doc;
}

function tooDeep(): Document {
  let value: unknown = 0;
  for (let i = 0; i < 2_000; i++) value = [value];
  return value as Document;
}

const emptyMarkup = serialize(null as unknown as Document);
const emptyJson = stringify(null as unknown as Document);
const emptyDocument = canonicalize(null as unknown as Document);

for (const [name, document] of [
  ["cyclic", cyclic()],
  ["too-deep", tooDeep()],
] as const) {
  test(`${name} serialize is W200, not an empty screen`, () => {
    const markup = serialize(document);
    assert.deepEqual(JSON.parse(markup), fixture.reported);
    assert.notEqual(markup, emptyMarkup);
  });

  test(`${name} stringify is W200, not an empty screen`, () => {
    const json = stringify(document);
    assert.deepEqual(JSON.parse(json), fixture.reported);
    assert.notEqual(json, emptyJson);
  });

  test(`${name} canonicalize is W200, not an empty screen`, () => {
    const canonical = canonicalize(document);
    assert.deepEqual(canonical, fixture.reported);
    assert.notDeepEqual(canonical, emptyDocument);
  });

  test(`${name} expand is W200 with no document`, () => {
    const result = expand(document, catalog);
    assert.equal(result.document, undefined);
    assert.deepEqual(result.diagnostics, fixture.expand.diagnostics);
  });
}
