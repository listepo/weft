// Context (SPEC §2.3) through Penpot: the same plugin data and rules as Figma, through the shared
// design-tool layer.
import assert from "node:assert/strict";
import { serialize } from "@weft/core";
import { KEY } from "@weft/design-tool";
import { describe, test } from "vitest";
import { dataOf } from "../src/layer.ts";
import { selectionContext } from "../src/index.ts";
import type { FakeShape } from "./fake-penpot.ts";
import { built, contextMarkup, read } from "./helpers.ts";

const ids = (...shapes: FakeShape[]) => {
  const reply = selectionContext(shapes);
  return reply.type === "context" ? reply.entries.map((e) => e.id) : undefined;
};

describe("a screen with context", () => {
  test("comes back byte-identical, with no losses and no diagnostics", async () => {
    const { root } = await built(contextMarkup);
    assert.notEqual(dataOf(root).getPluginData(KEY.context), "");
    const result = await read(root);
    assert.deepEqual(result.losses, []);
    assert.deepEqual(result.diagnostics, []);
    assert.equal(serialize(result.document), contextMarkup);
  });

  test("a removed shape takes its entries with it, as a context loss", async () => {
    const { penpot, root } = await built(contextMarkup);
    penpot.find(root, "button#go").remove();
    const result = await read(root);
    assert.deepEqual(
      result.losses.map((l) => [l.kind, l.path]),
      [["context", "/screen#login/context/entry#submit-disabled"]],
    );
    assert.deepEqual(result.diagnostics, []);
    assert.deepEqual(
      result.document.context?.map((e) => e.id),
      ["why", "reset-where"],
    );
  });
});

describe("the panel", () => {
  test("lists the screen's entries on the root board and an element's on its shape", async () => {
    const { penpot, root } = await built(contextMarkup);
    assert.deepEqual(ids(root), ["why"]);
    assert.deepEqual(ids(penpot.find(root, "link#reset")), ["reset-where"]);
    assert.deepEqual(ids(root, penpot.find(root, "link#reset")), []);
  });
});
