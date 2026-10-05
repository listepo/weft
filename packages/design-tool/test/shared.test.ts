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
  modeToken,
  resolverDocument,
  tokenColor,
  variantAxes,
  type PluginTool,
} from "../src/index.ts";
import { loadResolver } from "./modes.ts";

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

describe("token modes", () => {
  test("a hex string colour is read, with its alpha", () => {
    assert.deepEqual(tokenColor({ type: "color", value: "#ff000080" }), {
      r: 1,
      g: 0,
      b: 0,
      a: 128 / 255,
    });
    assert.equal(tokenColor({ type: "color", value: "red" }), undefined);
  });

  test("a value from a file becomes a token only when it is well formed", () => {
    const rem = { type: "dimension", value: { value: 1, unit: "rem" } };
    assert.equal(modeToken(rem, 16), rem);
    assert.deepEqual(modeToken(rem, 20), {
      type: "dimension",
      value: { value: 20, unit: "px" },
    });
    assert.deepEqual(modeToken({ type: "number", value: 1 }, 2), { type: "number", value: 2 });
    for (const bad of [Number.NaN, "16", null, { r: 2, g: 0, b: 0 }, { r: 0, g: 0 }])
      assert.equal(modeToken(rem, bad), undefined, JSON.stringify(bad));
  });

  test("names that are no DTCG names are left out; the modifier's pointer is escaped", () => {
    const token = { type: "number", value: 1 };
    const document = resolverDocument({
      name: "a/b~c",
      default: "one",
      contexts: new Map([
        [
          "one",
          new Map([
            ["n", token],
            ["$bad", token],
            ["x.{y}", token],
            ["n.deeper", token],
          ]),
        ],
        ["__proto__", new Map([["n", { type: "number", value: 2 }]])],
      ]),
    });
    const [modifier] = loadResolver(document);
    assert.equal(modifier?.name, "a/b~c");
    assert.deepEqual([...(modifier?.contexts.get("one")?.keys() ?? [])], ["n"]);
    assert.deepEqual(modifier?.contexts.get("__proto__")?.get("n"), { type: "number", value: 2 });
    assert.equal(({} as Record<string, unknown>)["n"], undefined);
  });
});
