// The fixtures are documents a writer could have produced, so they follow the current SPEC; only
// the extension fixture breaks it on purpose, to exercise the SPEC §8 fallbacks.
import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { validate } from "@weft/core";
import { fixtures } from "./fixtures.ts";

const DELIBERATELY_INVALID = new Set(["extension"]);

for (const { name, document } of fixtures()) {
  if (DELIBERATELY_INVALID.has(name)) continue;
  test(`fixture ${name} is valid in strict mode`, () => {
    const found = validate(document, { catalog: coreCatalog, mode: "strict" });
    assert.deepEqual(
      found.map((d) => `${d.code} ${d.path}`),
      [],
    );
  });
}
