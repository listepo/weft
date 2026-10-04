import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { describe, test } from "vitest";
import {
  buildRequest,
  exportRequest,
  finishExport,
  handleRequest,
  MAX_MARKUP,
  MAX_TOKENS,
} from "../src/index.ts";
import { FakeFigma, type FakeNode } from "./fake-figma.ts";
import { corpusMarkup, tokens } from "./helpers.ts";

const ui = { catalog: coreCatalog, tokens };
const main = { catalog: coreCatalog };
const login = corpusMarkup("login");

describe("plugin requests", () => {
  test("a pasted screen is built and exports back unchanged", async () => {
    const figma = new FakeFigma();
    const { request } = buildRequest(login, ui);
    assert.ok(request);
    const built = await handleRequest(figma, [], request, main);
    assert.equal(built.type, "built", JSON.stringify(built));
    if (built.type !== "built") return;
    const frame = figma.currentPage.children.find((c) => c.id === built.id) as FakeNode;
    const exported = await handleRequest(figma, [frame], exportRequest(ui), main);
    assert.equal(exported.type, "exported", JSON.stringify(exported));
    if (exported.type !== "exported") return;
    const file = finishExport(exported, ui);
    assert.equal(file.markup, login);
    assert.equal(file.fileName, "login.weft");
    assert.deepEqual(file.losses, []);
    assert.deepEqual(file.diagnostics, []);
  });

  test("markup with errors or too large is not sent", () => {
    const broken = buildRequest("<screen", ui);
    assert.equal(broken.request, undefined);
    assert.ok(broken.diagnostics.length > 0);
    assert.equal(buildRequest(" ".repeat(MAX_MARKUP + 1), ui).request, undefined);
  });

  test("malformed messages are refused before anything is built", async () => {
    const figma = new FakeFigma();
    const document = { weft: "0.1", root: { kind: "screen", id: "s" } };
    for (const message of [
      null,
      "build",
      { type: "build", tokens: [] },
      { type: "build", document: { weft: "0.1" }, tokens: [] },
      { type: "build", document, tokens: [["a", 1]] },
      {
        type: "build",
        document,
        tokens: Array.from({ length: MAX_TOKENS + 1 }, (_, i) => [
          `t${i}`,
          { type: "number", value: 1 },
        ]),
      },
      { type: "export" },
      { type: "drop-tables", tokens: [] },
    ]) {
      const reply = await handleRequest(figma, [], message, main);
      assert.equal(reply.type, "error", JSON.stringify(message)?.slice(0, 60));
    }
    assert.equal(figma.root.children.length, 1);
    assert.equal(figma.currentPage.children.length, 0);
  });

  test("export needs exactly one selected layer", async () => {
    const figma = new FakeFigma();
    const none = await handleRequest(figma, [], exportRequest(ui), main);
    assert.equal(none.type, "error");
    const two = await handleRequest(
      figma,
      [figma.createFrame(), figma.createFrame()],
      exportRequest(ui),
      main,
    );
    assert.equal(two.type, "error");
  });
});
