// The project file of SPEC §10: token layers, a catalog extension, host actions and a data schema
// shared by every screen of a project. Pure: files reach it through an injected reader, so the
// same code serves the CLI (files on disk) and the MCP server (content passed as an argument).
// The project file and every file it names are untrusted: problems become diagnostics, never
// exceptions. The loader is the Rust catalog crate (crates/weft-catalog/src/project.rs) through
// WebAssembly.
import type { Catalog, DataSchema, Diagnostic, Mode } from "@weft/core";
import { toJson, wasm, wellFormed } from "@weft/core/wasm";
import type { Token } from "./tokens.ts";

export type Project = {
  /** The core catalog, or the core catalog with the project's extension merged in. */
  catalog: Catalog;
  /** Present when the project declares `tokens`. */
  tokens?: Map<string, Token> | undefined;
  /** Present when the project declares `actions`. */
  actions?: string[] | undefined;
  /** Present when the project declares `data`. */
  data?: DataSchema | undefined;
  /** The tool sections that passed their checks (SPEC §10.6). */
  settings: Settings;
};

/** A limit of the MCP server that `mcp.limits` may set. */
export type LimitName =
  | "markupChars"
  | "dataChars"
  | "patches"
  | "patchesChars"
  | "projectChars"
  | "diagnostics"
  | "inputElements";

/**
 * The tool sections of `weft.json`, shaped as in the file (`schemas/weft.schema.json` documents
 * them). A key that is absent takes the tool's default; an explicit tool argument overrides both.
 * File names are relative to the project file.
 */
export type Settings = {
  validate?: { mode?: Mode };
  format?: { write?: boolean };
  render?: { data?: string; tokens?: string[]; outDir?: string };
  export?: { react?: { outDir?: string } };
  import?: { html?: { outDir?: string } };
  mcp?: { limits?: Partial<Record<LimitName, number>> };
  plugins?: Record<string, Record<string, unknown>>;
};

export type ProjectOptions = {
  /**
   * Returns the text of a file named by the project, relative to the project directory, or
   * `undefined` when it cannot be read. Without a reader, members hold the files' JSON content
   * instead of their names (SPEC §10.1).
   */
  read?: ((name: string) => string | undefined) | undefined;
  /** Where diagnostic pointers start: `#` for a project file, `#/project` for a tool argument. */
  prefix?: string | undefined;
  mode?: Mode | undefined;
};

export type ProjectResult = { project: Project; diagnostics: Diagnostic[] };

export const PROJECT_FILE = "weft.json";

type Loaded = {
  catalog: Catalog;
  tokens: [string, Token][] | null;
  actions: string[] | null;
  data: string | null;
  settings: Settings;
  diagnostics: Diagnostic[];
};

/** Loads a project from the text of its file. */
export function loadProjectText(text: string, options: ProjectOptions = {}): ProjectResult {
  const source = wellFormed(text);
  // The Rust side cannot call back into JavaScript, so it first names the files it would read
  // (never one outside the project directory), and then loads with their text.
  let files: string | undefined;
  if (options.read !== undefined) {
    const names = JSON.parse(wasm.projectFiles(source)) as string[];
    const found: [string, string][] = [];
    for (const name of names) {
      const content = options.read(name);
      if (content !== undefined) found.push([name, wellFormed(content)]);
    }
    files = JSON.stringify(Object.fromEntries(found));
  }
  const wire = JSON.stringify({ strict: options.mode === "strict", prefix: options.prefix ?? "#" });
  const out = JSON.parse(wasm.loadProject(source, files, wire)) as Loaded;
  const project: Project = { catalog: out.catalog, settings: out.settings };
  if (out.tokens !== null) project.tokens = new Map(out.tokens);
  if (out.actions !== null) project.actions = out.actions;
  if (out.data !== null) project.data = { json: out.data };
  return { project, diagnostics: out.diagnostics };
}

/** Loads a project from its parsed JSON, as a tool argument carries it. */
export function loadProject(json: unknown, options: ProjectOptions = {}): ProjectResult {
  return loadProjectText(toJson(json) ?? "", options);
}
