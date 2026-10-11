#!/usr/bin/env node
// `weft validate` and `weft fmt`, reduced to what the package's own tests drive. This is NOT
// the `weft` command: the Rust CLI (docs/cli.md) is, and shipping this under the same name in
// bin made installs resolve usage errors for flags the real CLI accepts.
import {
  chmodSync,
  lstatSync,
  readFileSync,
  realpathSync,
  renameSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { parseArgs } from "node:util";
import { hasErrors } from "./diagnostics.ts";
import { CatalogSchema, type Catalog, type Diagnostic } from "./model.ts";
import { parse } from "./parse.ts";
import { serialize } from "./serialize.ts";
import { validate } from "./validate.ts";

export type Io = { stdout: (text: string) => void; stderr: (text: string) => void };

const USAGE = `usage:
  weft validate <file> [--catalog <catalog.json>] [--strict]
  weft fmt <file> [--write]
`;

const defaultIo: Io = {
  stdout: (text) => process.stdout.write(text),
  stderr: (text) => process.stderr.write(text),
};

/** Returns the exit code: 0 ok, 1 diagnostics with errors, 2 usage or I/O failure. */
export function main(argv: readonly string[], io: Io = defaultIo): number {
  let parsed;
  try {
    parsed = parseArgs({
      args: [...argv],
      allowPositionals: true,
      options: {
        catalog: { type: "string" },
        strict: { type: "boolean" },
        write: { type: "boolean" },
      },
    });
  } catch (error) {
    io.stderr(`${(error as Error).message}\n${USAGE}`);
    return 2;
  }
  const [command, file, ...rest] = parsed.positionals;
  if (file === undefined || rest.length > 0 || (command !== "validate" && command !== "fmt")) {
    io.stderr(USAGE);
    return 2;
  }
  const text = read(file, io);
  if (text === undefined) return 2;

  if (command === "fmt") {
    const result = parse(text);
    if (result.document === undefined) {
      print(file, result.diagnostics, io);
      return 1;
    }
    const formatted = serialize(result.document);
    if (parsed.values.write !== true) io.stdout(formatted);
    else if (formatted !== text) writeAtomic(file, formatted);
    return 0;
  }

  let catalog: Catalog | undefined;
  if (parsed.values.catalog !== undefined) {
    catalog = loadCatalog(parsed.values.catalog, io);
    if (catalog === undefined) return 2;
  } else {
    io.stderr("weft: no --catalog given; only the syntax layer was checked\n");
  }
  const mode = parsed.values.strict === true ? "strict" : "lenient";
  let diagnostics: Diagnostic[];
  if (file.endsWith(".json")) {
    let json: unknown;
    try {
      json = JSON.parse(text);
    } catch (error) {
      io.stderr(`${file}: ${(error as Error).message}\n`);
      return 1;
    }
    diagnostics = catalog === undefined ? [] : validate(json, { catalog, mode });
  } else {
    diagnostics = parse(text, { catalog, mode }).diagnostics;
  }
  print(file, diagnostics, io);
  return hasErrors(diagnostics) ? 1 : 0;
}

function read(file: string, io: Io): string | undefined {
  try {
    return readFileSync(file, "utf8");
  } catch (error) {
    io.stderr(`weft: cannot read ${file}: ${(error as Error).message}\n`);
    return undefined;
  }
}

function loadCatalog(file: string, io: Io): Catalog | undefined {
  const text = read(file, io);
  if (text === undefined) return undefined;
  let json: unknown;
  try {
    json = JSON.parse(text);
  } catch (error) {
    io.stderr(`weft: catalog ${file} is not JSON: ${(error as Error).message}\n`);
    return undefined;
  }
  const result = CatalogSchema.safeParse(json);
  if (!result.success) {
    io.stderr(`weft: catalog ${file} is not a Weft catalog:\n${result.error.message}\n`);
    return undefined;
  }
  return result.data;
}

/**
 * Writes `contents` by renaming a sibling temp file into the file `file` names. A symbolic link is
 * followed to its target and left in place; the target's permissions are kept.
 */
function writeAtomic(file: string, contents: string): void {
  const target = lstatSync(file).isSymbolicLink() ? realpathSync(file) : file;
  const mode = statSync(target).mode;
  const tmp = `${target}.${process.pid}.tmp`;
  try {
    writeFileSync(tmp, contents);
    chmodSync(tmp, mode);
    renameSync(tmp, target);
  } catch (error) {
    try {
      unlinkSync(tmp);
    } catch {
      // The temp file may already be gone.
    }
    throw error;
  }
}

/** One line per diagnostic: `file:line:col code message — hint`; JSON input has a path instead of a position. */
function print(file: string, diagnostics: readonly Diagnostic[], io: Io): void {
  for (const d of diagnostics) {
    const where = d.line === undefined ? `${file}:${d.path}` : `${file}:${d.line}:${d.column ?? 1}`;
    io.stdout(`${where} ${d.code} ${d.message}${d.hint === undefined ? "" : ` — ${d.hint}`}\n`);
  }
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
