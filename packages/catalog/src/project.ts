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
  /** The core catalog, or the core catalog with the project's catalogs merged in (SPEC §10.4). */
  catalog: Catalog;
  /** The catalogs merged over the core, in the order the project lists them. */
  catalogs: CatalogSource[];
  /** For every kind of `catalog`, the catalog that defined it and those that extended it. */
  kinds: Record<string, KindSource>;
  /** Present when the project declares `tokens`; with a resolver, its default context. */
  tokens?: Map<string, Token> | undefined;
  /** The modifiers of the project's resolver, each resolved for every context (SPEC §10.3). */
  modifiers?: TokenModifier[] | undefined;
  /** The light and dark tokens of the resolver's appearance modifier, when it has one. */
  appearance?: Appearance | undefined;
  /** Present when the project declares `actions`. */
  actions?: string[] | undefined;
  /** Present when the project declares `data`. */
  data?: DataSchema | undefined;
  /** The tool sections that passed their checks (SPEC §10.6). */
  settings: Settings;
};

/** A catalog the project loaded over the core; `source` is the file it was read from. */
export type CatalogSource = { name: string; version: string; prefix?: string; source?: string };

export type KindSource = { catalog: string; extendedBy?: string[] };

/** A resolver modifier: every context's tokens, with the other modifiers at their default. */
export type TokenModifier = {
  name: string;
  /** The context the default input selects. */
  default: string;
  contexts: Map<string, Map<string, Token>>;
};

/** The appearance modifier (SPEC §10.3) and its light and dark contexts. */
export type Appearance = {
  modifier: string;
  light: Map<string, Token>;
  dark: Map<string, Token>;
};

/** The side of the appearance a page is drawn with (`render.appearance`, SPEC §10.6). */
export type ColorScheme = "light" | "dark";

/**
 * The tokens a renderer draws with, and the page's colour scheme: the `asked` side of the
 * appearance (SPEC §10.3), else the side the appearance modifier's default context names. Without
 * an appearance the tokens are as loaded and the page declares no scheme.
 */
export function withAppearance(
  layers: Pick<Project, "tokens" | "modifiers" | "appearance">,
  asked?: ColorScheme,
): { tokens: Map<string, Token> | undefined; scheme: ColorScheme | undefined } {
  const { appearance } = layers;
  if (appearance === undefined) return { tokens: layers.tokens, scheme: undefined };
  const modifier = layers.modifiers?.find((m) => m.name === appearance.modifier);
  // Context names compare case-insensitively, as the loader matches them (SPEC §10.3).
  const fallback = modifier?.default.toLowerCase() === "dark" ? "dark" : "light";
  const scheme = asked ?? fallback;
  return { tokens: appearance[scheme], scheme };
}

/**
 * Whether parsed token JSON is a DTCG resolver document: `resolutionOrder` is an array, which a
 * token tree cannot hold (every member of a group is an object).
 */
export function isResolver(json: unknown): boolean {
  return (
    typeof json === "object" &&
    json !== null &&
    Array.isArray((json as Record<string, unknown>)["resolutionOrder"])
  );
}

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
type ExportContext = "keep" | "strip";
type JsxExportSettings = {
  outDir?: string;
  typescript?: boolean;
  source?: boolean;
  context?: ExportContext;
};
/** An importer that reads context back: a generated file's source comment, a frame's plugin data. */
type SourceImportSettings = { outDir?: string; context?: "keep" | "drop" };

export type Settings = {
  validate?: { mode?: Mode };
  format?: { write?: boolean };
  explain?: { context?: boolean };
  render?: {
    data?: string;
    tokens?: string[] | string;
    outDir?: string;
    appearance?: "light" | "dark";
  };
  export?: {
    html?: { outDir?: string; source?: boolean; context?: ExportContext };
    react?: JsxExportSettings;
    solid?: JsxExportSettings;
    swiftui?: { outDir?: string };
    css?: { outDir?: string };
    a2ui?: { outDir?: string };
    slint?: { outDir?: string; context?: ExportContext };
  };
  import?: {
    html?: SourceImportSettings;
    react?: SourceImportSettings;
    solid?: SourceImportSettings;
    slint?: SourceImportSettings;
    swiftui?: { outDir?: string };
    a2ui?: { outDir?: string };
    figma?: SourceImportSettings;
  };
  mcp?: { limits?: Partial<Record<LimitName, number>>; context?: "read-write" | "read-only" };
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
  catalogs: CatalogSource[];
  kinds: Record<string, KindSource>;
  tokens: [string, Token][] | null;
  modifiers: { name: string; default: string; contexts: [string, [string, Token][]][] }[];
  appearance: { modifier: string; light: string; dark: string } | null;
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
    // A resolver names more files once it is read, so ask until nothing new is missing.
    const found = new Map<string, string>();
    const tried = new Set<string>();
    for (;;) {
      const known = JSON.stringify(Object.fromEntries(found));
      const names = (JSON.parse(wasm.projectFiles(source, known)) as string[]).filter(
        (n) => !tried.has(n),
      );
      if (names.length === 0) break;
      for (const name of names) {
        tried.add(name);
        const content = options.read(name);
        if (content !== undefined) found.set(name, wellFormed(content));
      }
    }
    files = JSON.stringify(Object.fromEntries(found));
  }
  const wire = JSON.stringify({ strict: options.mode === "strict", prefix: options.prefix ?? "#" });
  const out = JSON.parse(wasm.loadProject(source, files, wire)) as Loaded;
  const project: Project = {
    catalog: out.catalog,
    catalogs: out.catalogs,
    kinds: out.kinds,
    settings: out.settings,
  };
  if (out.tokens !== null) project.tokens = new Map(out.tokens);
  if (out.modifiers.length > 0) {
    project.modifiers = out.modifiers.map((m) => ({
      name: m.name,
      default: m.default,
      contexts: new Map(m.contexts.map(([c, t]) => [c, new Map(t)])),
    }));
  }
  const pair = out.appearance;
  const modifier = project.modifiers?.find((m) => m.name === pair?.modifier);
  const light = pair && modifier?.contexts.get(pair.light);
  const dark = pair && modifier?.contexts.get(pair.dark);
  if (pair && light && dark) project.appearance = { modifier: pair.modifier, light, dark };
  if (out.actions !== null) project.actions = out.actions;
  if (out.data !== null) project.data = { json: out.data };
  return { project, diagnostics: out.diagnostics };
}

/** Loads a project from its parsed JSON, as a tool argument carries it. */
export function loadProject(json: unknown, options: ProjectOptions = {}): ProjectResult {
  return loadProjectText(toJson(json) ?? "", options);
}
