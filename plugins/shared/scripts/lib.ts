// File plumbing shared by the plugin's scripts. The MCP server is file-free by design, so reading
// and writing files lives here, and every file is bounded by the same limits the server applies.
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { coreCatalog, type Project } from "@weft/catalog";
import { chooseProject, projectPath, readProject } from "@weft/catalog/node";
import { hasErrors, type Diagnostic, type Document, type Mode } from "@weft/core";
import { contextOf, LIMITS, readMarkup, type Context } from "@weft/mcp";

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

/** `--project` and `--no-project`, the same in every script (SPEC §10.1). */
export const PROJECT_OPTIONS = {
  project: { type: "string" },
  "no-project": { type: "boolean" },
} as const;

/** The project a script works in, and the file it came from; both absent without one. */
export type Workspace = { file?: string | undefined; project?: Project | undefined };

/**
 * The project for `input`: the one `--project` names, none with `--no-project`, else the
 * `weft.json` above the input. Its diagnostics are printed; a project with errors, or one that
 * cannot be read, stops the script, because its output would follow the wrong catalog.
 */
export function openProject(
  input: string,
  values: { project?: string | undefined; "no-project"?: boolean | undefined },
  io: Io,
): Workspace | number {
  const file = chooseProject(input, { project: values.project, noProject: values["no-project"] });
  if (file === undefined) return {};
  let loaded;
  try {
    loaded = readProject(file, { maxChars: LIMITS.projectChars });
  } catch (error) {
    io.stderr(`weft: cannot read ${file}: ${(error as Error).message}\n`);
    return EXIT.failure;
  }
  printDiagnostics(file, loaded.diagnostics, io);
  return hasErrors(loaded.diagnostics) ? EXIT.invalid : { file, project: loaded.project };
}

/** What a screen is checked against: the project's catalog and resources, or the core catalog. */
export function contextFor(workspace: Workspace): Context {
  return workspace.project === undefined ? { catalog: coreCatalog } : contextOf(workspace.project);
}

/** Reads and parses a `.weft` screen. Returns the exit code instead when it cannot (already reported). */
export function readScreen(
  file: string,
  limit: number,
  io: Io,
  options: { mode?: Mode; context?: Context } = {},
): Document | number {
  const markup = readText(file, limit, io);
  if (markup === undefined) return EXIT.failure;
  const context = options.context ?? { catalog: coreCatalog };
  const { document, diagnostics, ok } = readMarkup(markup, context, options.mode ?? "strict");
  printDiagnostics(file, diagnostics, io);
  return ok && document !== undefined ? document : EXIT.invalid;
}

/** A Markdown table, one row per loss, so the model can relay it to the user as it is. */
export function lossTable(losses: readonly { kind: string; path: string; note: string }[]): string {
  if (losses.length === 0) return "No losses.\n";
  const cell = (value: string) => value.replaceAll("|", "\\|").replaceAll("\n", " ");
  const rows = losses.map((l) => `| ${l.kind} | ${cell(l.path)} | ${cell(l.note)} |`);
  return ["| Kind | Path | Note |", "| --- | --- | --- |", ...rows].join("\n") + "\n";
}

/** `dir/name.ext` for `dir/name.other`: generated files land next to their source. */
export function siblingPath(input: string, extension: string): string {
  return join(dirname(input), basename(input, extname(input)) + extension);
}

/**
 * Where a generated file goes: the path given on the command line, else the project's `outDir`
 * setting for this command (created when missing), else next to its source. `undefined` when the
 * directory cannot be made (already reported).
 */
export function targetPath(
  input: string,
  extension: string,
  explicit: string | undefined,
  workspace: Workspace,
  outDir: string | undefined,
  io: Io,
): string | undefined {
  if (explicit !== undefined) return explicit;
  const sibling = siblingPath(input, extension);
  if (outDir === undefined || workspace.file === undefined) return sibling;
  // The loader has already refused an outDir that leaves the project directory.
  const dir = projectPath(workspace.file, outDir);
  try {
    mkdirSync(dir, { recursive: true });
  } catch (error) {
    io.stderr(`weft: cannot create ${dir}: ${(error as Error).message}\n`);
    return undefined;
  }
  return join(dir, basename(sibling));
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
