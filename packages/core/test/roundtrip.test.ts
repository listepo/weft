import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import fc from "fast-check";
import {
  canonicalize,
  parse,
  serialize,
  stringify,
  validate,
  type Child,
  type Document,
  type Node,
  type PropDef,
  type Value,
} from "../src/index.ts";
import { catalog } from "./catalog.ts";

const spec = readFileSync(new URL("../../../SPEC.md", import.meta.url), "utf8");
const examples = [...spec.matchAll(/```xml\n([\s\S]*?)```/g)].map((m) => m[1] ?? "");

/** Snippets in the spec are wrapped in a canonical screen so that they form a document. */
function asDocument(example: string): string {
  if (example.startsWith("<screen") || example.startsWith("<fragment")) return example;
  const body = example
    .trimEnd()
    .split("\n")
    .map((line) => `  ${line}`)
    .join("\n");
  return `<screen id="spec" weft="0.1">\n${body}\n</screen>\n`;
}

test("the SPEC has markup examples", () => {
  assert.ok(examples.length >= 3);
});

for (const [index, example] of examples.entries()) {
  test(`SPEC example ${index + 1} parses cleanly and is canonical`, () => {
    const markup = asDocument(example);
    const result = parse(markup, { catalog });
    // The dialog example shows a slot only; the fixture requires `actions`, which it has.
    assert.deepEqual(result.diagnostics, []);
    assert.ok(result.document);
    assert.equal(serialize(result.document), markup);
    assert.deepEqual(parse(serialize(result.document), { catalog }).document, result.document);
  });
}

test("stringify is byte-stable regardless of key order", () => {
  const a: Document = {
    weft: "0.1",
    root: { children: ["  hi  there "], props: { z: 1, a: -0 }, id: "s", kind: "screen" },
  };
  const b: Document = {
    root: { kind: "screen", id: "s", props: { a: 0, z: 1 }, children: ["hi", "there"] },
    weft: "0.1",
  } as Document;
  assert.equal(stringify(a), stringify(b));
  assert.equal(
    stringify(a),
    `${JSON.stringify({ weft: "0.1", root: { kind: "screen", id: "s", props: { a: 0, z: 1 }, children: ["hi there"] } }, null, 2)}\n`,
  );
});

test("literals are typed by the catalog", () => {
  const { document } = parse(
    '<screen id="s" weft="0.1"><heading id="h" level="2" x-acme-level="2">T</heading><x-acme-box id="b" role="group" hidden="true"/></screen>',
    { catalog },
  );
  const [heading, box] = (document?.root.children ?? []) as Node[];
  assert.deepEqual(heading?.props, { level: 2, "x-acme-level": "2" });
  assert.deepEqual(box?.props, { hidden: "true", role: "group" });
});

test("a literal starting with { round-trips through {{", () => {
  const doc: Document = {
    weft: "0.1",
    root: {
      kind: "screen",
      id: "s",
      children: [{ kind: "text", id: "t", props: { text: "{$.x}" } }],
    },
  };
  const markup = serialize(doc);
  assert.match(markup, /text="\{\{\$\.x\}"/);
  assert.deepEqual(parse(markup, { catalog }).document, canonicalize(doc));
});

// --- Properties -------------------------------------------------------------------------------

const char = fc.constantFrom(
  "a",
  "b",
  "Z",
  "0",
  " ",
  "\n",
  "\t",
  "\r",
  "&",
  "<",
  ">",
  '"',
  "'",
  "{",
  "}",
  "$",
  "!",
  "]",
  "-",
  String.fromCharCode(0xe9),
  String.fromCodePoint(0x1f600),
  String.fromCharCode(0xa0),
);
const text = fc.string({ unit: char, maxLength: 8 });
const segment = fc.constantFrom("a", "items", "0", "12", "_x");
const bindPath = fc.oneof(
  fc.array(segment, { minLength: 1, maxLength: 3 }).map((s) => `$.${s.join(".")}`),
  fc.array(segment, { maxLength: 2 }).map((s) => ["$row", ...s].join(".")),
);
const reference: fc.Arbitrary<Value> = fc.oneof(
  bindPath.map((bind) => ({ bind })),
  bindPath.map((bind) => ({ bind, not: true as const })),
  fc.constantFrom("space.md", "color.accent-2").map((token) => ({ token })),
);

function literal(def: PropDef | undefined, states: readonly string[]): fc.Arbitrary<Value> {
  switch (def?.type) {
    case "number":
      return fc.oneof(fc.integer(), fc.double({ noNaN: true, noDefaultInfinity: true }));
    case "boolean":
      return fc.boolean();
    case "enum":
      return fc.constantFrom(...(def.values ?? states), "other");
    default:
      return text;
  }
}

const kinds = [...Object.keys(catalog.components), "each", "x-acme-chart", "fancy"];
const universal: Record<string, PropDef> = {
  label: { description: "", type: "string" },
  hidden: { description: "", type: "boolean" },
  state: { description: "", type: "enum" },
};

function propsFor(kind: string, isRoot: boolean): fc.Arbitrary<Record<string, Value>> {
  const component = catalog.components[kind];
  const defs: Record<string, PropDef | undefined> =
    component === undefined
      ? { label: undefined, role: undefined, data: undefined }
      : { ...universal, ...component.props };
  if (isRoot) delete defs["weft"];
  if (kind === "each") return fc.record({ in: reference, as: text }, { requiredKeys: [] });
  const shape: Record<string, fc.Arbitrary<Value>> = { "x-acme-flag": text, unknown: text };
  for (const [name, def] of Object.entries(defs)) {
    shape[name] = fc.oneof(literal(def, component?.states ?? []), reference);
  }
  // With no required keys, absent props are omitted rather than set to undefined.
  return fc.record(shape, { requiredKeys: [] }) as fc.Arbitrary<Record<string, Value>>;
}

/** Explicit depth instead of `fc.letrec`, whose depth control does not reach through `chain`. */
function nodeArbitrary(depth: number): fc.Arbitrary<Node> {
  const child: fc.Arbitrary<Child> = depth <= 0 ? text : fc.oneof(text, nodeArbitrary(depth - 1));
  return fc.constantFrom(...kinds).chain((kind) =>
    fc
      .record(
        {
          id: fc.option(text, { nil: undefined }),
          props: propsFor(kind, false),
          on: fc.dictionary(fc.constantFrom("press", "change", "x-acme-hover"), text, {
            maxKeys: 2,
          }),
          children: fc.array(child, { maxLength: 3 }),
          slots: fc.dictionary(
            fc.constantFrom("footer", "actions", "aside"),
            fc.array(child, { maxLength: 2 }),
            { maxKeys: 2 },
          ),
        },
        { requiredKeys: ["children"] },
      )
      // `<each>` cannot hold a `<slot>` in markup, so its JSON form has no slots either.
      .map((parts) => ({ kind, ...parts, ...(kind === "each" ? { slots: {} } : {}) }) as Node),
  );
}

const documentArbitrary: fc.Arbitrary<Document> = fc
  .record({
    props: propsFor("screen", true),
    id: fc.option(text, { nil: undefined }),
    children: fc.array(fc.oneof(text, nodeArbitrary(2)), { maxLength: 4 }),
  })
  .map(({ props, id, children }) => ({
    weft: "0.1",
    root: {
      kind: "screen",
      ...(id === undefined ? {} : { id }),
      props,
      children: children as Child[],
    },
  }));

test("property: parse(serialize(x)) equals canonicalize(x)", () => {
  fc.assert(
    fc.property(documentArbitrary, (doc) => {
      const markup = serialize(doc);
      const result = parse(markup, { catalog });
      assert.ok(result.document, JSON.stringify(result.diagnostics));
      assert.deepStrictEqual(result.document, canonicalize(doc));
      assert.equal(serialize(result.document), markup);
    }),
    { numRuns: 10_000 },
  );
});

const markupChar = fc.constantFrom(
  "<",
  ">",
  "/",
  "=",
  '"',
  "'",
  "&",
  ";",
  "#",
  "!",
  "-",
  "?",
  "[",
  "]",
  "a",
  "x",
  "screen",
  "slot",
  "each",
  " id=",
  " ",
  "\n",
  "{",
  "}",
  "$",
  "&#0;",
  "&amp;",
  "<!--",
  "-->",
  String.fromCharCode(1),
  String.fromCharCode(0xd800),
);

test("property: parse never throws on arbitrary input", () => {
  fc.assert(
    fc.property(
      fc.oneof(fc.string({ unit: markupChar, maxLength: 60 }), fc.string({ unit: "binary" })),
      (input) => {
        const result = parse(input, { catalog });
        for (const d of result.diagnostics) assert.ok(d.code && d.path && d.expected);
      },
    ),
    { numRuns: 5_000 },
  );
});

test("property: parse never throws on mutated valid markup", () => {
  fc.assert(
    fc.property(documentArbitrary, fc.nat(), fc.nat(), (doc, a, b) => {
      const markup = serialize(doc);
      const from = a % markup.length;
      const mutated = markup.slice(0, from) + markup.slice(from + (b % 8));
      parse(mutated, { catalog, mode: "strict" });
    }),
    { numRuns: 2_000 },
  );
});

test("property: validate never throws on arbitrary JSON", () => {
  fc.assert(
    fc.property(fc.jsonValue(), (value) => {
      validate(value, { catalog });
      validate({ weft: "0.1", root: { kind: "screen", id: "s", children: [value] } }, { catalog });
    }),
    { numRuns: 2_000 },
  );
});
