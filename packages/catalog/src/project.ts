// The project file of SPEC §10: token layers, a catalog extension, host actions and a data schema
// shared by every screen of a project. Pure: files reach it through an injected reader, so the
// same code serves the CLI (files on disk) and the MCP server (content passed as an argument).
// The project file and every file it names are untrusted: problems become diagnostics, never
// exceptions.
import {
  ACTION,
  ComponentDefSchema,
  compileDataSchema,
  diagnostic,
  didYouMean,
  NAME,
  oneOf,
  type Catalog,
  type DataSchema,
  type Diagnostic,
  type DiagnosticCode,
  type Mode,
} from "@weft/core";
import { coreCatalog } from "./core.ts";
import { diffCatalogs } from "./diff.ts";
import { loadTokens, type Token, type TokenProblem } from "./tokens.ts";

export type Project = {
  /** The core catalog, or the core catalog with the project's extension merged in. */
  catalog: Catalog;
  /** Present when the project declares `tokens`. */
  tokens?: Map<string, Token> | undefined;
  /** Present when the project declares `actions`. */
  actions?: string[] | undefined;
  /** Present when the project declares `data`. */
  data?: DataSchema | undefined;
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
export const MAX_TOKEN_FILES = 64;

const MEMBERS = ["$schema", "tokens", "catalog", "actions", "data"];
/** Joined rather than replaced by an extension entry (SPEC §10.4). */
const JOINED = ["states", "events", "allowedChildren", "allowedParents"];
/** Merged by name rather than replaced by an extension entry. */
const MERGED = ["props", "slots"];
/** Element names with a fixed meaning in markup, which no component may take. */
const STRUCTURAL = ["each", "slot"];
/** One more than the JSON nesting `parse_json` of the Rust core accepts; see `parseJson`. */
const JSON_DEPTH_LIMIT = 773;

const TOKEN_EXPECTED: Record<TokenProblem["code"], string> = {
  T001: "a JSON object of DTCG groups and tokens",
  T002: 'a name without "{", "}" or "."',
  T003: "a $type on the token or on a group above it",
  T004: "an alias to a token that exists",
  T005: "aliases that end at a value",
  T006: "a JSON object for every token and group, with a string $type",
};

type Json = Record<string, unknown>;
type DiagnosticInit = Parameters<typeof diagnostic>[1];

const isObject = (v: unknown): v is Json =>
  typeof v === "object" && v !== null && !Array.isArray(v);

const escapePointer = (name: string) => name.replaceAll("~", "~0").replaceAll("/", "~1");

const quote = (s: string) => JSON.stringify(s);

/** Deepest bracket nesting outside strings, counted as the Rust core counts it. */
function bracketDepth(text: string): number {
  let depth = 0;
  let max = 0;
  let inString = false;
  let escaped = false;
  for (const c of text) {
    if (inString) {
      if (escaped) escaped = false;
      else if (c === "\\") escaped = true;
      else if (c === '"') inString = false;
      continue;
    }
    if (c === '"') inString = true;
    else if (c === "[" || c === "{") max = Math.max(max, ++depth);
    else if (c === "]" || c === "}") depth = Math.max(0, depth - 1);
  }
  return max;
}

class NotJson extends Error {}

/**
 * `JSON.parse`, narrowed to what the Rust core's `parse_json` accepts too, so both report the
 * same files as not JSON: no nesting past its limit, no number beyond the double range and no
 * lone surrogate.
 */
export function parseJson(text: string): { ok: true; value: unknown } | { ok: false } {
  if (bracketDepth(text) > JSON_DEPTH_LIMIT) return { ok: false };
  try {
    const value: unknown = JSON.parse(text, (key, v: unknown) => {
      if (!key.isWellFormed()) throw new NotJson();
      if (typeof v === "number" && !Number.isFinite(v)) throw new NotJson();
      if (typeof v === "string" && !v.isWellFormed()) throw new NotJson();
      return v;
    });
    return { ok: true, value };
  } catch {
    return { ok: false };
  }
}

/** SPEC §10.2: relative, `/`-separated, and inside the project directory. */
export function isProjectFileName(name: string): boolean {
  return (
    name !== "" &&
    !name.startsWith("/") &&
    !/[\\:\0]/.test(name) &&
    name.split("/").every((segment) => segment !== "" && segment !== "..")
  );
}

/** DTCG trees merge group by group; a token replaces whatever was at its path (SPEC §10.3). */
function mergeTokenTrees(earlier: unknown, later: unknown): unknown {
  if (!isObject(earlier) || !isObject(later) || "$value" in earlier || "$value" in later) {
    return later;
  }
  // Built from entries so that a "__proto__" name stays an ordinary member.
  const merged = new Map(Object.entries(earlier));
  for (const [name, value] of Object.entries(later)) {
    merged.set(name, merged.has(name) ? mergeTokenTrees(merged.get(name), value) : value);
  }
  return Object.fromEntries(merged);
}

function containsNull(v: unknown): boolean {
  if (v === null) return true;
  if (Array.isArray(v)) return v.some(containsNull);
  return isObject(v) && Object.values(v).some(containsNull);
}

/** An extension entry over a core definition, field by field (SPEC §10.4). */
function extendDefinition(core: Json, entry: Json): Json {
  const merged = new Map(Object.entries(core));
  for (const [field, value] of Object.entries(entry)) {
    const before = merged.get(field);
    if (MERGED.includes(field) && isObject(before) && isObject(value)) {
      merged.set(field, Object.fromEntries([...Object.entries(before), ...Object.entries(value)]));
    } else if (JOINED.includes(field) && Array.isArray(before) && Array.isArray(value)) {
      merged.set(field, [...before, ...value.filter((v: unknown) => !before.includes(v))]);
    } else {
      merged.set(field, value);
    }
  }
  return Object.fromEntries(merged);
}

/** Loads a project from the text of its project file. */
export function loadProjectText(text: string, options: ProjectOptions = {}): ProjectResult {
  const parsed = parseJson(text);
  if (parsed.ok) return loadProject(parsed.value, options);
  return {
    project: { catalog: coreCatalog },
    diagnostics: [
      diagnostic("W701", {
        path: options.prefix ?? "#",
        message: "The project file is not JSON.",
        expected: "a JSON object",
      }),
    ],
  };
}

/** Loads a project from its parsed project file, or from project content without a reader. */
export function loadProject(json: unknown, options: ProjectOptions = {}): ProjectResult {
  const { read, mode = "lenient" } = options;
  const prefix = options.prefix ?? "#";
  const diagnostics: Diagnostic[] = [];
  const report = (code: DiagnosticCode, init: DiagnosticInit) =>
    diagnostics.push(diagnostic(code, init, mode));
  const project: Project = { catalog: coreCatalog };

  if (!isObject(json)) {
    report("W701", {
      path: prefix,
      message: "The project file is not a JSON object.",
      expected: "a JSON object",
    });
    return { project, diagnostics };
  }

  for (const name of Object.keys(json)) {
    if (MEMBERS.includes(name)) continue;
    report("W702", {
      path: `${prefix}/${escapePointer(name)}`,
      message: `Unknown member ${quote(name)} in the project file.`,
      expected: oneOf(MEMBERS),
      got: name,
      hint: didYouMean(name, MEMBERS),
    });
  }

  const wrongType = (pointer: string, message: string, expected: string) =>
    report("W701", { path: pointer, message, expected });

  /** The JSON a member names: read from a file, or the member itself without a reader. */
  const content = (
    pointer: string,
    value: unknown,
    what: string,
  ): { value: unknown } | undefined => {
    if (read === undefined) return { value };
    if (typeof value !== "string") {
      wrongType(pointer, `${what} must be a file name.`, "a file name relative to the project");
      return undefined;
    }
    if (!isProjectFileName(value)) {
      report("W703", {
        path: pointer,
        message: `The file name ${quote(value)} is absolute, leaves the project directory or is malformed.`,
        expected: 'a relative path inside the project directory, separated by "/"',
        got: value,
      });
      return undefined;
    }
    const text = read(value);
    const parsed = text === undefined ? undefined : parseJson(text);
    if (parsed?.ok !== true) {
      report("W704", {
        path: pointer,
        message:
          text === undefined
            ? `The file ${quote(value)} cannot be read.`
            : `The file ${quote(value)} is not JSON.`,
        expected: "a readable JSON file",
        got: value,
      });
      return undefined;
    }
    return { value: parsed.value };
  };

  const schemaMember = json["$schema"];
  if (schemaMember !== undefined && typeof schemaMember !== "string") {
    wrongType(`${prefix}/$schema`, '"$schema" must be a string.', "a string");
  }

  if (json["tokens"] !== undefined) {
    project.tokens = loadTokenLayers(json["tokens"]);
  }
  if (json["catalog"] !== undefined) {
    const found = content(`${prefix}/catalog`, json["catalog"], '"catalog"');
    if (found !== undefined) project.catalog = extendCatalog(found.value);
  }
  if (json["actions"] !== undefined) {
    project.actions = loadActions(json["actions"]);
  }
  if (json["data"] !== undefined) {
    const found = content(`${prefix}/data`, json["data"], '"data"');
    if (found !== undefined) {
      const { schema, problems } = compileDataSchema(found.value);
      project.data = schema;
      for (const p of problems) {
        report(p.code, {
          path: `${prefix}/data${p.pointer}`,
          message: p.message,
          expected:
            p.code === "W709"
              ? "a JSON Schema 2020-12 object or boolean (SPEC §10.5)"
              : '"type", "properties", "additionalProperties" or "items"',
        });
      }
    }
  }
  return { project, diagnostics };

  function loadTokenLayers(member: unknown): Map<string, Token> | undefined {
    const pointer = `${prefix}/tokens`;
    const what = read === undefined ? "token trees" : "token file names";
    if (!Array.isArray(member)) {
      wrongType(pointer, `"tokens" must be an array of ${what}.`, `an array of ${what}`);
      return undefined;
    }
    if (member.length > MAX_TOKEN_FILES) {
      wrongType(
        pointer,
        `"tokens" lists ${member.length} files; a project has at most ${MAX_TOKEN_FILES}.`,
        `at most ${MAX_TOKEN_FILES} token files`,
      );
      return undefined;
    }
    let tree: unknown = {};
    member.forEach((entry: unknown, index) => {
      const at = `${pointer}/${index}`;
      const found = content(at, entry, `Entry ${index} of "tokens"`);
      if (found === undefined) return;
      if (!isObject(found.value)) {
        report("W705", {
          path: at,
          message: "A token file must be a JSON object.",
          expected: TOKEN_EXPECTED.T001,
        });
        return;
      }
      tree = mergeTokenTrees(tree, found.value);
    });
    const { tokens, problems } = loadTokens(tree);
    for (const p of problems) {
      report("W705", {
        path: pointer,
        message: p.path === "" ? p.message : `${p.path}: ${p.message}`,
        expected: TOKEN_EXPECTED[p.code],
        got: p.path,
      });
    }
    return tokens;
  }

  function loadActions(member: unknown): string[] | undefined {
    const pointer = `${prefix}/actions`;
    if (!Array.isArray(member)) {
      wrongType(pointer, '"actions" must be an array of action names.', "an array of action names");
      return undefined;
    }
    const actions: string[] = [];
    member.forEach((name: unknown, index) => {
      const at = `${pointer}/${index}`;
      if (typeof name !== "string") {
        wrongType(at, `Entry ${index} of "actions" must be a string.`, "an action name");
      } else if (!ACTION.test(name)) {
        report("W708", {
          path: at,
          message: `${quote(name)} in "actions" is not an action name.`,
          expected: 'a dotted name such as "cart.add"',
          got: name,
        });
      } else actions.push(name);
    });
    return actions;
  }

  function extendCatalog(extension: unknown): Catalog {
    const pointer = `${prefix}/catalog`;
    const valid =
      isObject(extension) &&
      Object.keys(extension).every((k) => ["weft", "name", "version", "components"].includes(k)) &&
      typeof extension["weft"] === "string" &&
      typeof extension["name"] === "string" &&
      typeof extension["version"] === "string" &&
      isObject(extension["components"]);
    if (!valid) {
      report("W706", {
        path: pointer,
        message: "The catalog extension is not a catalog.",
        expected: 'an object with "weft", "name", "version" and "components" (SPEC §5)',
      });
      return coreCatalog;
    }
    const core = coreCatalog.components as Record<string, Json>;
    const components = new Map<string, unknown>(Object.entries(core));
    for (const [kind, entry] of Object.entries(extension["components"] as Json)) {
      const at = `${pointer}/components/${escapePointer(kind)}`;
      const base = Object.hasOwn(core, kind) ? core[kind] : undefined;
      const invalid = (message: string, expected: string) =>
        report("W706", { path: at, message, expected, got: kind });
      if (base === undefined && (!NAME.test(kind) || STRUCTURAL.includes(kind))) {
        invalid(
          `The new component name ${quote(kind)} is not allowed.`,
          'a lowercase name such as "rating" that is not "each" or "slot" and does not start with "x-"',
        );
        continue;
      }
      if (base === undefined && kind.startsWith("x-")) {
        invalid(
          `The new component name ${quote(kind)} starts with "x-", which every catalog treats as opaque.`,
          'a lowercase name such as "rating" that does not start with "x-"',
        );
        continue;
      }
      const merged = isObject(entry) && base !== undefined ? extendDefinition(base, entry) : entry;
      if (containsNull(entry) || !ComponentDefSchema.safeParse(merged).success) {
        invalid(
          `The catalog extension entry ${quote(kind)} is not a valid component definition.`,
          "a component definition of SPEC §5 without null values",
        );
        continue;
      }
      if (base !== undefined) {
        const breaking = diffCatalogs(
          { components: { [kind]: base } } as never,
          { components: { [kind]: merged } } as never,
        ).changes.find((c) => c.level === "major");
        if (breaking !== undefined) {
          report("W707", {
            path: at,
            message: `The extension of ${quote(kind)} would break existing screens, so it keeps its core definition: ${breaking.message}`,
            expected: "a change that only widens the core definition (SPEC §8)",
            got: breaking.path,
          });
          continue;
        }
      }
      components.set(kind, merged);
    }
    return {
      weft: extension["weft"] as string,
      name: extension["name"] as string,
      version: extension["version"] as string,
      components: Object.fromEntries(components) as Catalog["components"],
    };
  }
}
