// Writes a static HTML page for a canonical JSON document:
//   node src/write-page.ts <document.weft.json> <out.html> [--data <data.json>]
//     [--tokens <tokens.json>] [--project <weft.json> | --no-project]
// The catalog, tokens and sample data come from the project file above the document (SPEC §10.1),
// if any: `render.tokens` and `render.data` (SPEC §10.6), else the project's tokens. `--tokens`
// and `--data` override them.
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { chooseProject, projectPath, readProject, readTokenLayers } from "@weft/catalog/node";
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
const projectFile = chooseProject(input, {
  project: values.project,
  noProject: values["no-project"],
});
let catalog = coreCatalog;
let tokens;
let dataFile = values.data;
if (projectFile !== undefined) {
  const report = (d: { path: string; code: string; message: string }) =>
    console.error(`${projectFile}:${d.path} ${d.code} ${d.message}`);
  const { project, diagnostics } = readProject(projectFile);
  diagnostics.forEach(report);
  catalog = project.catalog;
  tokens = project.tokens;
  const render = project.settings.render;
  if (render?.tokens !== undefined && values.tokens === undefined) {
    const layered = readTokenLayers(projectFile, render.tokens, "#/render");
    layered.diagnostics.forEach(report);
    tokens = layered.tokens;
  }
  if (render?.data !== undefined) dataFile ??= projectPath(projectFile, render.data);
}
if (values.tokens !== undefined) {
  const loaded = loadTokens(readJson(values.tokens));
  for (const p of loaded.problems) console.error(`${p.code} ${p.path}: ${p.message}`);
  tokens = loaded.tokens;
}
const html = renderPage(readJson(input) as Document, {
  catalog,
  data: dataFile === undefined ? {} : readJson(dataFile),
  ...(tokens ? { tokens } : {}),
});
writeFileSync(output, html);
