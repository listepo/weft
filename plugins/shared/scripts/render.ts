#!/usr/bin/env node
// `.weft` screen → static HTML page (`renderPage` of `@weft/render-react`), to preview in a browser:
//   node render.ts <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]
//     [--appearance light|dark] [--force] [--project weft.json | --no-project]
// The project above the screen (SPEC §10.1) supplies the catalog, and its settings (SPEC §10.6)
// the defaults: `render.tokens` (else the project's tokens, else the catalog's default tokens, as
// in the corpus gallery), `render.data`, `render.appearance` and `render.outDir`. Arguments
// override them; `--tokens` takes a token file or a DTCG resolver.
import { basename, extname, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import {
  isResolver,
  loadTokens,
  tokenTypes,
  withAppearance,
  type ColorScheme,
  type Project,
} from "@weft/catalog";
import { hasErrors } from "@weft/core";
import { projectPath, readResolver, readTokenLayers } from "@weft/catalog/node";
import { LIMITS } from "@weft/mcp";
import { renderPage } from "@weft/render-react";
// A JSON module, not a path under the repository, so the bundle needs no file beside it.
import defaultTokens from "../../../packages/catalog/tokens/default.tokens.json" with { type: "json" };
import {
  contextFor,
  defaultIo,
  EXIT,
  openProject,
  printDiagnostics,
  PROJECT_OPTIONS,
  readJson,
  readScreen,
  targetPath,
  writeOutput,
  type Io,
  type Workspace,
} from "./lib.ts";

const USAGE =
  "usage: render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json] [--appearance light|dark] [--force] [--project weft.json | --no-project]\n";
const SCHEMES: readonly string[] = ["light", "dark"];

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: {
        data: { type: "string" },
        tokens: { type: "string" },
        appearance: { type: "string" },
        force: { type: "boolean" },
        ...PROJECT_OPTIONS,
      },
    });
  } catch (error) {
    io.stderr(`${(error as Error).message}\n${USAGE}`);
    return EXIT.failure;
  }
  const [input, output, ...rest] = parsed.positionals;
  if (input === undefined || rest.length > 0) {
    io.stderr(USAGE);
    return EXIT.failure;
  }
  const asked = parsed.values.appearance;
  if (asked !== undefined && !SCHEMES.includes(asked)) {
    io.stderr(`--appearance must be "light" or "dark", not "${asked}"\n${USAGE}`);
    return EXIT.failure;
  }

  const workspace = openProject(input, parsed.values, io);
  if (typeof workspace === "number") return workspace;
  const settings = workspace.project?.settings.render;

  const layers = renderTokens(parsed.values.tokens, workspace, io);
  if (typeof layers === "number") return layers;
  const { tokens, scheme } = withAppearance(
    layers,
    (asked as ColorScheme | undefined) ?? settings?.appearance,
  );

  const dataFile =
    parsed.values.data ??
    (settings?.data !== undefined && workspace.file !== undefined
      ? projectPath(workspace.file, settings.data)
      : undefined);
  let data: unknown = {};
  if (dataFile !== undefined) {
    const read = readJson(dataFile, LIMITS.dataChars, io);
    if (read === undefined) return EXIT.failure;
    data = read.value;
  }

  // Token references are checked against the tokens the page is rendered with.
  const context = { ...contextFor(workspace), tokens: tokenTypes(tokens ?? new Map()) };
  const document = readScreen(input, LIMITS.markupChars, io, { context });
  if (typeof document === "number") return document;

  const html = renderPage(document, {
    catalog: context.catalog,
    data,
    ...(tokens ? { tokens } : {}),
    title: `Weft: ${basename(input, extname(input))}`,
    colorScheme: scheme,
  });
  const target = targetPath(input, ".html", output, workspace, settings?.outDir, io);
  if (target === undefined) return EXIT.failure;
  if (!writeOutput(target, html, parsed.values.force === true, io)) return EXIT.failure;
  // The URL is what the Desktop browser pane opens.
  io.stdout(`Wrote ${target}\n${pathToFileURL(resolve(target)).href}\n`);
  return EXIT.ok;
}

type Layers = Pick<Project, "tokens" | "modifiers" | "appearance">;

/**
 * The tokens to render with: `--tokens`, else `render.tokens`, else the project's own, else the
 * catalog's defaults, with the resolver's contexts when they come from one. The exit code instead
 * when they cannot be read (already reported); a problem in `render.tokens` or a resolver stops
 * the script as one in the project's own tokens does.
 */
function renderTokens(explicit: string | undefined, workspace: Workspace, io: Io): Layers | number {
  let json: unknown = defaultTokens;
  if (explicit !== undefined) {
    const read = readJson(explicit, LIMITS.markupChars, io);
    if (read === undefined) return EXIT.failure;
    json = read.value;
    if (isResolver(json)) {
      const layers = readResolver(explicit, { maxChars: LIMITS.projectChars });
      printDiagnostics(explicit, layers.diagnostics, io);
      return hasErrors(layers.diagnostics) ? EXIT.invalid : layers;
    }
  } else if (workspace.file !== undefined && workspace.project !== undefined) {
    const names = workspace.project.settings.render?.tokens;
    if (names !== undefined) {
      const layered = readTokenLayers(workspace.file, names, "#/render", {
        maxChars: LIMITS.projectChars,
      });
      printDiagnostics(workspace.file, layered.diagnostics, io);
      if (hasErrors(layered.diagnostics)) return EXIT.invalid;
      if (layered.tokens !== undefined) return layered;
    } else if (workspace.project.tokens !== undefined) {
      return workspace.project;
    }
  }
  const { tokens, problems } = loadTokens(json);
  for (const p of problems) io.stderr(`${p.code} ${p.path}: ${p.message}\n`);
  return { tokens };
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
