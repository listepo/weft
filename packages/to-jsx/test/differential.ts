// Cases the Rust React generator answers to (crates/weft-web/tests/react.rs). The fixture was first
// written by the TypeScript generator and reproduced by the Rust one; the generator now runs in
// Rust, so it pins the output against unnoticed change and ties the package to the crate. Random
// cases use a fixed seed, so the fixture only changes with the code.
import { readdirSync, readFileSync } from "node:fs";
import fc from "fast-check";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { toJsx } from "../src/index.ts";

export const reactFixture = new URL(
  "../../../crates/weft-web/tests/fixtures/react.json",
  import.meta.url,
);

const SEED = 20261005;
const RUNS = 300;
const catalog = coreCatalog;
const repository = new URL("../../../", import.meta.url);

function documents(): Document[] {
  const out: Document[] = [];
  const corpus = new URL("corpus/", repository);
  for (const name of readdirSync(corpus, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .toSorted()) {
    const parsed = parse(readFileSync(new URL(`${name}/screen.weft`, corpus), "utf8"), { catalog });
    if (!parsed.document) throw new Error(`corpus/${name} does not parse`);
    out.push(parsed.document);
  }
  const examples = new URL("packages/catalog/examples/", repository);
  for (const file of readdirSync(examples)
    .filter((f) => f.endsWith(".weft"))
    .toSorted()) {
    const parsed = parse(readFileSync(new URL(file, examples), "utf8"), { catalog });
    if (parsed.document) out.push(parsed.document);
  }
  return out;
}

// The hostile strings of generate.test.ts, minus the lone surrogate: JSON text cannot carry one to
// Rust, which reads it as U+FFFF instead.
const HOSTILE = [
  '"});globalThis.__weftPwned = 1;//',
  "`${globalThis.__weftPwned = 1}`",
  "</main>{globalThis.__weftPwned = 1}<main>",
  "' + (globalThis.__weftPwned = 1) + '",
  "  \\u0022 */ globalThis.__weftPwned = 1 /*",
  "&lt;script&gt;alert(1)&lt;/script&gt;",
  "Crème brûlée 日本  x﻿",
];

const doc = (...children: unknown[]): unknown => ({
  weft: "0.1",
  root: { kind: "screen", id: "root", props: { weft: "0.1" }, children },
});

function hostile(s: string): unknown {
  return doc(
    { kind: "heading", id: s, props: { level: 1, label: s, text: s } },
    { kind: "text", id: "t", children: [s] },
    { kind: "button", id: "b", props: { variant: s }, on: { press: s }, children: [s] },
    { kind: "link", id: "l", props: { href: s, text: s } },
    {
      kind: "field",
      id: "f",
      props: { label: s, value: { bind: s }, placeholder: s, error: s, type: s },
    },
    { kind: "stack", id: "st", props: { gap: { token: s }, align: s, direction: s } },
    {
      kind: "list",
      id: "li",
      children: [
        { kind: "each", id: "e", props: { in: { bind: "$.items" }, as: s }, children: [] },
      ],
      slots: { [s]: [{ kind: "text", id: "x", props: { text: { bind: `$.a.${s}` } } }] },
    },
    { kind: s, id: "k", props: { role: s, label: s }, children: [s] },
  );
}

const each = (src: string, children: unknown[]): unknown => ({
  kind: "each",
  id: `e-${src}`,
  props: { in: { bind: `$.${src}` }, as: "it" },
  children,
});

// Readings the corpus does not reach: bound props of every kind, literal edge values, loops in
// every position and names that need renaming.
const EDGES: unknown[] = [
  doc(
    { kind: "heading", id: "h1", props: { level: { bind: "$.level" } }, children: ["H"] },
    { kind: "heading", id: "h2", props: { level: 9, text: "  a   b " } },
    { kind: "heading", id: "h3", props: { level: " 0x3 " }, children: ["x"] },
    { kind: "heading", id: "h4", props: { level: "4.0" }, children: ["x"] },
    { kind: "grid", id: "g", props: { columns: { bind: "$.level" } } },
    { kind: "grid", id: "g2", props: { columns: 3, gap: { token: "space.md" } } },
    {
      kind: "stack",
      id: "s1",
      props: { direction: { bind: "$.d" }, align: { bind: "$.a" }, wrap: { bind: "$.w" } },
    },
    { kind: "stack", id: "s2", props: { direction: "row", align: "center", wrap: true } },
    { kind: "button", id: "b", props: { text: { bind: "$.title" } }, children: ["Content wins"] },
    { kind: "button", id: "b2", props: { text: { bind: "$.title" } } },
    {
      kind: "button",
      id: "b3",
      props: { disabled: { bind: "$.off", not: true }, variant: "primary", state: "busy" },
      children: ["x"],
    },
    {
      kind: "item-host",
      id: "x",
      props: { role: "group" },
      slots: { b: [{ kind: "text", id: "xb", children: ["b"] }], header: [] },
    },
    { kind: "x-region", id: "xr", props: { role: "region", label: { bind: "$.title" } } },
    { kind: "x-none", id: "xn", props: { role: "none" }, children: ["n"] },
    {
      kind: "list",
      id: "l",
      props: { state: { bind: "$.state" }, ordered: { bind: "$.o" } },
      children: [
        each("items", [{ kind: "item", id: "i", props: { text: { bind: "$it" } } }]),
        { kind: "item", id: "hid", props: { hidden: { bind: "$.hide" } }, children: ["h"] },
      ],
      slots: { empty: [{ kind: "text", id: "none", children: ["Nothing"] }] },
    },
    {
      kind: "list",
      id: "l2",
      props: { ordered: true },
      children: [each("items", [{ kind: "item", id: "i2", on: { press: "go" }, children: ["x"] }])],
      slots: { empty: [{ kind: "text", id: "none2", children: ["Nothing"] }] },
    },
    {
      kind: "table",
      id: "t",
      props: { label: "T" },
      children: [
        { kind: "column", id: "c1", props: { sort: "ascending" }, children: ["A"] },
        each("items", [
          { kind: "column", id: "c", props: { text: { bind: "$it" }, sort: { bind: "$.s" } } },
        ]),
        each("items", [
          {
            kind: "row",
            id: "r",
            props: { selected: { bind: "$it.on" } },
            on: { press: "open" },
            children: [{ kind: "cell", id: "rc" }, "loose", { kind: "text", id: "rt" }],
          },
        ]),
        "stray",
        { kind: "text", id: "tt", children: ["wrapped"] },
      ],
      slots: { empty: [{ kind: "text", id: "none3", children: ["Empty"] }] },
    },
    {
      kind: "radio-group",
      id: "rg",
      props: { label: { bind: "$.title" }, value: { bind: "$.pick" } },
      on: { change: "picked" },
      children: [
        { kind: "radio", id: "r1", props: { value: "a", label: "Name" }, children: ["Shown"] },
        { kind: "radio", id: "r2", props: { value: { bind: "$.v" } } },
      ],
    },
    {
      kind: "radio-group",
      id: "rg2",
      props: { value: "a" },
      children: [{ kind: "radio", id: "r3", props: { value: "a" }, children: ["A"] }],
    },
    { kind: "radio", id: "lone", props: { value: "a" }, children: ["A"] },
    { kind: "field", id: "f", props: { label: "" } },
    {
      kind: "field",
      id: "f2",
      props: {
        label: "N",
        type: { bind: "$.t" },
        value: { bind: "$.n" },
        error: { bind: "$.err" },
        state: { bind: "$.st" },
        required: true,
      },
      on: { change: "typed" },
    },
    { kind: "field", id: "f3", props: { label: "M", type: "multiline", value: "v" } },
    { kind: "field", id: "f4", props: { label: "N", type: "number", value: { bind: "$.n" } } },
    { kind: "checkbox", id: "c", props: { label: "C", checked: { bind: "$.c" } } },
    { kind: "switch", id: "sw", props: { label: "S", checked: true }, on: { change: "flip" } },
    {
      kind: "select",
      id: "sel",
      props: { label: "P", value: { bind: "$.p" } },
      on: { change: "chose" },
      children: [
        { kind: "option", id: "o1", props: { value: "a" }, children: ["A"] },
        each("items", [{ kind: "option", id: "o2", props: { value: { bind: "$it" } } }]),
        { kind: "text", id: "dropped", children: ["x"] },
      ],
    },
    {
      kind: "select",
      id: "sel2",
      props: { label: "Q", value: "a" },
      children: [{ kind: "option", id: "o3", props: { value: "a" }, children: ["A"] }],
    },
    {
      kind: "tabs",
      id: "tb",
      props: { selected: { bind: "$.tab" } },
      on: { change: "tabbed" },
      children: [
        { kind: "tab", id: "ta", props: { label: "A" }, children: ["one"] },
        each("items", [{ kind: "tab", id: "tx", props: { label: { bind: "$it" } } }]),
        { kind: "text", id: "extra", children: ["extra"] },
      ],
    },
    {
      kind: "dialog",
      id: "d",
      props: { open: { bind: "$.open" }, modal: { bind: "$.m" }, label: "D" },
      on: { close: "closed" },
      children: ["d"],
    },
    { kind: "dialog", id: "d2", props: { open: true, modal: false }, children: ["e"] },
    { kind: "dialog", id: "d3", props: { open: false }, children: ["f"] },
    { kind: "alert", id: "al", props: { tone: "danger" }, children: ["Oops"] },
    {
      kind: "menu",
      id: "m",
      props: { label: "M" },
      children: [
        { kind: "menu-item", id: "mi", props: { disabled: true }, on: { press: "a" } },
        { kind: "menu-item", id: "mj", children: ["J"] },
      ],
    },
    { kind: "image", id: "img", props: { src: { bind: "$.src" }, label: "I" } },
    { kind: "link", id: "ln", props: { href: "/x" }, on: { press: "nav" }, children: ["L"] },
    {
      kind: "form",
      id: "fm",
      on: { submit: "send" },
      children: [{ kind: "button", id: "go", props: { submit: "true" }, on: { press: "p" } }],
    },
    {
      kind: "section",
      id: "s",
      props: { label: "S", hidden: { bind: "$.hide" } },
      children: [{ kind: "text", id: "st", props: { tone: { bind: "$.tone" } }, children: ["b"] }],
      slots: { header: [{ kind: "heading", id: "sh", props: { level: 2 }, children: ["Head"] }] },
    },
    {
      kind: "list",
      id: "nest",
      children: [
        {
          kind: "each",
          id: "outer",
          props: { in: { bind: "$.rows" }, as: "data" },
          children: [
            {
              kind: "each",
              id: "inner",
              props: { in: { bind: "$data.cells" }, as: "constructor" },
              children: [
                {
                  kind: "item",
                  id: "cell",
                  props: { text: { bind: "$constructor.name" } },
                  on: { press: "pick" },
                },
              ],
            },
          ],
        },
      ],
    },
    { kind: "text", id: "g1", props: { text: { bind: "$.constructor" } } },
    { kind: "text", id: "g2", props: { text: { bind: "$.s.length" } } },
    { kind: "text", id: "g3", props: { text: { bind: "$.a-b.0" } } },
    { kind: "text", id: "g4", props: { text: 3.5 } },
    { kind: "text", id: "g5", props: { text: true } },
    { kind: "text", id: "g6", children: ["a", " ", "  b\n c "] },
    { kind: "text", id: "g7", props: { text: "  " }, children: [" "] },
  ),
  { weft: "0.1", root: { kind: "screen", id: "r", props: { hidden: { bind: "$.h" } } } },
  { weft: "0.1", root: { kind: "screen", id: "r", props: { hidden: true } } },
];

function deep(levels: number): unknown {
  type El = { kind: string; id: string; props: unknown; children: unknown[] };
  const root: El = { kind: "section", id: "s0", props: { label: "x" }, children: [] };
  let node = root;
  for (let i = 1; i < levels; i++) {
    const next: El = { kind: "section", id: `s${i}`, props: { label: "x" }, children: [] };
    node.children = [next];
    node = next;
  }
  return doc(root);
}

// The arbitrary of generate.test.ts's property, so the crate answers to the same shapes.
const json = fc.letrec((tie) => ({
  value: fc.oneof(
    { depthSize: "small" },
    fc.string(),
    fc.constantFrom(...HOSTILE),
    fc.integer(),
    fc.double(),
    fc.boolean(),
    fc.constant(null),
    fc.record({
      bind: fc.constantFrom("$.a", "$x", "$", "bad", "$.a.0", "$.constructor", "$x.length"),
    }),
    fc.record({ token: fc.string() }),
    fc.array(tie("value"), { maxLength: 3 }),
    fc.record(
      {
        kind: fc.oneof(
          fc.constantFrom(...Object.keys(coreCatalog.components), "each", "x-a-b"),
          fc.string(),
        ),
        id: fc.oneof(fc.string(), fc.constantFrom(...HOSTILE)),
        props: fc.dictionary(
          fc.constantFrom(
            "label",
            "in",
            "as",
            "value",
            "selected",
            "open",
            "level",
            "href",
            "text",
            "hidden",
            "state",
            "columns",
            "checked",
            "ordered",
            "submit",
            "modal",
            "type",
            "error",
          ),
          tie("value"),
          { maxKeys: 4 },
        ),
        on: fc.dictionary(
          fc.constantFrom("press", "change", "submit", "close"),
          fc.oneof(fc.string(), fc.constantFrom(...HOSTILE)),
          { maxKeys: 2 },
        ),
        children: fc.array(tie("value"), { maxLength: 3 }),
        slots: fc.dictionary(
          fc.oneof(fc.string(), fc.constantFrom("empty", "header", "10", "2")),
          fc.array(tie("value"), { maxLength: 2 }),
          { maxKeys: 2 },
        ),
      },
      { requiredKeys: ["kind"] },
    ),
  ),
}));

export type ReactCase = { document: unknown; componentName?: string; output: string };

/** Only what survives a JSON round trip reaches the Rust side, so expectations use the same. */
const wire = (value: unknown): unknown => JSON.parse(JSON.stringify(value) ?? "null");

export function reactCases(): ReactCase[] {
  const inputs: { document: unknown; componentName?: string }[] = [
    ...documents().map((d) => ({ document: d })),
    ...HOSTILE.map((s) => ({ document: hostile(s) })),
    ...EDGES.map((d) => ({ document: d })),
    { document: deep(260) },
    ...[null, 1, "x", [], {}, { root: 5 }, { root: { kind: 1 } }].map((d) => ({ document: d })),
    { document: doc(), componentName: "Settings" },
    ...fc
      .sample(json.value, { seed: SEED, numRuns: RUNS })
      .map((root) => ({ document: { weft: "0.1", root } })),
  ];
  return inputs.map(({ document, componentName }) => {
    const d = wire(document);
    const output =
      componentName === undefined ? toJsx(d, { catalog }) : toJsx(d, { catalog, componentName });
    return componentName === undefined
      ? { document: d, output }
      : { document: d, componentName, output };
  });
}
