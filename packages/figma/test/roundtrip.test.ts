// The stage-one done criterion: every corpus screen survives Weft → Figma → Weft byte-identical,
// with no losses and no diagnostics.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { describe, test } from "vitest";
import type { FakeFrame } from "./fake-figma.ts";
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

describe("a tilt", () => {
  test("is drawn as the turn in the picture plane, counterclockwise as Figma counts", async () => {
    const { figma, frame } = await built(corpusMarkup("tilt"));
    assert.equal(figma.find<FakeFrame>(frame, "section#lean").rotation, 6);
    // Turning about x or y has no drawing.
    assert.equal(figma.find<FakeFrame>(frame, "section#front").rotation, 0);
  });

  test("a designer turning a layer is a visual edit, and the document stays as written", async () => {
    const markup = corpusMarkup("tilt");
    const { figma, frame } = await built(markup);
    figma.find<FakeFrame>(frame, "section#front").rotation = 30;
    const result = await read(figma, frame);
    assert.deepEqual(
      result.losses.map((l) => l.kind),
      ["tokens"],
    );
    assert.equal(serialize(result.document), markup);
  });
});
