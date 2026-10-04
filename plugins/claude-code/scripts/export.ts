#!/usr/bin/env node
// `.weft` screen → React component (`@weft/to-jsx`):
//   node export.ts <screen.weft> [out.jsx] [--name Component] [--force]
import { parseArgs } from "node:util";
import { coreCatalog } from "@weft/catalog";
import { LIMITS } from "@weft/mcp";
import { toJsx } from "@weft/to-jsx";
import { defaultIo, EXIT, readScreen, siblingPath, writeOutput, type Io } from "./lib.ts";

const USAGE = "usage: export <screen.weft> [out.jsx] [--name Component] [--force]\n";

export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: { name: { type: "string" }, force: { type: "boolean" } },
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
  const document = readScreen(input, LIMITS.markupChars, io);
  if (typeof document === "number") return document;

  let jsx: string;
  try {
    jsx = toJsx(document, {
      catalog: coreCatalog,
      ...(parsed.values.name === undefined ? {} : { componentName: parsed.values.name }),
    });
  } catch (error) {
    // `toJsx` throws only for a component name that is not an identifier.
    io.stderr(`weft: ${(error as Error).message}\n`);
    return EXIT.failure;
  }
  const target = output ?? siblingPath(input, ".jsx");
  if (!writeOutput(target, jsx, parsed.values.force === true, io)) return EXIT.failure;
  io.stdout(`Wrote ${target}\n`);
  return EXIT.ok;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
