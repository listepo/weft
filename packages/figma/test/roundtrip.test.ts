// The stage-one done criterion: every corpus screen survives Weft → Figma → Weft byte-identical,
// with no losses and no diagnostics.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { describe, test } from "vitest";
import { built, corpusMarkup, corpusNames, read } from "./helpers.ts";

for (const name of corpusNames) {
  describe(`corpus ${name}`, () => {
    test("comes back byte-identical", async () => {
      const markup = corpusMarkup(name);
      const { figma, frame } = await built(markup);
      const result = await read(figma, frame);
      assert.deepEqual(result.losses, []);
      assert.deepEqual(result.diagnostics, []);
      assert.equal(serialize(result.document), markup);
    });
  });
}
