// The tool-independent parts on their own. The round trips through a tool live in the tool
// packages (`@weft/figma`, `@weft/penpot`), which run this code over their fakes.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { describe, test } from "vitest";
import {
  combinations,
  drawing,
  findBelow,
  handleRequest,
  layoutView,
  MAX_TOKENS,
  variantAxes,
  type PluginTool,
} from "../src/index.ts";

const document = { weft: "0.1", root: { kind: "screen", id: "s" } };

describe("plugin messages", () => {
  test("malformed messages reach neither build nor read", async () => {
    let calls = 0;
    const tool: PluginTool<string> = {
      build: async () => {
        calls++;
        return { id: "x" };
      },
      read: async () => {
        calls++;
        return { document: document as never, losses: [], diagnostics: [] };
      },
    };
    for (const message of [
      null,
      "build",
      { type: "build", tokens: [] },
      { type: "build", document: { weft: "0.1" }, display: {}, tokens: [] },
      { type: "build", document, display: {}, tokens: [["a", 1]] },
      { type: "build", document, display: { a: 1 }, tokens: [] },
      {
        type: "build",
        document,
        display: {},
        tokens: Array.from({ length: MAX_TOKENS + 1 }, (_, i) => [
          `t${i}`,
          { type: "number", value: 1 },
        ]),
      },
      { type: "export" },
      { type: "drop-tables", tokens: [] },
    ]) {
      const reply = await handleRequest(tool, ["layer"], message);
      assert.equal(reply.type, "error", JSON.stringify(message)?.slice(0, 60));
    }
    assert.equal(calls, 0);
  });

  test("a build returns the root's id; an export needs exactly one layer", async () => {
    const tool: PluginTool<string> = {
      build: async () => ({ id: "root" }),
      read: async () => ({ document: document as never, losses: [], diagnostics: [] }),
    };
    const message = { type: "build", document, display: {}, tokens: [] };
    assert.deepEqual(await handleRequest(tool, [], message), { type: "built", id: "root" });
    const exportMessage = { type: "export", tokens: [] };
    assert.equal((await handleRequest(tool, [], exportMessage)).type, "error");
    assert.equal((await handleRequest(tool, ["a", "b"], exportMessage)).type, "error");
    assert.equal((await handleRequest(tool, ["a"], exportMessage)).type, "exported");
  });

  test("a failure in the tool is a reply, not a rejection", async () => {
    const tool: PluginTool<string> = {
      build: async () => {
        throw new Error("the file is locked");
      },
      read: async () => {
        throw new Error("unused");
      },
    };
    const reply = await handleRequest(tool, [], {
      type: "build",
      document,
      display: {},
      tokens: [],
    });
    assert.deepEqual(reply, { type: "error", message: "the file is locked" });
  });
});

describe("the layout view", () => {
  test("is written in neutral modes", () => {
    assert.equal(layoutView("stack", { direction: "row" }, undefined, "column").mode, "row");
    assert.equal(layoutView("stack", undefined, undefined, "column").mode, "column");
    assert.equal(layoutView("grid", { columns: 3 }, undefined, "column").columns, 3);
    assert.equal(layoutView("each", undefined, undefined, "row").mode, "row");
    assert.equal(layoutView("each", undefined, undefined, "grid").mode, "column");
    assert.equal(layoutView("stack", { align: "stretch" }, undefined, "column").align, "start");
  });
});

describe("the library drawings", () => {
  test("every catalog kind draws a box for each of its variants", () => {
    for (const [kind, def] of Object.entries(coreCatalog.components)) {
      for (const values of combinations(variantAxes(def))) {
        const d = drawing(kind, def, values);
        assert.equal(d.type, "box", kind);
      }
    }
  });

  test("a filled button takes its color from the variant's token", () => {
    const def = coreCatalog.components["button"];
    assert.ok(def);
    const d = drawing("button", def, { variant: "danger", state: "(unset)" });
    assert.deepEqual(d.fill && "token" in d.fill ? d.fill.token : undefined, "color.action.danger");
    assert.equal(d.stroke, undefined);
  });
});

describe("a search below a layer", () => {
  test("stops at the depth limit", () => {
    type T = { name: string; children?: T[] };
    let deep: T = { name: "target" };
    for (let i = 0; i < 12; i++) deep = { name: `level${i}`, children: [deep] };
    const find = (root: T) =>
      findBelow(
        root,
        (c) => c.name === "target",
        (n) => n.children,
      );
    assert.equal(find(deep), undefined);
    let shallow: T = { name: "target" };
    for (let i = 0; i < 3; i++) shallow = { name: `level${i}`, children: [shallow] };
    assert.equal(find(shallow)?.name, "target");
  });
});
