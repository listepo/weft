import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { describe, test } from "vitest";
import { handleRequest, MAX_MARKUP } from "../src/index.ts";
import { FakeFigma, type FakeNode } from "./fake-figma.ts";
import { corpusMarkup, tokens } from "./helpers.ts";

const options = { catalog: coreCatalog, tokens };
const login = corpusMarkup("login");

describe("plugin requests", () => {
  test("a pasted screen is built and exports back unchanged", async () => {
    const figma = new FakeFigma();
    const built = await handleRequest(figma, [], { type: "build", markup: login }, options);
    assert.equal(built.type, "built", JSON.stringify(built));
    if (built.type !== "built") return;
    const frame = figma.currentPage.children.find((c) => c.id === built.id) as FakeNode;
    const exported = await handleRequest(figma, [frame], { type: "export" }, options);
    assert.equal(exported.type, "exported", JSON.stringify(exported));
    if (exported.type !== "exported") return;
    assert.equal(exported.markup, login);
    assert.equal(exported.fileName, "login.weft");
    assert.deepEqual(exported.losses, []);
  });

  test("markup with errors builds nothing and says why", async () => {
    const figma = new FakeFigma();
    const reply = await handleRequest(figma, [], { type: "build", markup: "<screen" }, options);
    assert.equal(reply.type, "error");
    assert.ok(reply.type === "error" && (reply.diagnostics?.length ?? 0) > 0);
    assert.equal(figma.currentPage.children.length, 0);
  });

  test("malformed and oversized messages are refused", async () => {
    const figma = new FakeFigma();
    for (const message of [
      null,
      "build",
      { type: "build" },
      { type: "build", markup: 1 },
      { type: "drop-tables" },
      { type: "build", markup: " ".repeat(MAX_MARKUP + 1) },
    ]) {
      const reply = await handleRequest(figma, [], message, options);
      assert.equal(reply.type, "error", JSON.stringify(message)?.slice(0, 40));
    }
    assert.equal(figma.root.children.length, 1);
  });

  test("export needs exactly one selected layer", async () => {
    const figma = new FakeFigma();
    const none = await handleRequest(figma, [], { type: "export" }, options);
    assert.equal(none.type, "error");
    const a = figma.createFrame();
    const b = figma.createFrame();
    const two = await handleRequest(figma, [a, b], { type: "export" }, options);
    assert.equal(two.type, "error");
  });
});
