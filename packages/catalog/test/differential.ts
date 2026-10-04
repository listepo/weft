// Cases for the TypeScript–Rust differential check of the catalog package. The TypeScript code
// computes the expected results into crates/weft-catalog/tests/fixtures/differential.json; the
// Rust crate must reproduce them. Random cases use a fixed seed, so the fixture only changes with
// the code.
import { readFileSync } from "node:fs";
import fc from "fast-check";
import { coreCatalog, diffCatalogs, loadTokens } from "../src/index.ts";
import { base, rows } from "./diff-cases.ts";

export const fixturePath = new URL(
  "../../../crates/weft-catalog/tests/fixtures/differential.json",
  import.meta.url,
);

const SEED = 20261004;
const RUNS = 300;

/** Only what survives a JSON round trip reaches the Rust side, so expectations use the same. */
const json = (value: unknown): unknown => JSON.parse(JSON.stringify(value) ?? "null");

const tokenTree = (() => {
  const name = fc.constantFrom("a", "b", "c", "$root", "2", "10", "x.y", "{z}", "$description");
  const type = fc.constantFrom("color", "dimension", "number", 7);
  const value = fc.oneof(
    { arbitrary: fc.constantFrom(1, "#fff", { value: 4, unit: "px" }, null, [1, 2]), weight: 1 },
    {
      arbitrary: fc.constantFrom(
        "{a}",
        "{b}",
        "{c}",
        "{a.b}",
        "{c.a}",
        "{a.$root}",
        "{missing}",
        "{b.c.a}",
        "{}",
        "{a}{b}",
      ),
      weight: 2,
    },
  );
  const token = fc.record({ $value: value, $type: type }, { requiredKeys: ["$value"] });
  const { group } = fc.letrec((tie) => ({
    group: fc
      .tuple(
        fc.option(type, { nil: undefined }),
        fc.dictionary(name, fc.oneof({ maxDepth: 3 }, token, tie("group"), fc.constant(5)), {
          maxKeys: 4,
        }),
      )
      .map(([t, children]) => (t === undefined ? children : { $type: t, ...children })),
  }));
  return group;
})();

/** Edits that keep a catalog's shape plausible, so both sides read the same structure. */
const KEYS = [
  "description",
  "role",
  "content",
  "values",
  "min",
  "max",
  "required",
  "default",
  "bindable",
  "writable",
  "tokenType",
  "states",
  "events",
  "allowedChildren",
  "allowedParents",
  "requiresLabel",
  "pattern",
  "integer",
];
const LEAVES = [
  "Changed.",
  "button",
  "text",
  "nodes",
  "mixed",
  "none",
  0,
  3,
  100,
  -1,
  1.5,
  true,
  false,
  null,
  ["primary"],
  ["a", "b"],
  [],
  { x: 1 },
];
const LISTS = new Set(["values", "states", "events", "allowedChildren", "allowedParents"]);

function plausible(catalog: unknown): boolean {
  const isMap = (v: unknown) => typeof v === "object" && v !== null && !Array.isArray(v);
  const components = (catalog as { components?: unknown }).components;
  if (!isMap(components)) return false;
  for (const component of Object.values(components as object)) {
    if (!isMap(component)) return false;
    const c = component as Record<string, unknown>;
    if (!["none", "text", "nodes", "mixed"].includes(c["content"] as string)) return false;
    for (const record of [c["props"], c["slots"]]) {
      if (record === undefined) continue;
      if (!isMap(record)) return false;
      for (const def of Object.values(record as object)) {
        if (!isMap(def)) return false;
        for (const [k, v] of Object.entries(def as object))
          if (LISTS.has(k) && !Array.isArray(v)) return false;
      }
    }
    for (const [k, v] of Object.entries(c)) if (LISTS.has(k) && !Array.isArray(v)) return false;
  }
  return true;
}

/** One concrete edit: `path` leads from the catalog root to an object, then a key is set or deleted. */
type Edit = { path: string[]; key: string; value?: unknown };

function applyEdit(catalog: unknown, e: Edit) {
  let target = catalog as Record<string, unknown>;
  for (const k of e.path) target = target[k] as Record<string, unknown>;
  if ("value" in e) target[e.key] = structuredClone(e.value);
  else delete target[e.key];
}

function catalogEdits(): [string, Edit[]][] {
  const step = fc.record({
    pick: fc.double({ min: 0, max: 1, noNaN: true }),
    action: fc.constantFrom("set", "set", "delete"),
    key: fc.constantFrom(...KEYS),
    value: fc.constantFrom(...LEAVES),
  });
  const out: [string, Edit[]][] = [];
  fc.sample(fc.array(step, { minLength: 1, maxLength: 3 }), {
    seed: SEED + 1,
    numRuns: RUNS,
  }).forEach((steps, i) => {
    const next = structuredClone(coreCatalog) as unknown;
    const edits: Edit[] = [];
    for (const s of steps) {
      const containers: string[][] = [];
      const walk = (v: unknown, path: string[]) => {
        if (typeof v !== "object" || v === null || Array.isArray(v)) return;
        containers.push(path);
        for (const [k, c] of Object.entries(v)) walk(c, [...path, k]);
      };
      walk((next as { components: unknown }).components, ["components"]);
      const path = containers[Math.floor(s.pick * containers.length)] ?? ["components"];
      let target = next as Record<string, unknown>;
      for (const k of path) target = target[k] as Record<string, unknown>;
      const keys = Object.keys(target);
      const e: Edit =
        s.action === "delete"
          ? { path, key: keys[Math.floor(s.pick * keys.length)] ?? s.key }
          : { path, key: s.key, value: s.value };
      applyEdit(next, e);
      edits.push(e);
    }
    if (plausible(next)) out.push([`catalog edit ${i}`, edits]);
  });
  return out;
}

function tokensExpect(input: unknown) {
  const { tokens, problems } = loadTokens(input);
  return { tokens: [...tokens], problems };
}

/** The whole fixture, as the TypeScript code computes it. */
export function differential() {
  const defaults: unknown = JSON.parse(
    readFileSync(new URL("../tokens/default.tokens.json", import.meta.url), "utf8"),
  );
  const tokenInputs: [string, unknown][] = [
    ["default tokens", defaults],
    ...[null, 1, "x", []].map((bad, i): [string, unknown] => [`not an object ${i}`, bad]),
    ...fc
      .sample(tokenTree, { seed: SEED, numRuns: RUNS })
      .map((tree, i): [string, unknown] => [`token tree ${i}`, json(tree)]),
  ];
  const diff = (previous: unknown, next: unknown) => ({
    forward: diffCatalogs(previous as never, next as never),
    reversed: diffCatalogs(next as never, previous as never),
  });
  const core = json(coreCatalog);
  return {
    tokens: tokenInputs.map(([name, input]) => ({ name, input, expect: tokensExpect(input) })),
    // Each case is diffed both ways; `rows` carry whole catalogs, `edits` are applied to the core.
    diffBase: json(base),
    rows: rows.map(([name, next]) => ({
      name,
      next: json(next),
      expect: diff(json(base), json(next)),
    })),
    edits: [["core against itself", [] as Edit[]] as const, ...catalogEdits()].map(
      ([name, edits]) => {
        const next = structuredClone(core);
        for (const e of edits) applyEdit(next, e);
        return { name, edits, expect: diff(core, next) };
      },
    ),
  };
}
