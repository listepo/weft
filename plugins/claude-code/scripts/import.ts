#!/usr/bin/env node
// HTML page → `.weft` screen, with the loss table the import leaves behind (SPEC §9):
//   node import.ts <page.html> [out.weft] [--force]
import { parseArgs } from "node:util";
import { coreCatalog } from "@weft/catalog";
import { hasErrors, serialize, validate } from "@weft/core";
import { fromDom, MAX_HTML_LENGTH, type Loss } from "@weft/from-aria";
import {
  defaultIo,
  EXIT,
  printDiagnostics,
  readText,
  siblingPath,
  writeOutput,
  type Io,
} from "./lib.ts";

const USAGE = "usage: import.ts <page.html> [out.weft] [--force]\n";

/** A Markdown table, one row per loss, so the model can relay it to the user as it is. */
export function lossTable(losses: readonly Loss[]): string {
  if (losses.length === 0) return "No losses.\n";
  const cell = (value: string) => value.replaceAll("|", "\\|").replaceAll("\n", " ");
  const rows = losses.map((l) => `| ${l.kind} | ${cell(l.path)} | ${cell(l.note)} |`);
  return ["| Kind | Path | Note |", "| --- | --- | --- |", ...rows].join("\n") + "\n";
}

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: { force: { type: "boolean" } },
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
  const html = readText(input, MAX_HTML_LENGTH, io);
  if (html === undefined) return EXIT.failure;

  const { document, losses, diagnostics } = fromDom(html, { catalog: coreCatalog });
  const problems = [
    ...diagnostics,
    ...validate(document, { catalog: coreCatalog, mode: "lenient" }),
  ];
  printDiagnostics(input, problems, io);
  if (hasErrors(problems)) return EXIT.invalid;

  const target = output ?? siblingPath(input, ".weft");
  if (!writeOutput(target, serialize(document), parsed.values.force === true, io))
    return EXIT.failure;
  io.stdout(`Wrote ${target}\n\nImport losses:\n\n${lossTable(losses)}`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
