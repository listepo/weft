// Every corpus screen, built into the fake Penpot file and read back, is byte-identical.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { describe, test } from "vitest";
import { built, corpusMarkup, corpusNames, read } from "./helpers.ts";

for (const name of corpusNames) {
  describe(`corpus screen ${name}`, () => {
    test("comes back byte-identical", async () => {
      const markup = corpusMarkup(name);
      const { root } = await built(markup);
      const result = await read(root);
      assert.deepEqual(result.losses, []);
      assert.deepEqual(result.diagnostics, []);
      assert.equal(serialize(result.document), markup);
    });
  });
}
