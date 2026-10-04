// Writes a static HTML page for a canonical JSON document:
//   node src/write-page.ts <document.weft.json> <out.html> [--data <data.json>]
//     [--tokens <tokens.json>] [--project <weft.json> | --no-project]
// The catalog and tokens come from the project file above the document (SPEC §10.1), if any;
// `--tokens` replaces the project's tokens.
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { findProject, readProject } from "@weft/catalog/node";
import type { Document } from "@weft/core";
import { renderPage } from "./page.ts";

const readJson = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    data: { type: "string" },
    tokens: { type: "string" },
    project: { type: "string" },
    "no-project": { type: "boolean" },
  },
});
const [input, output] = positionals;
if (input === undefined || output === undefined) {
  console.error(
    "usage: write-page <document.weft.json> <out.html> [--data <file>] [--tokens <file>] [--project <file> | --no-project]",
  );
  process.exit(2);
}
const projectFile =
  values["no-project"] === true ? undefined : (values.project ?? findProject(input));
let catalog = coreCatalog;
let tokens;
if (projectFile !== undefined) {
  const { project, diagnostics } = readProject(projectFile);
  for (const d of diagnostics) console.error(`${projectFile}:${d.path} ${d.code} ${d.message}`);
  catalog = project.catalog;
  tokens = project.tokens;
}
if (values.tokens !== undefined) {
  const loaded = loadTokens(readJson(values.tokens));
  for (const p of loaded.problems) console.error(`${p.code} ${p.path}: ${p.message}`);
  tokens = loaded.tokens;
}
const html = renderPage(readJson(input) as Document, {
  catalog,
  data: values.data === undefined ? {} : readJson(values.data),
  ...(tokens ? { tokens } : {}),
});
writeFileSync(output, html);
