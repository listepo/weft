// The static page helper and the CLI that writes it.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { renderPage } from "../src/index.ts";
import { doc, el } from "./helpers.ts";

test("renderPage writes a complete document with an escaped title", () => {
  const page = renderPage(doc(el("text", "t", {}, ["Hi"])), {
    catalog: coreCatalog,
    title: "</title><script>",
  });
  assert.match(page, /^<!doctype html>\n<html lang="en"><head><meta charSet="utf-8"\/>/);
  assert.match(page, /<title>&lt;\/title&gt;&lt;script&gt;<\/title>/);
  assert.match(
    page,
    /<body><main data-weft-id="root" aria-label="Test"><div data-weft-id="t">Hi<\/div><\/main><\/body><\/html>\n$/,
  );
});

test("renderPage declares the colour scheme the tokens were picked for", () => {
  const text = doc(el("stack", "s", { gap: { token: "space.sm" } }, []));
  const tokens = new Map([["space.sm", { type: "dimension", value: { value: 6, unit: "px" } }]]);
  const dark = renderPage(text, { catalog: coreCatalog, tokens, colorScheme: "dark" });
  assert.match(dark, /<meta name="color-scheme" content="dark"\/><title>/);
  assert.ok(!renderPage(text, { catalog: coreCatalog }).includes("color-scheme"));
});

test("renderPage puts a given stylesheet in its head and refuses one that closes the element", () => {
  const text = doc(el("text", "t", {}, ["Hi"]));
  const page = renderPage(text, { catalog: coreCatalog, styles: "a > b { color: red; }" });
  assert.match(page, /<title>Weft<\/title><style>a > b \{ color: red; \}<\/style><\/head>/);
  assert.throws(
    () => renderPage(text, { catalog: coreCatalog, styles: "a {}</STYLE><script>" }),
    /close its <style> element/,
  );
});

test("write-page renders a fixture with its data and tokens to a file", () => {
  const out = join(mkdtempSync(join(tmpdir(), "weft-page-")), "page.html");
  const path = (p: string) => fileURLToPath(new URL(p, import.meta.url));
  execFileSync(process.execPath, [
    path("../src/write-page.ts"),
    path("../fixtures/grid.weft.json"),
    out,
    "--data",
    path("../fixtures/list.data.json"),
    "--tokens",
    path("../../catalog/tokens/default.tokens.json"),
  ]);
  const html = readFileSync(out, "utf8");
  assert.match(html, /data-weft-id="cards" style="gap:24px;display:grid/);
});

test("write-page takes data and tokens from render settings; flags override them", () => {
  const dir = mkdtempSync(join(tmpdir(), "weft-page-project-"));
  const path = (p: string) => fileURLToPath(new URL(p, import.meta.url));
  copyFileSync(path("../fixtures/grid.weft.json"), join(dir, "grid.weft.json"));
  copyFileSync(path("../fixtures/list.data.json"), join(dir, "sample.json"));
  copyFileSync(path("../../catalog/tokens/default.tokens.json"), join(dir, "t.tokens.json"));
  writeFileSync(
    join(dir, "weft.json"),
    JSON.stringify({ render: { data: "sample.json", tokens: ["t.tokens.json"] } }),
  );
  const run = (...args: string[]) => {
    const out = join(dir, "page.html");
    execFileSync(process.execPath, [
      path("../src/write-page.ts"),
      join(dir, "grid.weft.json"),
      out,
      ...args,
    ]);
    return readFileSync(out, "utf8");
  };
  const flags = [
    "--data",
    path("../fixtures/list.data.json"),
    "--tokens",
    path("../../catalog/tokens/default.tokens.json"),
  ];
  const fromSettings = run();
  assert.match(fromSettings, /data-weft-id="cards" style="gap:24px;display:grid/);
  assert.equal(fromSettings, run(...flags, "--no-project"));
  // Without the project there are no tokens, so the gap is not resolved.
  assert.doesNotMatch(run("--no-project"), /gap:24px/);
});

test("gallery writes one page per corpus screen and an index", () => {
  const out = mkdtempSync(join(tmpdir(), "weft-gallery-"));
  execFileSync(process.execPath, [
    fileURLToPath(new URL("../src/gallery.ts", import.meta.url)),
    out,
  ]);
  const files = readdirSync(out).sort();
  assert.equal(files.length, 18);
  assert.ok(files.includes("index.html") && files.includes("login.html"));
  assert.match(readFileSync(join(out, "index.html"), "utf8"), /<a href="login.html">login<\/a>/);
  assert.match(readFileSync(join(out, "login.html"), "utf8"), /<title>Weft corpus: login<\/title>/);
});
