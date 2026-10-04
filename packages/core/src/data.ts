// The data schema of SPEC §10.5: a JSON Schema (2020-12) subset read into a closed shape, and a
// pass that checks every binding of a document against it. It is a pass of its own rather than a
// `validate` option so that the option types every binding constructs stay as they are; callers
// run it after `parse` or `validate`. The schema is untrusted: nothing here throws.
import { didYouMean, diagnostic, oneOf, type DiagnosticInit } from "./diagnostics.ts";
import type { Catalog, Child, Diagnostic, Document, Node, PropDef, Value } from "./model.ts";
import { BINDING, EACH, LOOP_VARIABLE, MAX_DEPTH, own, UNIVERSAL_PROPS } from "./rules.ts";
import { pathSegment, type SourceMap } from "./source.ts";

export type DataSchema =
  | { kind: "any" }
  | { kind: "never" }
  | {
      kind: "shape";
      /** Absent: any JSON type. */
      types?: readonly string[] | undefined;
      properties?: ReadonlyMap<string, DataSchema> | undefined;
      /** What names outside `properties` lead to; `never` closes the object. */
      additional: DataSchema;
      items?: DataSchema | undefined;
    };

export type DataSchemaProblem = {
  code: "W709" | "W710";
  /** JSON Pointer into the schema, "" for its root. */
  pointer: string;
  message: string;
};

export type DataCheckOptions = {
  catalog: Catalog;
  data: DataSchema;
  /** Positions from `parse`, so that diagnostics carry line and column. */
  source?: SourceMap | undefined;
};

const ANY: DataSchema = { kind: "any" };
const NEVER: DataSchema = { kind: "never" };
const TYPES = ["array", "boolean", "integer", "null", "number", "object", "string"];
const UNSUPPORTED = new Set([
  "$ref",
  "$dynamicRef",
  "allOf",
  "anyOf",
  "oneOf",
  "not",
  "if",
  "then",
  "else",
  "dependentSchemas",
  "prefixItems",
  "contains",
  "patternProperties",
  "propertyNames",
  "unevaluatedItems",
  "unevaluatedProperties",
]);
const INDEX = /^(?:0|[1-9][0-9]*)$/;

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

const escapePointer = (name: string) => name.replaceAll("~", "~0").replaceAll("/", "~1");

const at = (pointer: string) => (pointer === "" ? "the root of the data schema" : pointer);

export function compileDataSchema(json: unknown): {
  schema: DataSchema;
  problems: DataSchemaProblem[];
} {
  const problems: DataSchemaProblem[] = [];
  const malformed = (pointer: string, message: string) =>
    problems.push({ code: "W709", pointer, message });

  const compile = (s: unknown, pointer: string, depth: number): DataSchema => {
    if (depth > MAX_DEPTH) {
      malformed(pointer, `Schemas nest deeper than ${MAX_DEPTH} levels at ${at(pointer)}.`);
      return ANY;
    }
    if (s === true) return ANY;
    if (s === false) return NEVER;
    if (!isObject(s)) {
      malformed(pointer, `The schema at ${at(pointer)} is neither an object nor a boolean.`);
      return ANY;
    }
    let unsupported = false;
    for (const key of Object.keys(s)) {
      if (!UNSUPPORTED.has(key)) continue;
      unsupported = true;
      const where = `${pointer}/${escapePointer(key)}`;
      problems.push({
        code: "W710",
        pointer: where,
        message: `"${key}" at ${where} is not supported; that schema accepts any data.`,
      });
    }
    if (unsupported) return ANY;

    let types: string[] | undefined;
    const type = s["type"];
    if (type !== undefined) {
      const list =
        typeof type === "string"
          ? [type]
          : Array.isArray(type) && type.every((t) => typeof t === "string")
            ? (type as string[])
            : undefined;
      if (list === undefined || list.some((t) => !TYPES.includes(t))) {
        malformed(`${pointer}/type`, `"type" at ${pointer}/type names no JSON Schema type.`);
      } else types = list;
    }
    let properties: Map<string, DataSchema> | undefined;
    const props = s["properties"];
    if (props !== undefined) {
      if (!isObject(props)) {
        malformed(
          `${pointer}/properties`,
          `"properties" at ${pointer}/properties is not an object.`,
        );
      } else {
        properties = new Map();
        for (const [name, sub] of Object.entries(props)) {
          properties.set(
            name,
            compile(sub, `${pointer}/properties/${escapePointer(name)}`, depth + 1),
          );
        }
      }
    }
    const extra = s["additionalProperties"];
    // SPEC §10.5: listed properties are all the properties, unlike JSON Schema's open default.
    const additional =
      extra === undefined
        ? properties === undefined
          ? ANY
          : NEVER
        : compile(extra, `${pointer}/additionalProperties`, depth + 1);
    const items =
      s["items"] === undefined ? undefined : compile(s["items"], `${pointer}/items`, depth + 1);
    return { kind: "shape", types, properties, additional, items };
  };

  return { schema: compile(json, "", 0), problems };
}

const allows = (s: DataSchema, type: string) =>
  s.kind === "any" || (s.kind === "shape" && (s.types === undefined || s.types.includes(type)));

/** One path step; `undefined` when the schema does not declare it. */
function step(s: DataSchema, segment: string): DataSchema | undefined {
  if (s.kind === "any") return ANY;
  if (s.kind === "never") return undefined;
  if (
    INDEX.test(segment) &&
    allows(s, "array") &&
    (s.types !== undefined || s.items !== undefined)
  ) {
    return s.items ?? ANY;
  }
  if (!allows(s, "object")) return undefined;
  const next = s.properties?.get(segment) ?? s.additional;
  return next.kind === "never" ? undefined : next;
}

const names = (s: DataSchema): string[] =>
  s.kind === "shape" && s.properties !== undefined
    ? [...s.properties].filter(([, v]) => v.kind !== "never").map(([k]) => k)
    : [];

const typeText = (s: DataSchema) =>
  s.kind === "shape" && s.types !== undefined ? s.types.join(" or ") : "any type";

/** JSON types a plain binding may deliver to a prop of this definition (SPEC §10.5). */
function accepted(def: PropDef): readonly string[] {
  switch (def.type) {
    case "string":
      return ["string", "number", "integer"];
    case "number":
      return ["number", "integer"];
    case "boolean":
      return ["boolean"];
    case "enum":
    case "token":
      return ["string"];
  }
}

const isBinding = (v: Value): v is { bind: string; not?: true } =>
  typeof v === "object" && "bind" in v && typeof v.bind === "string";

export function checkData(document: Document, options: DataCheckOptions): Diagnostic[] {
  const out: Diagnostic[] = [];
  const { catalog, data, source } = options;
  const report = (code: "W315" | "W316", init: DiagnosticInit) => out.push(diagnostic(code, init));

  const attrAt = (node: Node, path: string, attr: string) => {
    const s = source?.get(node);
    return { path: `${path}/@${attr}`, pos: s?.attrs.get(attr) ?? s?.pos };
  };

  /** The schema at the end of `path`, or `undefined` when it is unknown or was just reported. */
  const resolve = (
    path: string,
    vars: ReadonlyMap<string, DataSchema>,
    where: { path: string; pos?: { line: number; column: number } | undefined },
  ): DataSchema | undefined => {
    // A malformed path or an unknown loop variable is the validator's to report (W214, W305).
    if (!BINDING.test(path)) return undefined;
    let schema: DataSchema;
    let segments: string[];
    let walked: string;
    if (path.startsWith("$.")) {
      schema = data;
      segments = path.slice(2).split(".");
      walked = "$";
    } else {
      const [variable = "", ...rest] = path.slice(1).split(".");
      const found = vars.get(variable);
      if (found === undefined) return undefined;
      schema = found;
      segments = rest;
      walked = `$${variable}`;
    }
    for (const segment of segments) {
      const next = step(schema, segment);
      if (next === undefined) {
        const declared = names(schema);
        report("W315", {
          ...where,
          message: `The data schema does not declare "${segment}" in ${walked}.`,
          expected:
            declared.length > 0
              ? oneOf(declared)
              : allows(schema, "array") && schema.kind === "shape" && schema.types !== undefined
                ? "an array index such as 0"
                : "a path the data schema declares",
          got: path,
          hint: didYouMean(segment, declared),
        });
        return undefined;
      }
      schema = next;
      walked = `${walked}.${segment}`;
    }
    return schema;
  };

  const visitList = (
    list: readonly Child[],
    listPath: string,
    vars: ReadonlyMap<string, DataSchema>,
    depth: number,
  ) => {
    list.forEach((child, index) => {
      if (typeof child !== "string") {
        visit(child, `${listPath}/${pathSegment(child.kind, child.id, index)}`, vars, depth + 1);
      }
    });
  };

  const visit = (
    node: Node,
    path: string,
    vars: ReadonlyMap<string, DataSchema>,
    depth: number,
  ) => {
    if (depth > MAX_DEPTH) return;
    const component = node.kind === EACH ? undefined : own(catalog.components, node.kind);
    let item: DataSchema = ANY;
    for (const [name, value] of Object.entries(node.props ?? {})) {
      if (!isBinding(value)) continue;
      const where = attrAt(node, path, name);
      const found = resolve(value.bind, vars, where);
      if (found === undefined) continue;
      if (node.kind === EACH) {
        if (name !== "in" || value.not === true) continue;
        if (!allows(found, "array")) {
          report("W316", {
            ...where,
            message: `<each> repeats an array; the data at ${value.bind} is ${typeText(found)}.`,
            expected: "array",
            got: typeText(found),
          });
        } else if (found.kind === "shape" && found.items !== undefined) item = found.items;
        continue;
      }
      // A negated binding tests emptiness, which every type has (SPEC §10.5).
      if (value.not === true) continue;
      const def =
        (component === undefined ? undefined : own(component.props, name)) ??
        own(UNIVERSAL_PROPS, name);
      if (def === undefined || found.kind !== "shape" || found.types === undefined) continue;
      const takes = accepted(def);
      if (!found.types.some((t) => takes.includes(t))) {
        report("W316", {
          ...where,
          message: `The data at ${value.bind} is ${typeText(found)}; this attribute takes ${takes.join(" or ")}.`,
          expected: takes.join(" or "),
          got: typeText(found),
        });
      }
    }
    let inner = vars;
    const variable = node.props?.["as"];
    if (node.kind === EACH && typeof variable === "string" && LOOP_VARIABLE.test(variable)) {
      inner = new Map(vars).set(variable, item);
    }
    visitList(node.children ?? [], path, inner, depth);
    for (const [name, list] of Object.entries(node.slots ?? {})) {
      visitList(list, `${path}/slot[${name}]`, inner, depth);
    }
  };

  const root = document.root;
  visit(root, `/${pathSegment(root.kind, root.id)}`, new Map(), 1);
  return out;
}
