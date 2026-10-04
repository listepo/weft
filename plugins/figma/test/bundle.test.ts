// The bundles are what Figma runs. They are built here and run by `harness.ts` the way Figma
// splits a plugin: the main thread without WebAssembly, the UI with the core inlined in the page.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, test } from "vitest";
import { buildPlugin } from "../build.ts";

const screen = fileURLToPath(new URL("../../../corpus/login/screen.weft", import.meta.url));
const harness = fileURLToPath(new URL("harness.ts", import.meta.url));
const wasmModule = fileURLToPath(
  new URL("../../../packages/core/wasm/weft_bg.wasm", import.meta.url),
);

let dist = "";
beforeAll(async () => {
  dist = mkdtempSync(join(tmpdir(), "weft-figma-plugin-"));
  await buildPlugin(dist);
});
afterAll(() => rmSync(dist, { recursive: true, force: true }));

describe("the plugin bundles", () => {
  test("the main thread gets one classic script without the WebAssembly core", () => {
    const code = readFileSync(join(dist, "code.js"), "utf8");
    assert.doesNotMatch(code, /^\s*(import|export)\s/m);
    assert.doesNotMatch(code, /\brequire\(|["']node:|\bawait import\(/);
    assert.doesNotMatch(code, /WebAssembly|weft_bg/);
  });

  test("the UI page carries the core's module once, inline", () => {
    const html = readFileSync(join(dist, "ui.html"), "utf8");
    const base64 = readFileSync(wasmModule).toString("base64");
    assert.equal(html.split(base64).length - 1, 1);
    assert.equal(html.match(/<script\b/g)?.length, 1);
  });

  test("build and export work across the split", () => {
    const output = execFileSync(process.execPath, [harness, dist, screen], { encoding: "utf8" });
    const result = JSON.parse(output.trim().split("\n").at(-1) ?? "{}");
    assert.equal(result.mainHasWasm, false);
    assert.deepEqual(result.built, { status: "Built.", selected: 1 });
    assert.equal(result.exported.status, "Exported.");
    assert.equal(result.exported.markup, readFileSync(screen, "utf8"));
    assert.deepEqual(result.exported.notes, []);
    assert.equal(result.broken.status, "The markup has errors; nothing was built.");
    assert.ok(result.broken.notes > 0);
  });
});
