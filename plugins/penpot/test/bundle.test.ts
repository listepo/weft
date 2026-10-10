// The bundles are what Penpot runs. They are built here and run by `harness.ts` the way Penpot
// splits a plugin: the sandbox in an SES compartment, the UI with the core inlined in the page.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, test } from "vitest";
import { resolverDocument } from "../../../packages/design-tool/src/index.ts";
import { contextMarkup } from "../../../packages/design-tool/test/corpus.ts";
import { exampleModes } from "../../../packages/design-tool/test/modes.ts";
import { buildPlugin } from "../build.ts";

const screen = fileURLToPath(new URL("../../../corpus/login/screen.weft", import.meta.url));
const harness = fileURLToPath(new URL("harness.ts", import.meta.url));
const wasmModule = fileURLToPath(
  new URL("../../../packages/core/wasm/weft_bg.wasm", import.meta.url),
);

let dist = "";
beforeAll(async () => {
  dist = mkdtempSync(join(tmpdir(), "weft-penpot-plugin-"));
  await buildPlugin(dist);
});
afterAll(() => rmSync(dist, { recursive: true, force: true }));

describe("the plugin bundles", () => {
  test("the sandbox gets one classic script without the WebAssembly core", () => {
    const code = readFileSync(join(dist, "plugin.js"), "utf8");
    assert.doesNotMatch(code, /^\s*(import|export)\s/m);
    assert.doesNotMatch(code, /\brequire\(|["']node:|\bawait import\(/);
    assert.doesNotMatch(code, /WebAssembly|weft_bg/);
  });

  test("the UI page carries the core's module once, inline", () => {
    const html = readFileSync(join(dist, "ui.html"), "utf8");
    const base64 = readFileSync(wasmModule).toString("base64");
    assert.equal(html.split(base64).length - 1, 1);
    assert.equal(html.match(/<script\b/g)?.length, 1);
    assert.match(html, /Weft → Penpot/);
  });

  test("the manifest names the code and the permissions the conversion needs", () => {
    const manifest = JSON.parse(readFileSync(join(dist, "manifest.json"), "utf8"));
    assert.equal(manifest.code, "plugin.js");
    assert.equal(manifest.version, 2);
    assert.ok(manifest.description.length <= 200);
    for (const permission of ["content:write", "library:write"])
      assert.ok(manifest.permissions.includes(permission), permission);
  });

  test("build and export work across the split", () => {
    const output = execFileSync(process.execPath, [harness, dist, screen], { encoding: "utf8" });
    const result = JSON.parse(output.trim().split("\n").at(-1) ?? "{}");
    assert.equal(result.lockedDown, true);
    assert.equal(result.sandboxHasWasm, false);
    assert.deepEqual(result.opened, { name: "Weft", url: "ui.html" });
    assert.deepEqual(result.built, { status: "Built.", selected: 1 });
    assert.equal(result.exported.status, "Exported.");
    assert.equal(result.exported.markup, readFileSync(screen, "utf8"));
    assert.deepEqual(result.exported.notes, []);
    assert.equal(result.broken.status, "The markup has errors; nothing was built.");
    assert.ok(result.broken.notes > 0);
  });

  test("context survives the split, and the panel lists the built screen's own as text", () => {
    const file = join(dist, "context.weft");
    writeFileSync(file, contextMarkup);
    const output = execFileSync(process.execPath, [harness, dist, file], { encoding: "utf8" });
    const result = JSON.parse(output.trim().split("\n").at(-1) ?? "{}");
    assert.equal(result.built.status, "Built.");
    assert.deepEqual(result.context, [
      "intent (human Ivan): Returning users sign in with email and password.",
    ]);
    assert.equal(result.exported.status, "Exported.");
    assert.equal(result.exported.markup, contextMarkup);
  });

  test("a pasted resolver makes the file's modes, which export returns as a resolver", () => {
    // The UI reads a resolver without files, so the example's modes are written with inline sets.
    const file = join(dist, "tokens.resolver.json");
    writeFileSync(file, JSON.stringify(resolverDocument(exampleModes().modifier)));
    const output = execFileSync(process.execPath, [harness, dist, screen, file], {
      encoding: "utf8",
    });
    const result = JSON.parse(output.trim().split("\n").at(-1) ?? "{}");
    assert.equal(result.loaded, "Resolver loaded with 1 modifier.");
    assert.equal(result.built.status, "Built.");
    assert.equal(result.exported.status, "Exported.");
    assert.equal(result.exported.downloadable, true);
    const modes = JSON.parse(result.exported.resolver).modifiers.theme;
    assert.equal(modes.default, "light");
    assert.deepEqual(Object.keys(modes.contexts).sort(), ["dark", "light"]);
  });
});
