import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import type { Document } from "@weft/core";
import { render, safeUrl } from "@weft/render-react";
import fc from "fast-check";
import { parseSync } from "oxc-parser";
import { renderToStaticMarkup } from "react-dom/server";
import { RUNTIME, toJsx } from "../src/index.ts";
import { load, loadSolid, markup, normalizeMarkup, type Props } from "./compile.ts";

type El = {
  kind: string;
  id?: string;
  props?: Record<string, unknown>;
  on?: Record<string, string>;
  children?: unknown[];
  slots?: Record<string, unknown[]>;
};
const doc = (...children: unknown[]): Document =>
  ({
    weft: "0.1",
    root: { kind: "screen", id: "root", props: { weft: "0.1" }, children },
  }) as Document;
const jsx = (d: unknown, componentName?: string) =>
  toJsx(
    d,
    componentName === undefined
      ? { catalog: coreCatalog }
      : { catalog: coreCatalog, componentName },
  );

// Top-level statements a generated module may hold: the React import, the helpers and the
// component. Anything else would be code a document smuggled in.
function assertShape(source: string): void {
  const { program, errors } = parseSync("screen.jsx", source);
  assert.deepEqual(errors, [], source);
  const helpers = new Set(Object.keys(RUNTIME));
  for (const s of program.body) {
    if (s.type === "ImportDeclaration") assert.equal(s.source.value, "react");
    else if (s.type === "FunctionDeclaration")
      assert.ok(s.id && helpers.has(s.id.name), String(s.id?.name));
    else assert.equal(s.type, "ExportDefaultDeclaration");
  }
}

const HOSTILE = [
  '"});globalThis.__weftPwned = 1;//',
  "`${globalThis.__weftPwned = 1}`",
  "</main>{globalThis.__weftPwned = 1}<main>",
  "' + (globalThis.__weftPwned = 1) + '",
  "  \\u0022 */ globalThis.__weftPwned = 1 /*",
  "&lt;script&gt;alert(1)&lt;/script&gt;",
  "\ud800 lone surrogate",
];

test("hostile strings in text, attributes, ids, actions and paths stay data", async () => {
  for (const s of HOSTILE) {
    const d = doc(
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
    const source = jsx(d);
    assertShape(source);
    const component = await load(source);
    const html = markup(component, { data: { items: [1] } });
    assert.equal((globalThis as Record<string, unknown>)["__weftPwned"], undefined);
    assert.ok(!html.includes("<script"), html);
  }
});

test("document text is rendered as text, never as markup", async () => {
  const component = await load(
    jsx(doc({ kind: "text", id: "t", children: ["<b>bold</b> & {x}"] })),
  );
  assert.match(markup(component, {}), /&lt;b&gt;bold&lt;\/b&gt; &amp; \{x\}/);
});

test("unsafe URLs are dropped like the renderer drops them", async () => {
  const d = doc(
    { kind: "link", id: "a", props: { href: "javascript:alert(1)" }, children: ["a"] },
    { kind: "link", id: "b", props: { href: { bind: "$.url" } }, children: ["b"] },
    { kind: "image", id: "i", props: { src: " java\nscript:x", label: "i" } },
  );
  const html = markup(await load(jsx(d)), { data: { url: "data:text/html,x" } });
  assert.doesNotMatch(html, /href=|src=/);
});

test("loop variables that shadow the component's names are renamed", async () => {
  const source = jsx(
    doc({
      kind: "list",
      id: "l",
      children: [
        {
          kind: "each",
          id: "e",
          props: { in: { bind: "$.rows" }, as: "data" },
          children: [{ kind: "item", id: "i", props: { text: { bind: "$data.name" } } }],
        },
      ],
    }),
  );
  assert.match(source, /\(data_, _i0\)/);
  const html = markup(await load(source), { data: { rows: [{ name: "x" }, { name: "y" }] } });
  assert.match(html, /data-weft-id="i\[0\]">x<\/li><li data-weft-id="i\[1\]">y</);
});

test("bindings through inherited names read own properties only", async () => {
  const source = jsx(
    doc(
      { kind: "text", id: "t", props: { text: { bind: "$.constructor" } } },
      { kind: "text", id: "u", props: { text: { bind: "$.s.length" } } },
    ),
  );
  assert.match(source, /_get\(data, \["constructor"\]\)/);
  const html = markup(await load(source), { data: { s: "abc" } });
  assert.match(html, /<div data-weft-id="t"><\/div><div data-weft-id="u"><\/div>/);
});

test("events call the named action with id, action and item", async () => {
  const calls: unknown[] = [];
  const source = jsx(
    doc({
      kind: "list",
      id: "l",
      children: [
        {
          kind: "each",
          id: "e",
          props: { in: { bind: "$.rows" }, as: "row" },
          children: [{ kind: "item", id: "i", on: { press: "row.open" }, children: ["x"] }],
        },
      ],
    }),
  );
  assert.match(
    source,
    /_act\(actions, \{ id: "i\[" \+ _i0 \+ "\]", action: "row\.open", item: "\$\.rows\." \+ _i0 \}\)/,
  );
  const component = await load(source);
  // Server rendering attaches no handlers, so the element tree is walked to reach them.
  const tree = (component as (props: unknown) => unknown)({
    data: { rows: [0, 1] },
    actions: { "row.open": (e: unknown) => calls.push(e) },
  }) as unknown;
  const find = (n: unknown): ((...a: unknown[]) => void) | undefined => {
    if (Array.isArray(n)) return n.map(find).find(Boolean);
    if (typeof n !== "object" || n === null) return undefined;
    const props = (n as { props?: Record<string, unknown> }).props ?? {};
    if (props["data-weft-id"] === "i[1]" && typeof props["onClick"] === "function")
      return props["onClick"] as () => void;
    return find(props["children"]);
  };
  find(tree)?.();
  assert.deepEqual(calls, [{ id: "i[1]", action: "row.open", item: "$.rows.1" }]);
});

test("writable props become controlled inputs that report their data path", async () => {
  const source = jsx(
    doc({ kind: "field", id: "f", props: { label: "Name", value: { bind: "$.user.name" } } }),
  );
  assert.match(source, /value=\{_text\(data\?\.user\?\.name\)\}/);
  assert.match(source, /onChange\?\.\("\$\.user\.name", _e\.currentTarget\.value\)/);
});

test("SPEC behaviour the generator follows: text prop, submit buttons, empty slots", async () => {
  const d = doc(
    { kind: "heading", id: "h", props: { level: 2, text: { bind: "$.title" } } },
    {
      kind: "form",
      id: "f",
      children: [{ kind: "button", id: "s", props: { submit: true }, children: ["Go"] }],
    },
    {
      kind: "list",
      id: "l",
      children: [
        {
          kind: "each",
          id: "e",
          props: { in: { bind: "$.items" }, as: "it" },
          children: [{ kind: "item", id: "i", children: ["x"] }],
        },
      ],
      slots: { empty: [{ kind: "text", id: "none", children: ["Nothing"] }] },
    },
  );
  const component = await load(jsx(d));
  const full = markup(component, { data: { title: "T", items: [1] } });
  assert.match(full, /<h2 data-weft-id="h">T<\/h2>/);
  assert.match(full, /type="submit"/);
  assert.doesNotMatch(full, /Nothing/);
  assert.match(markup(component, { data: { title: "T", items: [] } }), /Nothing/);
});

// Readings the corpus does not reach, each rendered with data that takes every branch.
test("edge cases render like the reference renderer", async () => {
  const each = (src: string, children: El[]): El => ({
    kind: "each",
    id: `e-${src}`,
    props: { in: { bind: `$.${src}` }, as: "it" },
    children,
  });
  const d = doc(
    { kind: "heading", id: "h1", props: { level: { bind: "$.level" } }, children: ["H"] },
    { kind: "heading", id: "h2", props: { level: 9, text: "  a   b " } },
    { kind: "grid", id: "g", props: { columns: { bind: "$.level" } } },
    { kind: "button", id: "b", props: { text: { bind: "$.title" } }, children: ["Content wins"] },
    { kind: "button", id: "b2", props: { text: { bind: "$.title" } } },
    {
      kind: "item-host",
      id: "x",
      props: { role: "group" },
      slots: { b: [{ kind: "text", id: "xb", children: ["b"] }], header: [] },
    },
    {
      kind: "list",
      id: "l",
      props: { state: { bind: "$.state" } },
      children: [
        each("items", [{ kind: "item", id: "i", props: { text: { bind: "$it" } } }]),
        { kind: "item", id: "hid", props: { hidden: { bind: "$.hide" } }, children: ["h"] },
      ],
      slots: { empty: [{ kind: "text", id: "none", children: ["Nothing"] }] },
    },
    {
      kind: "list",
      id: "l2",
      children: [each("items", [{ kind: "item", id: "i2", children: ["x"] }])],
      slots: { empty: [{ kind: "text", id: "none2", children: ["Nothing"] }] },
    },
    {
      kind: "table",
      id: "t",
      props: { label: "T" },
      children: [
        { kind: "column", id: "c1", children: ["A"] },
        each("items", [{ kind: "column", id: "c", props: { text: { bind: "$it" } } }]),
        each("items", [{ kind: "row", id: "r", children: [{ kind: "cell", id: "rc" }] }]),
      ],
      slots: { empty: [{ kind: "text", id: "none3", children: ["Empty"] }] },
    },
    {
      kind: "radio-group",
      id: "rg",
      props: { label: { bind: "$.title" } },
      children: [
        { kind: "radio", id: "r1", props: { value: "a", label: "Name" }, children: ["Shown"] },
      ],
    },
    { kind: "field", id: "f", props: { label: "" } },
    {
      kind: "section",
      id: "s",
      props: { label: "S" },
      children: [{ kind: "text", id: "st", children: ["body"] }],
      slots: { header: [{ kind: "heading", id: "sh", props: { level: 2 }, children: ["Head"] }] },
    },
  );
  const component = await load(jsx(d));
  for (const data of [
    { level: 9.4, title: "T", items: ["p", "q"], state: "ready", hide: false },
    { level: 0, title: "  ", items: [], state: "empty", hide: true },
    { level: "3", items: "none", state: "", hide: false },
    {},
  ]) {
    assert.equal(
      normalizeMarkup(markup(component, { data })),
      normalizeMarkup(renderToStaticMarkup(render(d, { catalog: coreCatalog, data }))),
      JSON.stringify(data),
    );
  }
});

test("a submit button reports press, then its form's submit", async () => {
  const source = jsx(
    doc({
      kind: "form",
      id: "f",
      on: { submit: "form.send" },
      children: [
        {
          kind: "stack",
          id: "st",
          children: [
            { kind: "button", id: "go", props: { submit: true }, on: { press: "go.press" } },
          ],
        },
      ],
    }),
  );
  const calls: string[] = [];
  const component = (await load(source)) as (props: unknown) => unknown;
  // Server rendering attaches no handlers, so the element tree is walked to reach them.
  const tree = component({
    actions: {
      "go.press": () => calls.push("press"),
      "form.send": () => calls.push("submit"),
    },
  });
  const find = (n: unknown): ((e: unknown) => void) | undefined => {
    if (Array.isArray(n)) return n.map(find).find(Boolean);
    if (typeof n !== "object" || n === null) return undefined;
    const props = (n as { props?: Record<string, unknown> }).props ?? {};
    if (props["data-weft-id"] === "go") return props["onClick"] as (e: unknown) => void;
    return find(props["children"]);
  };
  let prevented = false;
  find(tree)?.({ preventDefault: () => (prevented = true) });
  assert.deepEqual(calls, ["press", "submit"]);
  assert.equal(prevented, true);
});

test("non-documents and an invalid component name", () => {
  for (const bad of [null, 1, "x", [], {}, { root: 5 }, { root: { kind: 1 } }]) {
    const source = jsx(bad);
    assertShape(source);
    assert.match(source, /return null;/);
  }
  assert.throws(() => jsx(doc(), "x; alert(1)"), TypeError);
  assert.match(jsx(doc(), "Settings"), /export default function Settings\(/);
});

test("output is deterministic", () => {
  const d = doc({ kind: "text", id: "t", children: ["a"] });
  assert.equal(jsx(d), jsx(structuredClone(d)));
});

test("deep nesting stops at the depth limit instead of overflowing", () => {
  let node: El = { kind: "section", id: "s0", props: { label: "x" }, children: [] };
  const root = node;
  for (let i = 1; i < 5000; i++) {
    const next: El = { kind: "section", id: `s${i}`, props: { label: "x" }, children: [] };
    node.children = [next];
    node = next;
  }
  assertShape(jsx(doc(root)));
});

// Arbitrary JSON in document positions: the generator never throws and never emits code
// outside the expected module shape.
const json = fc.letrec((tie) => ({
  value: fc.oneof(
    { depthSize: "small" },
    fc.string(),
    fc.constantFrom(...HOSTILE),
    fc.integer(),
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
          ),
          tie("value"),
          { maxKeys: 4 },
        ),
        on: fc.dictionary(
          fc.constantFrom("press", "change", "submit"),
          fc.oneof(fc.string(), fc.constantFrom(...HOSTILE)),
          { maxKeys: 2 },
        ),
        children: fc.array(tie("value"), { maxLength: 3 }),
        slots: fc.dictionary(fc.string(), fc.array(tie("value"), { maxLength: 2 }), { maxKeys: 2 }),
      },
      { requiredKeys: ["kind"] },
    ),
  ),
}));

// A `value` bound to something that is not a path has no write-back, and without `on-change`
// there is nothing left for the handler to do but read: it must still be a valid arrow function.
test("a read-only bound control keeps a valid change handler on every target", () => {
  for (const framework of ["react", "solid", "lit"] as const) {
    for (const kind of ["slider", "stepper", "date-picker", "color-picker", "combobox"]) {
      const d = doc({ kind, id: "c", props: { label: "L", value: { bind: "no path" } } });
      const source = toJsx(d, { catalog: coreCatalog, framework });
      const { errors } = parseSync("screen.jsx", source);
      assert.deepEqual(errors, [], `${framework} ${kind}\n${source}`);
      assert.doesNotMatch(source, /=> const /, `${framework} ${kind}`);
    }
  }
});

test("property: any input yields a well-formed module", () => {
  fc.assert(
    fc.property(json.value, (root) => {
      assertShape(jsx({ weft: "0.1", root }));
    }),
    { numRuns: 300 },
  );
});

// The helpers are plain JS strings; they are checked against the renderer's own functions.
function helper<T>(name: string): T {
  const source = RUNTIME[name];
  assert.ok(source);
  // Test-only evaluation of this package's own constant source, never of document content.
  return new Function(`${source}\nreturn ${name};`)() as T;
}

test("runtime: _url matches the renderer's safeUrl", () => {
  const url = helper<(v: string) => string | undefined>("_url");
  const samples = fc.oneof(
    fc.string(),
    fc.constantFrom(
      "javascript:x",
      " java\tscript:x",
      "HTTPS://a",
      "mailto:a",
      "/a:b",
      "a?b:c",
      "\u0000http://a",
      "data:x",
      "",
    ),
  );
  fc.assert(
    fc.property(samples, (s) => url(s) === safeUrl(s)),
    { numRuns: 500 },
  );
});

test("runtime: _text, _on, _level, _num, _squash, _float and _get", () => {
  const text = helper<(v: unknown) => string>("_text");
  const on = helper<(v: unknown) => boolean>("_on");
  const level = helper<(v: unknown) => number>("_level");
  const float = helper<(v: string) => string>("_float");
  const get = helper<(v: unknown, p: string[]) => unknown>("_get");
  const num = helper<(v: unknown, i: boolean, min?: number, max?: number) => unknown>("_num");
  const squash = helper<(v: string) => string>("_squash");
  assert.deepEqual(
    [text("a"), text(1), text(Number.NaN), text(true), text(null), text({})],
    ["a", "1", "", "true", "", ""],
  );
  assert.deepEqual(
    [on("false"), on(""), on("true"), on(0), on([])],
    [false, false, true, false, true],
  );
  assert.deepEqual([level("3"), level(7), level(2.5), level(undefined)], [3, 2, 2, 2]);
  assert.deepEqual(
    [
      num(9.4, true, 1, 6),
      num(0.2, false, 1),
      num(2.5, true),
      num("7", true, 1, 6),
      num(Infinity, false),
    ],
    [6, 1, 3, "7", undefined],
  );
  assert.equal(squash("  a \n b "), "a b");
  assert.deepEqual([float("1.5"), float("1e3"), float("x"), float("")], ["1.5", "1e3", "", ""]);
  assert.deepEqual(
    [get({ a: [{ b: 1 }] }, ["a", "0", "b"]), get({}, ["constructor"]), get("abc", ["length"])],
    [1, undefined, undefined],
  );
});

test("the number, date, colour and combobox controls read bound data like the renderer", async () => {
  const d = doc(
    {
      kind: "slider",
      id: "sl",
      props: {
        label: "S",
        value: { bind: "$.n" },
        min: { bind: "$.lo" },
        max: { bind: "$.hi" },
        step: { bind: "$.st" },
      },
    },
    { kind: "slider", id: "sl2", props: { label: "S2", value: 3, min: 1, max: 9, step: 4 } },
    {
      kind: "stepper",
      id: "sp",
      props: {
        label: "P",
        value: { bind: "$.n" },
        min: { bind: "$.lo" },
        max: { bind: "$.hi" },
        step: { bind: "$.st" },
      },
    },
    { kind: "stepper", id: "sp2", props: { label: "P2", disabled: true } },
    {
      kind: "date-picker",
      id: "dp",
      props: {
        label: "D",
        type: { bind: "$.t" },
        value: { bind: "$.d" },
        min: { bind: "$.dmin" },
        max: "2030-12-31",
      },
    },
    {
      kind: "date-picker",
      id: "dp2",
      props: { label: "D2", type: "datetime", value: "2026-10-05T09:30" },
    },
    { kind: "color-picker", id: "cp", props: { label: "C", value: { bind: "$.c" } } },
    {
      kind: "combobox",
      id: "cb",
      props: { label: "B", value: { bind: "$.b" }, placeholder: { bind: "$.ph" } },
      on: { change: "typed" },
      children: [
        { kind: "option", id: "o1", props: { value: "apple" }, children: ["Apple"] },
        {
          kind: "each",
          id: "e",
          props: { in: { bind: "$.items" }, as: "it" },
          children: [{ kind: "option", id: "o2", props: { value: { bind: "$it" } } }],
        },
      ],
    },
    {
      kind: "segmented-control",
      id: "sc",
      props: { label: "V", value: { bind: "$.seg" } },
      on: { change: "chose" },
      children: [
        { kind: "segment", id: "sg1", props: { value: "day" }, children: ["Day"] },
        {
          kind: "segment",
          id: "sg2",
          props: { value: "week", disabled: true },
          children: ["Week"],
        },
      ],
    },
  );
  const datas = [
    {
      n: 7.3,
      lo: 2,
      hi: 12,
      st: 2.5,
      t: "time",
      d: "09:30",
      dmin: "08:00",
      c: "#ABCDEF",
      b: "app",
      ph: "Pick",
      items: ["x", "y"],
      seg: "week",
    },
    {
      n: "5",
      lo: "1",
      hi: 0,
      st: 0,
      t: "datetime",
      d: "2026-02-30T10:00",
      dmin: "x",
      c: "red",
      b: 4,
      items: "none",
      seg: "",
    },
    {
      n: 1e21,
      lo: "0x10",
      hi: "Infinity",
      st: -3,
      t: "weird",
      d: "2026-10-05",
      c: "#12345",
      items: [],
    },
    { n: 0.1, lo: 0, hi: 1, st: 0.1 },
    {},
  ];
  for (const framework of ["react", "solid"] as const) {
    const source = toJsx(d, { catalog: coreCatalog, framework });
    const run: (props: Props) => string =
      framework === "react"
        ? (
            (c) => (props: Props) =>
              markup(c, props)
          )(await load(source))
        : await loadSolid(source);
    for (const data of datas) {
      assert.equal(
        normalizeMarkup(run({ data })),
        normalizeMarkup(renderToStaticMarkup(render(d, { catalog: coreCatalog, data }))),
        `${framework} ${JSON.stringify(data)}`,
      );
    }
  }
});
