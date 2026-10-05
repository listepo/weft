// Writes a static HTML page for a canonical JSON document:
//   node src/write-page.ts <document.weft.json> <out.html> [--data <data.json>]
//     [--tokens <tokens.json>] [--appearance light|dark] [--project <weft.json> | --no-project]
// The catalog, tokens and sample data come from the project file above the document (SPEC §10.1),
// if any: `render.tokens`, `render.data` and `render.appearance` (SPEC §10.6), else the project's
// tokens. `--tokens` (a token file or a resolver), `--data` and `--appearance` override them.
import { readFileSync, writeFileSync } from "node:fs";
import { parseArgs } from "node:util";
import {
  coreCatalog,
  isResolver,
  loadTokens,
  withAppearance,
  type ColorScheme,
  type Project,
} from "@weft/catalog";
import {
  chooseProject,
  projectPath,
  readProject,
  readResolver,
  readTokenLayers,
} from "@weft/catalog/node";
import type { Document } from "@weft/core";
import { renderPage } from "./page.ts";

const readJson = (path: string): unknown => JSON.parse(readFileSync(path, "utf8"));

const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    data: { type: "string" },
    tokens: { type: "string" },
    appearance: { type: "string" },
    project: { type: "string" },
    "no-project": { type: "boolean" },
  },
});
const [input, output] = positionals;
if (input === undefined || output === undefined) {
  console.error(
    "usage: write-page <document.weft.json> <out.html> [--data <file>] [--tokens <file>] [--appearance light|dark] [--project <file> | --no-project]",
  );
  process.exit(2);
}
const schemes: readonly (string | undefined)[] = ["light", "dark", undefined];
if (!schemes.includes(values.appearance)) {
  console.error(`--appearance must be "light" or "dark", not "${values.appearance}"`);
  process.exit(2);
}
let appearance = values.appearance as ColorScheme | undefined;
const projectFile = chooseProject(input, {
  project: values.project,
  noProject: values["no-project"],
});
let catalog = coreCatalog;
let layers: Pick<Project, "tokens" | "modifiers" | "appearance"> = {};
let dataFile = values.data;
if (projectFile !== undefined) {
  const report = (d: { path: string; code: string; message: string }) =>
    console.error(`${projectFile}:${d.path} ${d.code} ${d.message}`);
  const { project, diagnostics } = readProject(projectFile);
  diagnostics.forEach(report);
  catalog = project.catalog;
  layers = project;
  const render = project.settings.render;
  if (render?.tokens !== undefined && values.tokens === undefined) {
    const layered = readTokenLayers(projectFile, render.tokens, "#/render");
    layered.diagnostics.forEach(report);
    layers = layered;
  }
  if (render?.data !== undefined) dataFile ??= projectPath(projectFile, render.data);
  appearance ??= render?.appearance;
}
if (values.tokens !== undefined) {
  const json = readJson(values.tokens);
  if (isResolver(json)) {
    const read = readResolver(values.tokens);
    for (const d of read.diagnostics)
      console.error(`${values.tokens}:${d.path} ${d.code} ${d.message}`);
    layers = read;
  } else {
    const loaded = loadTokens(json);
    for (const p of loaded.problems) console.error(`${p.code} ${p.path}: ${p.message}`);
    layers = { tokens: loaded.tokens };
  }
}
const { tokens, scheme } = withAppearance(layers, appearance);
const html = renderPage(readJson(input) as Document, {
  catalog,
  data: dataFile === undefined ? {} : readJson(dataFile),
  ...(tokens ? { tokens } : {}),
  colorScheme: scheme,
});
writeFileSync(output, html);
