// Every corpus screen, built into the fake Penpot file and read back, is byte-identical.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { describe, test } from "vitest";
import type { FakeBoard } from "./fake-penpot.ts";
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

describe("a tilt", () => {
  test("is drawn as the turn in the picture plane, clockwise", async () => {
    const { penpot, root } = await built(corpusMarkup("tilt"));
    assert.equal(penpot.find<FakeBoard>(root, "section#lean").rotation, -6);
    // Turning about x or y has no drawing.
    assert.equal(penpot.find<FakeBoard>(root, "section#front").rotation, 0);
  });

  test("a designer turning a layer is a visual edit, and the document stays as written", async () => {
    const markup = corpusMarkup("tilt");
    const { penpot, root } = await built(markup);
    penpot.find<FakeBoard>(root, "section#front").rotation = 30;
    const result = await read(root);
    assert.deepEqual(
      result.losses.map((l) => l.kind),
      ["tokens"],
    );
    assert.equal(serialize(result.document), markup);
  });
});
