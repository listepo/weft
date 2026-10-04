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
import { FakePenpot, type FakeShape } from "./fake-penpot.ts";
import { corpusMarkup, tokens } from "./helpers.ts";

const ui = { catalog: coreCatalog, tokens };
const sandbox = { catalog: coreCatalog };
const login = corpusMarkup("login");

describe("plugin requests", () => {
  test("a pasted screen is built and exports back unchanged", async () => {
    const penpot = new FakePenpot();
    const { request } = buildRequest(login, ui);
    assert.ok(request);
    const built = await handleRequest(penpot, [], request, sandbox);
    assert.equal(built.type, "built", JSON.stringify(built));
    if (built.type !== "built") return;
    const root = penpot.currentPage.root.children.find((c) => c.id === built.id) as FakeShape;
    const exported = await handleRequest(penpot, [root], exportRequest(ui), sandbox);
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
    const penpot = new FakePenpot();
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
      const reply = await handleRequest(penpot, [], message, sandbox);
      assert.equal(reply.type, "error", JSON.stringify(message)?.slice(0, 60));
    }
    assert.equal(penpot.pages.length, 1);
    assert.equal(penpot.currentPage.root.children.length, 0);
    assert.equal(penpot.library.local.tokens.sets.length, 0);
  });

  test("export needs exactly one selected shape", async () => {
    const penpot = new FakePenpot();
    const none = await handleRequest(penpot, [], exportRequest(ui), sandbox);
    assert.equal(none.type, "error");
    const two = await handleRequest(
      penpot,
      [penpot.createBoard(), penpot.createBoard()],
      exportRequest(ui),
      sandbox,
    );
    assert.equal(two.type, "error");
  });
});
