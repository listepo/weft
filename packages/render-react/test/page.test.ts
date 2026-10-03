// The static page helper and the CLI that writes it.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
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
