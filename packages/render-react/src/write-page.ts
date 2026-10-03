// Writes a static HTML page for a canonical JSON document:
//   node src/write-page.ts <document.weft.json> <out.html> [--data <data.json>] [--tokens <tokens.json>]
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { coreCatalog, loadTokens } from "@weft/catalog";
import type { Document } from "@weft/core";
import { renderPage } from "./page.ts";

const readJson = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: { data: { type: "string" }, tokens: { type: "string" } },
});
const [input, output] = positionals;
if (input === undefined || output === undefined) {
  console.error(
    "usage: write-page <document.weft.json> <out.html> [--data <file>] [--tokens <file>]",
  );
  process.exit(2);
}
let tokens;
if (values.tokens !== undefined) {
  const loaded = loadTokens(readJson(values.tokens));
  for (const p of loaded.problems) console.error(`${p.code} ${p.path}: ${p.message}`);
  tokens = loaded.tokens;
}
const html = renderPage(readJson(input) as Document, {
  catalog: coreCatalog,
  data: values.data === undefined ? {} : readJson(values.data),
  ...(tokens ? { tokens } : {}),
});
writeFileSync(output, html);
