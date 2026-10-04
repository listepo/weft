// Cases for the TypeScript–Rust differential check. The TypeScript core computes the expected
// results into crates/weft-core/tests/fixtures/differential.json; the Rust core must reproduce
// them byte for byte. Random cases use a fixed seed, so the fixture only changes with the cores.
import { readdirSync, readFileSync } from "node:fs";
import fc from "fast-check";
import {
  applyPatches,
  checkData,
  compileDataSchema,
  parse,
  serialize,
  stringify,
  validate,
  type Catalog,
  type Diagnostic,
  type Document,
  type Mode,
} from "../src/index.ts";
import { cases } from "./cases.ts";
import { dataCases } from "./data-cases.ts";
import { catalog, tokens } from "./catalog.ts";
import { BASE, failures } from "./patch-cases.ts";

export const fixturePath = new URL(
  "../../../crates/weft-core/tests/fixtures/differential.json",
  import.meta.url,
);

const root = new URL("../../../", import.meta.url);
const coreCatalog = JSON.parse(
  readFileSync(new URL("packages/catalog/catalog.json", root), "utf8"),
) as Catalog;
const catalogs: Record<string, Catalog> = { test: catalog, core: coreCatalog };

type Options = {
  catalog: "test" | "core" | null;
  mode: Mode;
  tokens: boolean;
  actions: string[] | null;
};
type MarkupCase = Options & { name: string; markup: string };
type JsonCase = Options & { name: string; json: unknown };
type PatchCase = { name: string; mode: Mode; patches: unknown };
type DataCase = { name: string; catalog: "test" | "core"; schema: unknown; markup: string };

const SEED = 20261004;
const RUNS = 300;
const MODES: Mode[] = ["lenient", "strict"];

function files(dir: string, suffix: string): [string, string][] {
  const out: [string, string][] = [];
  for (const entry of readdirSync(new URL(dir, root), { recursive: true, encoding: "utf8" })) {
    if (entry.endsWith(suffix)) {
      out.push([`${dir}${entry}`, readFileSync(new URL(`${dir}${entry}`, root), "utf8")]);
    }
  }
  return out.toSorted(([a], [b]) => (a < b ? -1 : 1));
}

function sources(): [string, string][] {
  const spec = readFileSync(new URL("SPEC.md", root), "utf8");
  const examples = [...spec.matchAll(/```xml\n([\s\S]*?)```/g)].map((m, i): [string, string] => [
    `SPEC.md#${i}`,
    m[1] ?? "",
  ]);
  return [
    ...examples,
    ...files("corpus/", ".weft"),
    ...files("compat/", ".weft"),
    ...files("packages/catalog/examples/", ".weft"),
  ];
}

/** Pieces of markup that random edits splice in, so mutations hit every tokenizer branch. */
const PIECES = [
  "<",
  ">",
  "/>",
  "</",
  '"',
  "'",
  "=",
  "&",
  "&amp;",
  "&#0;",
  "&#x41;",
  "&bogus;",
  " ",
  "\n",
  "\t",
  "{",
  "{$.a}",
  "{!$.b}",
  "{token.space.md}",
  "{{",
  "<!--",
  "-->",
  "<![CDATA[",
  "<?x?>",
  '<slot name="footer">',
  "</slot>",
  '<text id="t">',
  "</text>",
  '<each in="{$.xs}" as="x">',
  "</each>",
  'id="dup"',
  'on-press="a.b"',
  "😀",
  "\u0001",
  "x-acme-",
  "Upper",
  "_",
];

function mutations(base: [string, string][]): MarkupCase[] {
  const edit = fc.record({
    source: fc.nat({ max: base.length - 1 }),
    at: fc.double({ min: 0, max: 1, noNaN: true }),
    drop: fc.nat({ max: 12 }),
    insert: fc.array(fc.constantFrom(...PIECES), { maxLength: 3 }),
    mode: fc.constantFrom(...MODES),
  });
  return fc.sample(edit, { seed: SEED, numRuns: RUNS }).map((e, i) => {
    const [name, text] = base[e.source] ?? ["", ""];
    const at = Math.floor(e.at * text.length);
    const markup = text.slice(0, at) + e.insert.join("") + text.slice(at + e.drop);
    return {
      name: `mutation ${i} of ${name}`,
      markup,
      catalog: "core",
      mode: e.mode,
      tokens: false,
      actions: null,
    };
  });
}

function jsonMutations(docs: [string, Document][]): JsonCase[] {
  const leaf = fc.oneof(
    fc.string({ maxLength: 4 }),
    fc.constantFrom(0, -0, 1.5, -1, 1e21, 7, true, false, null),
    fc.constantFrom(
      { bind: "$.x" },
      { bind: "$x", not: true },
      { bind: 1 },
      { token: "a" },
      { bind: "a", token: "b" },
      [],
      {},
    ),
  );
  const edit = fc.record({
    doc: fc.nat({ max: docs.length - 1 }),
    pick: fc.double({ min: 0, max: 1, noNaN: true }),
    action: fc.constantFrom("replace", "add", "delete"),
    key: fc.constantFrom(
      "kind",
      "id",
      "props",
      "on",
      "slots",
      "children",
      "extra",
      "text",
      "weft",
      "root",
      "0",
    ),
    value: leaf,
  });
  return fc.sample(edit, { seed: SEED + 1, numRuns: RUNS }).map((e, i) => {
    const [name, doc] = docs[e.doc] ?? ["", undefined];
    const json = JSON.parse(JSON.stringify(doc)) as unknown;
    const containers: Record<string, unknown>[] = [];
    const walk = (v: unknown) => {
      if (typeof v !== "object" || v === null) return;
      containers.push(v as Record<string, unknown>);
      for (const c of Object.values(v)) walk(c);
    };
    walk(json);
    const target = containers[Math.floor(e.pick * containers.length)] ?? {};
    const keys = Object.keys(target);
    const key = Array.isArray(target) ? String(Math.floor(e.pick * (keys.length + 1))) : e.key;
    if (e.action === "delete") delete target[keys[Math.floor(e.pick * keys.length)] ?? key];
    else target[key] = e.value;
    return {
      name: `json mutation ${i} of ${name}`,
      json,
      catalog: "core",
      mode: "lenient",
      tokens: false,
      actions: null,
    };
  });
}

const BASE_IDS = [
  "s",
  "main",
  "email",
  "pw",
  "f",
  "go",
  "reset",
  "l",
  "i1",
  "i2",
  "e",
  "i3",
  "dlg",
  "ok",
  "box",
  "nope",
];

function patchFuzz(): PatchCase[] {
  const id = fc.constantFrom(...BASE_IDS);
  const markup = fc.constantFrom(
    '<text id="n1">New</text>',
    '<button id="n2" on-press="a.b">Go</button><text id="n3">x</text>',
    '<item id="n4">Four</item>',
    '<text id="go">dup</text>',
    "loose text",
    "<text id='bad'>x</text>",
    '<slot name="footer"/>',
    '<x-acme-thing id="n5" role="img"/>',
  );
  const value = fc.oneof(
    {
      arbitrary: fc.constantFrom(
        "Hi",
        "",
        "{x",
        2,
        0.5,
        true,
        false,
        null,
        { bind: "$.v" },
        { bind: "$.v", not: true },
        { token: "space.md" },
      ),
      weight: 9,
    },
    { arbitrary: fc.constant([]), weight: 1 },
  );
  const prop = fc.constantFrom(
    "text",
    "label",
    "variant",
    "disabled",
    "on-press",
    "on-close",
    "id",
    "weft",
    "Bad",
    "level",
    "submit",
    "open",
  );
  const index = fc.oneof(
    { arbitrary: fc.constant(undefined), weight: 4 },
    { arbitrary: fc.constantFrom(0, 1, 2, 5, 99), weight: 4 },
    { arbitrary: fc.constantFrom(-1, 1.5, "0"), weight: 1 },
  );
  const slot = fc.oneof(
    { arbitrary: fc.constant(undefined), weight: 6 },
    { arbitrary: fc.constantFrom("footer", "actions", "header", "Bad"), weight: 2 },
  );
  const patch = fc.oneof(
    fc.record({ op: fc.constant("set"), id, prop, value }),
    fc.record(
      { op: fc.constant("insert"), parent: id, slot, index, markup },
      { requiredKeys: ["op", "parent", "markup"] },
    ),
    fc.record({ op: fc.constant("remove"), id }),
    fc.record(
      { op: fc.constant("move"), id, parent: id, slot, index },
      { requiredKeys: ["op", "id", "parent"] },
    ),
    // Malformed patches are rare, so most lists get past the shape check and exercise applying.
    fc.oneof(
      {
        arbitrary: fc.constantFrom(
          { op: "nope" },
          { op: "set", id: 1 },
          { op: "remove", id: "go", x: 1 },
          null,
          "remove",
        ),
        weight: 1,
      },
      { arbitrary: fc.constant({ op: "remove", id: "i2" }), weight: 3 },
    ),
  );
  return fc
    .sample(fc.array(patch, { minLength: 1, maxLength: 2 }), { seed: SEED + 2, numRuns: RUNS })
    .map((patches, i) => ({
      name: `patch fuzz ${i}`,
      mode: i % 2 === 0 ? "lenient" : "strict",
      patches,
    }));
}

/** A schema that describes `value` exactly, so corpus screens check against their own data. */
function inferSchema(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.length === 0 ? { type: "array" } : { type: "array", items: inferSchema(value[0]) };
  }
  if (value === null) return { type: "null" };
  if (typeof value === "object") {
    const properties = Object.fromEntries(
      Object.entries(value).map(([k, v]) => [k, inferSchema(v)]),
    );
    return { type: "object", properties };
  }
  if (typeof value === "number") return { type: Number.isInteger(value) ? "integer" : "number" };
  return { type: typeof value };
}

/** Random schemas over the names the corpus binds, so paths and types both pass and fail. */
function dataFuzz(screens: [string, string][]): DataCase[] {
  const name = fc.constantFrom("email", "todos", "title", "done", "user", "items", "0", "busy");
  const type = fc.constantFrom("string", "number", "integer", "boolean", "object", "array", "null");
  const { schema } = fc.letrec((tie) => ({
    schema: fc.oneof(
      { maxDepth: 4 },
      fc.constantFrom(true, false, {}, { $ref: "#/x" }, { type: "nope" }, 3),
      fc.record(
        {
          type: fc.oneof(type, fc.array(type, { maxLength: 2 })),
          properties: fc.dictionary(name, tie("schema"), { maxKeys: 3 }),
          additionalProperties: fc.oneof(fc.boolean(), tie("schema")),
          items: tie("schema"),
        },
        { requiredKeys: [] },
      ),
    ),
  }));
  const pick = fc.record({ screen: fc.nat({ max: screens.length - 1 }), schema });
  return fc.sample(pick, { seed: SEED + 3, numRuns: RUNS }).map((p, i) => {
    const [screenName, markup] = screens[p.screen] ?? ["", ""];
    return { name: `data fuzz ${i} of ${screenName}`, catalog: "core", schema: p.schema, markup };
  });
}

function dataCasesAll(): DataCase[] {
  const out: DataCase[] = Object.entries(dataCases).map(([name, c]) => ({
    name,
    catalog: "test",
    schema: c.schema,
    markup: c.markup,
  }));
  const screens: [string, string][] = [];
  for (const entry of readdirSync(new URL("corpus/", root), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const read = (f: string) => readFileSync(new URL(`corpus/${entry.name}/${f}`, root), "utf8");
    const markup = read("screen.weft");
    screens.push([entry.name, markup]);
    out.push({
      name: `corpus ${entry.name} against its data`,
      catalog: "core",
      schema: inferSchema(JSON.parse(read("data.json"))),
      markup,
    });
  }
  screens.sort(([a], [b]) => (a < b ? -1 : 1));
  out.sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
  return [...out, ...dataFuzz(screens)];
}

function dataExpect(c: DataCase) {
  const cat = catalogs[c.catalog] as Catalog;
  const { schema, problems } = compileDataSchema(c.schema);
  const { document, source } = parse(c.markup, { catalog: cat });
  return {
    problems,
    diagnostics:
      document === undefined ? [] : checkData(document, { catalog: cat, data: schema, source }),
  };
}

function markupExpect(c: MarkupCase) {
  const result = parse(c.markup, {
    catalog: c.catalog === null ? undefined : catalogs[c.catalog],
    mode: c.mode,
    tokens: c.tokens ? tokens : undefined,
    actions: c.actions ?? undefined,
  });
  return {
    diagnostics: result.diagnostics,
    json: result.document === undefined ? null : stringify(result.document),
    markup: result.document === undefined ? null : serialize(result.document),
  };
}

function jsonExpect(c: JsonCase): { diagnostics: Diagnostic[] } {
  const cat = c.catalog === null ? undefined : catalogs[c.catalog];
  if (cat === undefined) return { diagnostics: [] };
  return {
    diagnostics: validate(c.json, {
      catalog: cat,
      mode: c.mode,
      tokens: c.tokens ? tokens : undefined,
      actions: c.actions ?? undefined,
    }),
  };
}

/** The whole fixture, as the TypeScript core computes it. */
export function differential() {
  const base = parse(BASE, { catalog, tokens, mode: "strict" }).document;
  const markupCases: MarkupCase[] = [];
  const jsonCases: JsonCase[] = [];
  for (const [code, c] of Object.entries(cases)) {
    const options = {
      catalog: "test" as const,
      tokens: c.options?.tokens !== undefined,
      actions: c.options?.actions ? [...c.options.actions] : null,
    };
    for (const mode of MODES) {
      if (c.markup !== undefined)
        markupCases.push({ name: `${code} ${mode}`, markup: c.markup, mode, ...options });
      else jsonCases.push({ name: `${code} ${mode}`, json: c.json, mode, ...options });
    }
  }
  const docs: [string, Document][] = [];
  for (const [name, markup] of sources()) {
    for (const cat of ["core", null] as const) {
      for (const mode of MODES)
        markupCases.push({
          name: `${name} ${cat ?? "syntax"} ${mode}`,
          markup,
          catalog: cat,
          mode,
          tokens: false,
          actions: null,
        });
    }
    const document = parse(markup, { catalog: coreCatalog }).document;
    if (document !== undefined) {
      docs.push([name, document]);
      jsonCases.push({
        name: `${name} as JSON`,
        json: JSON.parse(stringify(document)),
        catalog: "core",
        mode: "strict",
        tokens: false,
        actions: null,
      });
    }
  }
  markupCases.push(...mutations(sources()));
  jsonCases.push(...jsonMutations(docs));
  const patchCases: PatchCase[] = [
    ...Object.entries(failures).map(([code, [patches]]): PatchCase => ({
      name: code,
      mode: "lenient",
      patches,
    })),
    ...patchFuzz(),
  ];
  return {
    catalogs,
    tokens: Object.fromEntries(tokens),
    patchBase: BASE,
    data: dataCasesAll().map((c) => ({ ...c, expect: dataExpect(c) })),
    markup: markupCases.map((c) => ({ ...c, expect: markupExpect(c) })),
    json: jsonCases.map((c) => ({ ...c, expect: jsonExpect(c) })),
    patches: patchCases.map((c) => {
      const result =
        base === undefined
          ? { diagnostics: [] }
          : applyPatches(base, c.patches, { catalog, tokens, mode: c.mode });
      return {
        ...c,
        expect: {
          diagnostics: result.diagnostics,
          markup:
            "document" in result && result.document !== undefined
              ? serialize(result.document)
              : null,
        },
      };
    }),
  };
}
