// File plumbing shared by the plugin's scripts. The MCP server is file-free by design, so reading
// and writing files lives here, and every file is bounded by the same limits the server applies.
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { coreCatalog } from "@weft/catalog";
import { hasErrors, parse, type Diagnostic, type Document, type Mode } from "@weft/core";

export type Io = { stdout: (text: string) => void; stderr: (text: string) => void };

export const defaultIo: Io = {
  stdout: (text) => process.stdout.write(text),
  stderr: (text) => process.stderr.write(text),
};

// Exit codes shared by every script: 0 ok, 1 the input has errors, 2 usage or I/O failure.
export const EXIT = { ok: 0, invalid: 1, failure: 2 } as const;

/** Reads a UTF-8 file of at most `maxChars` characters; reports and returns `undefined` otherwise. */
export function readText(file: string, maxChars: number, io: Io): string | undefined {
  try {
    // A UTF-8 character is at most 4 bytes, so a larger file is refused before it is read.
    if (statSync(file).size > maxChars * 4) throw new Error(`longer than ${maxChars} characters`);
    const text = readFileSync(file, "utf8");
    if (text.length > maxChars) throw new Error(`longer than ${maxChars} characters`);
    return text;
  } catch (error) {
    io.stderr(`weft: cannot read ${file}: ${(error as Error).message}\n`);
    return undefined;
  }
}

export function readJson(file: string, maxChars: number, io: Io): { value: unknown } | undefined {
  const text = readText(file, maxChars, io);
  if (text === undefined) return undefined;
  try {
    return { value: JSON.parse(text) };
  } catch (error) {
    io.stderr(`weft: ${file} is not JSON: ${(error as Error).message}\n`);
    return undefined;
  }
}

/** One line per diagnostic: `file:line:col code message — hint`, the format of `weft validate`. */
export function printDiagnostics(file: string, diagnostics: readonly Diagnostic[], io: Io): void {
  for (const d of diagnostics) {
    const where = d.line === undefined ? `${file}:${d.path}` : `${file}:${d.line}:${d.column ?? 1}`;
    io.stderr(`${where} ${d.code} ${d.message}${d.hint === undefined ? "" : ` — ${d.hint}`}\n`);
  }
}

/** Reads and parses a `.weft` screen. Returns the exit code instead when it cannot (already reported). */
export function readScreen(
  file: string,
  limit: number,
  io: Io,
  options: { mode?: Mode; tokens?: ReadonlyMap<string, string> } = {},
): Document | number {
  const markup = readText(file, limit, io);
  if (markup === undefined) return EXIT.failure;
  const { document, diagnostics } = parse(markup, {
    catalog: coreCatalog,
    mode: options.mode ?? "strict",
    ...(options.tokens ? { tokens: options.tokens } : {}),
  });
  printDiagnostics(file, diagnostics, io);
  return document !== undefined && !hasErrors(diagnostics) ? document : EXIT.invalid;
}

/** `dir/name.ext` for `dir/name.other`: generated files land next to their source. */
export function siblingPath(input: string, extension: string): string {
  return join(dirname(input), basename(input, extname(input)) + extension);
}

/** Writes a new file. An existing one is replaced only with `force`, so a command never loses work silently. */
export function writeOutput(file: string, text: string, force: boolean, io: Io): boolean {
  try {
    writeFileSync(file, text, { flag: force ? "w" : "wx" });
    return true;
  } catch (error) {
    const exists = (error as NodeJS.ErrnoException).code === "EEXIST";
    io.stderr(
      exists
        ? `weft: ${file} already exists; pass --force to replace it\n`
        : `weft: cannot write ${file}: ${(error as Error).message}\n`,
    );
    return false;
  }
}
