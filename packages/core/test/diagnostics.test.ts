import assert from "node:assert/strict";
import { test } from "vitest";
import {
  DIAGNOSTIC_CODES,
  parse,
  serialize,
  validate,
  type DiagnosticCode,
  type Diagnostic,
} from "../src/index.ts";
import { catalog } from "./catalog.ts";
import { cases, doc, screen, type Case } from "./cases.ts";

function run(c: Case, mode: "lenient" | "strict" = "lenient"): Diagnostic[] {
  const options = { catalog, mode, ...c.options };
  return c.markup !== undefined ? parse(c.markup, options).diagnostics : validate(c.json, options);
}

test("every registered code has a case", () => {
  assert.deepEqual(
    Object.keys(cases).toSorted(),
    Object.keys(DIAGNOSTIC_CODES)
      .filter((code) => !/^W[567]|^W31[56]$|^W810$/.test(code))
      .toSorted(),
  );
});

for (const [code, c] of Object.entries(cases)) {
  test(`${code}: ${DIAGNOSTIC_CODES[code as DiagnosticCode].summary}`, () => {
    const diagnostics = run(c);
    const hit = diagnostics.find((d) => d.code === code);
    assert.ok(hit, `${code} not reported; got ${JSON.stringify(diagnostics, null, 2)}`);
    for (const d of diagnostics) {
      assert.ok(d.message.length > 0);
      assert.match(d.path, /^[/#]/);
      assert.ok(d.expected !== undefined && d.expected.length > 0, `${d.code} has no expectation`);
      if (c.markup !== undefined)
        assert.ok(d.line !== undefined && d.column !== undefined, `${d.code} has no position`);
    }
  });
}

test("diagnostics carry a precise location and a repair hint", () => {
  const [d] = parse(screen('\n<form id="f1" state="submiting"/>'), { catalog }).diagnostics;
  assert.deepEqual(d, {
    code: "W203",
    severity: "error",
    message: '"submiting" is not an allowed value.',
    path: "/screen#s/form#f1/@state",
    line: 2,
    column: 15,
    expected: 'one of: "idle", "submitting", "invalid"',
    got: '"submiting"',
    hint: 'did you mean "submitting"?',
  });
});

test("text comes from content or from the text attribute, never both", () => {
  const valid = screen('<text id="a" text="{$.x}"/><link id="b" on-press="a.b">Go</link>');
  assert.deepEqual(parse(valid, { catalog, mode: "strict" }).diagnostics, []);
  const [d] = parse(screen('<text id="t" text="x">y</text>'), { catalog }).diagnostics;
  assert.equal(d?.code, "W310");
  assert.equal(d?.path, "/screen#s/text#t/@text");
});

test("a submit button is valid anywhere inside a form, slots and loops included", () => {
  const markup = screen(
    '<form id="f" on-submit="a.b"><stack id="st"><button id="b1" submit="true">A</button></stack><slot name="footer"><button id="b2" submit="true">B</button></slot></form>',
  );
  assert.deepEqual(parse(markup, { catalog, mode: "strict" }).diagnostics, []);
});

test("numbers are checked against min, max and integer", () => {
  const codes = (level: string) =>
    parse(screen(`<heading id="h" level="${level}">x</heading>`), { catalog }).diagnostics.map(
      (d) => [d.code, d.expected, d.hint],
    );
  assert.deepEqual(codes("1"), []);
  assert.deepEqual(codes("6"), []);
  assert.deepEqual(codes("0"), [["W224", "an integer from 1 to 6", "use 1"]]);
  assert.deepEqual(codes("2.5"), [["W224", "an integer from 1 to 6", "use 3"]]);
  assert.deepEqual(codes("9"), [["W224", "an integer from 1 to 6", "use 6"]]);
});

test("<each> repeats elements only, even inside a parent that takes text", () => {
  const markup = screen(
    '<list id="l"><item id="i"><each id="e" as="row" in="{$.rows}">Hi <text id="t" text="{$row.name}"/></each></item></list>',
  );
  assert.deepEqual(
    parse(markup, { catalog }).diagnostics.map((d) => [d.code, d.path]),
    [["W304", "/screen#s/list#l/item#i/each#e/#text[0]"]],
  );
});

test("JSON shape errors point into the JSON", () => {
  const [d] = run(cases.W200);
  assert.equal(d?.path, "#/root/children/0");
});

test("unknown elements and attributes warn in lenient mode and fail in strict mode", () => {
  for (const code of ["W401", "W402", "W403"] as const) {
    const lenient = run(cases[code]).filter((d) => d.code === code);
    const strict = run(cases[code], "strict").filter((d) => d.code === code);
    assert.deepEqual(
      lenient.map((d) => d.severity),
      ["warning"],
    );
    assert.deepEqual(
      strict.map((d) => d.severity),
      ["error"],
    );
  }
});

test("an unknown element inside a restricted parent is opaque, not a containment error", () => {
  const diagnostics = parse(screen('<list id="l"><fancy id="f"/></list>'), { catalog }).diagnostics;
  assert.deepEqual(
    diagnostics.map((d) => d.code),
    ["W401"],
  );
});

test("extension elements and attributes pass in both modes", () => {
  const markup = screen(
    '<x-acme-chart id="c" role="img" x-acme-kind="bar" anything="1"><text id="t" x-acme-note="n">x</text></x-acme-chart>',
  );
  for (const mode of ["lenient", "strict"] as const) {
    assert.deepEqual(parse(markup, { catalog, mode }).diagnostics, []);
  }
});

test("unknown content survives a round-trip", () => {
  const markup = [
    '<screen id="s" weft="0.4">',
    '  <fancy id="f" level="3" mood="calm">',
    "    Hello",
    '    <x-acme-chart id="c" role="img"/>',
    '    <slot name="aside">',
    '      <text id="t" colour="red">x</text>',
    "    </slot>",
    "  </fancy>",
    "</screen>",
    "",
  ].join("\n");
  const result = parse(markup, { catalog });
  assert.ok(result.document);
  assert.deepEqual(
    result.diagnostics.map((d) => [d.code, d.severity]),
    [
      ["W403", "warning"],
      ["W401", "warning"],
      ["W402", "warning"],
    ],
  );
  assert.equal(serialize(result.document), markup);
  assert.deepEqual(result.document.root.children?.[0], {
    kind: "fancy",
    id: "f",
    props: { level: "3", mood: "calm" },
    slots: {
      aside: [{ kind: "text", id: "t", props: { colour: "red" }, children: ["x"] }],
    },
    children: ["Hello", { kind: "x-acme-chart", id: "c", props: { role: "img" } }],
  });
});

test("validate on JSON has no positions; on parsed markup it has them", () => {
  const json = doc({ kind: "text", id: "t", props: { tone: "loud" } });
  const [fromJson] = validate(json, { catalog });
  assert.equal(fromJson?.line, undefined);
  assert.equal(fromJson?.path, "/screen#s/text#t/@tone");
  const [fromMarkup] = parse(screen('<text id="t" tone="loud"/>'), { catalog }).diagnostics;
  assert.equal(fromMarkup?.path, fromJson?.path);
  assert.equal(fromMarkup?.line, 1);
  // A later pass over the parsed document keeps the positions too.
  const parsed = parse(screen('<text id="t" tone="loud"/>'));
  const [again] = validate(parsed.document, { catalog, source: parsed.source });
  assert.equal(again?.path, fromJson?.path);
  assert.equal(again?.line, 1);
});

test("parse without a catalog checks syntax only and keeps literals as strings", () => {
  const result = parse(screen('<heading id="h" level="2">x</heading><fancy/>'));
  assert.deepEqual(result.diagnostics, []);
  assert.deepEqual(result.document?.root.children?.[0], {
    kind: "heading",
    id: "h",
    props: { level: "2" },
    children: ["x"],
  });
});

test("hostile names do not reach Object.prototype", () => {
  const json: unknown = JSON.parse(
    '{"weft":"0.1","root":{"kind":"screen","id":"s","children":[{"kind":"constructor","id":"c","props":{"__proto__":"x"}}]}}',
  );
  const diagnostics = validate(json, { catalog });
  assert.ok(diagnostics.some((d) => d.code === "W401"));
  assert.ok(diagnostics.some((d) => d.code === "W223"));
});
