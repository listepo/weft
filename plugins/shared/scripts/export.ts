#!/usr/bin/env node
// `.weft` screen → React component (`@weft/to-jsx`):
//   node export.ts <screen.weft> [out.jsx] [--name Component] [--force]
//     [--project weft.json | --no-project]
// The project above the screen (SPEC §10.1) supplies the catalog, tokens, actions and data schema
// the screen is checked against, and `export.react` (SPEC §10.6) where the file goes (`outDir`),
// whether it is TSX (`typescript`), whether it keeps its source comment (`source`) and whether
// that comment carries the screen's context (`context`, `keep` by default).
import { parseArgs } from "node:util";
import { LIMITS } from "@weft/mcp";
import { toJsx } from "@weft/to-jsx";
import {
  contextFor,
  defaultIo,
  EXIT,
  openProject,
  PROJECT_OPTIONS,
  readScreen,
  targetPath,
  writeOutput,
  type Io,
} from "./lib.ts";

const USAGE =
  "usage: export <screen.weft> [out.jsx] [--name Component] [--force] [--project weft.json | --no-project]\n";

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: { name: { type: "string" }, force: { type: "boolean" }, ...PROJECT_OPTIONS },
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
  const workspace = openProject(input, parsed.values, io);
  if (typeof workspace === "number") return workspace;
  const context = contextFor(workspace);
  const document = readScreen(input, LIMITS.markupChars, io, { context });
  if (typeof document === "number") return document;

  const settings = workspace.project?.settings.export?.react;
  const typescript = settings?.typescript === true;
  let jsx: string;
  try {
    // An empty block is left out of the canonical source, so `strip` needs no other change.
    const screen = settings?.context === "strip" ? { ...document, context: [] } : document;
    jsx = toJsx(screen, {
      catalog: context.catalog,
      typescript,
      source: settings?.source === true,
      ...(parsed.values.name === undefined ? {} : { componentName: parsed.values.name }),
    });
  } catch (error) {
    // `toJsx` throws only for a component name that is not an identifier.
    io.stderr(`weft: ${(error as Error).message}\n`);
    return EXIT.failure;
  }
  const target = targetPath(
    input,
    typescript ? ".tsx" : ".jsx",
    output,
    workspace,
    settings?.outDir,
    io,
  );
  if (target === undefined) return EXIT.failure;
  if (!writeOutput(target, jsx, parsed.values.force === true, io)) return EXIT.failure;
  io.stdout(`Wrote ${target}\n`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
