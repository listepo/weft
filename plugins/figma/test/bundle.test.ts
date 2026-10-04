// The bundle is what Figma runs: one classic script with the `figma` global and nothing else.
// It is built here and run in a bare VM context over the fake file of @weft/figma's tests.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { runInNewContext } from "node:vm";
import type { PluginReply } from "@weft/figma";
import { build } from "vite";
import { afterAll, beforeAll, describe, test } from "vitest";
import { FakeFigma, type FakeNode } from "../../../packages/figma/test/fake-figma.ts";

const root = new URL("..", import.meta.url).pathname;
const login = readFileSync(new URL("../../../corpus/login/screen.weft", import.meta.url), "utf8");

let outDir = "";
let code = "";

beforeAll(async () => {
  outDir = mkdtempSync(join(tmpdir(), "weft-figma-plugin-"));
  await build({ configFile: join(root, "vite.config.ts"), build: { outDir }, logLevel: "silent" });
  code = readFileSync(join(outDir, "code.js"), "utf8");
});

afterAll(() => rmSync(outDir, { recursive: true, force: true }));

/** The fake file plus the parts of `figma` only the plugin's main code touches. */
function sandbox() {
  const figma = new FakeFigma();
  const posted: PluginReply[] = [];
  const page = figma.currentPage as typeof figma.currentPage & { selection: FakeNode[] };
  page.selection = [];
  const ui: {
    onmessage?: (message: unknown) => Promise<void>;
    postMessage: (m: PluginReply) => void;
  } = {
    postMessage: (m) => posted.push(m),
  };
  const shown: string[] = [];
  const api = Object.assign(figma, {
    ui,
    showUI: (html: string) => shown.push(html),
    viewport: { scrollAndZoomIntoView: () => {} },
    getNodeByIdAsync: async (id: string) =>
      figma.currentPage.children.find((c) => c.id === id) ?? null,
  });
  return { figma: api, page, ui, posted, shown };
}

describe("the plugin bundle", () => {
  test("is a single script with no module syntax or Node imports", () => {
    assert.doesNotMatch(code, /^\s*(import|export)\s/m);
    assert.doesNotMatch(code, /\brequire\(/);
    assert.doesNotMatch(code, /["']node:/);
  });

  test("builds a pasted screen and exports the selection back", async () => {
    const s = sandbox();
    runInNewContext(code, { figma: s.figma, __html__: "<p>ui</p>", console });
    assert.deepEqual(s.shown, ["<p>ui</p>"]);
    assert.ok(s.ui.onmessage, "the plugin listens to its UI");

    await s.ui.onmessage({ type: "build", markup: login });
    const built = s.posted.at(-1);
    assert.equal(built?.type, "built", JSON.stringify(built));
    assert.equal(s.page.selection.length, 1, "the new frame is selected");

    await s.ui.onmessage({ type: "export" });
    const exported = s.posted.at(-1);
    assert.equal(exported?.type, "exported", JSON.stringify(exported));
    assert.equal(exported?.type === "exported" ? exported.markup : "", login);
  });

  test("answers a malformed message with an error", async () => {
    const s = sandbox();
    runInNewContext(code, { figma: s.figma, __html__: "", console });
    await s.ui.onmessage?.({ type: "build" });
    assert.equal(s.posted.at(-1)?.type, "error");
  });
});
