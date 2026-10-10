// Context (SPEC §2.3) through Figma: kept in the root frame's plugin data, never drawn, read back
// exactly, and listed read-only for the selected layer.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { serialize } from "@weft/core";
import { KEY } from "@weft/design-tool";
import { describe, test } from "vitest";
import { dataOf } from "../src/data.ts";
import { readScreen, selectionContext } from "../src/index.ts";
import { FakeText, type FakeFrame, type FakeNode } from "./fake-figma.ts";
import { built, contextMarkup, read, tokens } from "./helpers.ts";

const ids = (...nodes: FakeNode[]) => {
  const reply = selectionContext(nodes);
  assert.equal(reply.type, "context");
  return reply.type === "context" ? reply.entries.map((e) => e.id) : [];
};

describe("a screen with context", () => {
  test("comes back byte-identical, with no losses and no diagnostics", async () => {
    const { figma, frame } = await built(contextMarkup);
    const result = await read(figma, frame);
    assert.deepEqual(result.losses, []);
    assert.deepEqual(result.diagnostics, []);
    assert.equal(serialize(result.document), contextMarkup);
  });

  test("keeps the entries on the root frame only, and draws none of them", async () => {
    const { figma, frame } = await built(contextMarkup);
    const stored = JSON.parse(dataOf(frame).getPluginData(KEY.context)) as { id: string }[];
    assert.deepEqual(
      stored.map((e) => e.id),
      ["why", "submit-disabled", "reset-where"],
    );
    for (const node of figma.everyDataHolderFor()) {
      if (node !== frame) assert.equal(dataOf(node).getPluginData(KEY.context), "");
      if (node instanceof FakeText)
        assert.doesNotMatch(node.characters, /Returning|Disabled|reset/);
    }
  });

  test("a removed layer takes its entries with it, as a context loss", async () => {
    const { figma, frame } = await built(contextMarkup);
    figma.find(frame, "link#reset").remove();
    const result = await read(figma, frame);
    assert.deepEqual(result.losses, [
      {
        kind: "context",
        path: "/screen#login/context/entry#reset-where",
        note: "the layer this entry is about was removed",
      },
    ]);
    assert.deepEqual(result.diagnostics, []);
    assert.deepEqual(
      result.document.context?.map((e) => e.id),
      ["why", "submit-disabled"],
    );
  });

  test("context that is not a list of entries is ignored", async () => {
    const { figma, frame } = await built(contextMarkup);
    for (const crafted of ['{"id":"x"}', "[1]", "not json", '[{"id":"x","__proto__":{}}]']) {
      dataOf(frame).setPluginData(KEY.context, crafted);
      const result = await read(figma, frame);
      assert.equal(result.document.context, undefined, crafted);
      assert.deepEqual(result.diagnostics, []);
    }
  });

  test("drop reads none of it", async () => {
    const { figma, frame } = await built(contextMarkup);
    const result = await readScreen(figma, frame, {
      catalog: coreCatalog,
      tokens,
      context: "drop",
    });
    assert.equal(result.document.context, undefined);
    assert.deepEqual(result.diagnostics, []);
  });
});

describe("the panel", () => {
  test("lists the screen's entries on the root, and an element's on its layer or below it", async () => {
    const { figma, frame } = await built(contextMarkup);
    assert.deepEqual(ids(frame), ["why"]);
    assert.deepEqual(ids(figma.find(frame, "button#go")), ["submit-disabled"]);
    assert.deepEqual(ids(figma.find(frame, "link#reset")), ["reset-where"]);
    // A layer that is not an element shows the element it belongs to.
    const text = figma.find<FakeFrame>(frame, "button#go").children[0] as FakeNode;
    assert.deepEqual(ids(text), ["submit-disabled"]);
    assert.deepEqual(ids(figma.find(frame, "form#f1")), []);
  });

  test("lists nothing for two layers, or for a layer outside a built screen", async () => {
    const { figma, frame } = await built(contextMarkup);
    assert.deepEqual(ids(frame, figma.find(frame, "button#go")), []);
    assert.deepEqual(ids(figma.createFrame()), []);
  });
});
