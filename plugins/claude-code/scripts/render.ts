#!/usr/bin/env node
// `.weft` screen → static HTML page (`renderPage` of `@weft/render-react`), to preview in a browser:
//   node render.ts <screen.weft> [out.html] [--data data.json] [--tokens tokens.json] [--force]
// Without `--tokens` the catalog's default tokens apply, as in the corpus gallery.
import { basename, extname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { coreCatalog, loadTokens, tokenTypes } from "@weft/catalog";
import { LIMITS } from "@weft/mcp";
import { renderPage } from "@weft/render-react";
import { defaultIo, EXIT, readJson, readScreen, siblingPath, writeOutput, type Io } from "./lib.ts";

const USAGE =
  "usage: render.ts <screen.weft> [out.html] [--data data.json] [--tokens tokens.json] [--force]\n";

const DEFAULT_TOKENS = new URL(
  "../../../packages/catalog/tokens/default.tokens.json",
  import.meta.url,
);

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: { data: { type: "string" }, tokens: { type: "string" }, force: { type: "boolean" } },
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

  const tokensRead = readJson(
    parsed.values.tokens ?? fileURLToPath(DEFAULT_TOKENS),
    LIMITS.markupChars,
    io,
  );
  if (tokensRead === undefined) return EXIT.failure;
  const tokensJson = tokensRead.value;
  const { tokens, problems } = loadTokens(tokensJson);
  for (const p of problems) io.stderr(`${p.code} ${p.path}: ${p.message}\n`);

  let data: unknown = {};
  if (parsed.values.data !== undefined) {
    const read = readJson(parsed.values.data, LIMITS.dataChars, io);
    if (read === undefined) return EXIT.failure;
    data = read.value;
  }

  const document = readScreen(input, LIMITS.markupChars, io, { tokens: tokenTypes(tokens) });
  if (typeof document === "number") return document;

  const html = renderPage(document, {
    catalog: coreCatalog,
    data,
    tokens,
    title: `Weft: ${basename(input, extname(input))}`,
  });
  const target = output ?? siblingPath(input, ".html");
  if (!writeOutput(target, html, parsed.values.force === true, io)) return EXIT.failure;
  // The URL is what the Desktop browser pane opens.
  io.stdout(`Wrote ${target}\n${pathToFileURL(resolve(target)).href}\n`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
