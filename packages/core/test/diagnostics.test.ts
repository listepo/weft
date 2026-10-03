import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DIAGNOSTIC_CODES,
  parse,
  serialize,
  validate,
  type DiagnosticCode,
  type Diagnostic,
  type ValidateOptions,
} from "../src/index.ts";
import { catalog, tokens } from "./catalog.ts";

const screen = (inner: string, weft = "0.1") => `<screen id="s" weft="${weft}">${inner}</screen>`;
const doc = (child: unknown) => ({
  weft: "0.1",
  root: { kind: "screen", id: "s", children: [child] },
});

type Case = {
  markup?: string;
  json?: unknown;
  options?: Pick<ValidateOptions, "tokens" | "actions">;
};

const cases: Record<DiagnosticCode, Case> = {
  W101: { markup: screen('<text id="t" tone="muted"value="x"/>') },
  W102: { markup: `<?xml version="1.0"?>${screen("")}` },
  W103: { markup: `<!DOCTYPE screen>${screen("")}` },
  W104: { markup: screen('<text id="t"><![CDATA[x]]></text>') },
  W105: { markup: screen('<Button id="b">x</Button>') },
  W106: { markup: screen("<text id='t'>x</text>") },
  W107: { markup: screen('<field id="f" label="L" required/>') },
  W108: { markup: screen('<text id="t" tone="muted" tone="danger">x</text>') },
  W109: { markup: screen('<form id="f"><text id="t">x</form></text>') },
  W110: { markup: '<screen id="s" weft="0.1"><form id="f">' },
  W111: { markup: `${screen("")}</form>` },
  W112: { markup: screen('<text id="t">a &nbsp; b</text>') },
  W113: { markup: screen('<text id="t" value="a<b"/>') },
  W114: { markup: `hello ${screen("")}` },
  W115: { markup: screen("<!-- a -- b -->") },
  W116: { markup: screen('<text id="t" value="{oops}"/>') },
  W117: { markup: screen(`${'<stack id="x">'.repeat(300)}${"</stack>".repeat(300)}`) },
  W118: { markup: screen('<form id="f"><slot/></form>') },
  W119: {
    markup: screen(
      '<form id="f"><slot name="footer"><text id="a">a</text></slot><slot name="footer"><text id="b">b</text></slot></form>',
    ),
  },
  W200: { json: { weft: "0.1", root: { kind: "screen", id: "s", children: [{ id: 1 }] } } },
  W201: { markup: '<form id="f" weft="0.1"/>' },
  W202: { markup: screen("<text>x</text>") },
  W203: { markup: screen('<button id="b" variant="primay">x</button>') },
  W204: { markup: screen('<heading id="h" level="two">x</heading>') },
  W205: { markup: screen('<heading id="h">x</heading>') },
  W206: { markup: screen('<text id="t" on-press="a.b">x</text>') },
  W207: { markup: screen('<form id="f"><slot name="header"><text id="t">x</text></slot></form>') },
  W208: { markup: screen('<dialog id="d" label="L"><text id="t">x</text></dialog>') },
  W209: { markup: screen('<button id="b" role="link">x</button>') },
  W210: { markup: screen('<x-acme-map id="m"/>') },
  W211: { markup: screen('<x-acme-map id="m" role="mapp"/>') },
  W212: { markup: screen('<text id="1t">x</text>') },
  W213: { markup: screen('<text id="t" value="Hello {$.name}"/>') },
  W214: { markup: screen('<text id="t" value="{$..a}"/>') },
  W215: { markup: screen('<stack id="st" gap="{token.}"/>') },
  W216: { markup: screen('<button id="b" on-press="Bad Action">x</button>') },
  W217: { markup: screen('<x-acme-map id="m" role="{$.role}"/>') },
  W218: { markup: screen('<field id="f" label="L" value="{!$.v}"/>') },
  W219: { markup: screen("", "one") },
  W220: { markup: screen('<x-map id="m" role="img"/>') },
  W221: { json: doc({ kind: "text", id: "t", children: [`a${String.fromCharCode(1)}`] }) },
  W222: {
    markup: screen(
      '<list id="l"><each id="e" as="item" in="$.items"><item id="i">x</item></each></list>',
    ),
  },
  W223: { json: doc({ kind: "text", id: "t", props: { "on-press": "a.b" } }) },
  W301: { markup: screen('<text id="t">a</text><text id="t">b</text>') },
  W302: { markup: screen('<list id="l"><text id="t">x</text></list>') },
  W303: { markup: screen('<item id="i">x</item>') },
  W304: { markup: screen('<button id="b"><text id="t">x</text></button>') },
  W305: { markup: screen('<text id="t" value="{$todo.title}"/>') },
  W306: { markup: screen('<stack id="st" gap="{token.space.xl}"/>'), options: { tokens } },
  W307: { markup: screen('<stack id="st" gap="{token.color.accent}"/>'), options: { tokens } },
  W308: {
    markup: screen('<button id="b" on-press="auth.sumbit">x</button>'),
    options: { actions: ["auth.submit"] },
  },
  W309: { markup: screen('<tabs id="tb" selected="nope"><tab id="a" label="A"/></tabs>') },
  W310: { markup: screen('<text id="t" value="x">y</text>') },
  W311: {
    markup: screen(
      '<list id="l"><each id="e1" as="row" in="{$.rows}"><each id="e2" as="row" in="{$row.items}"><item id="i">x</item></each></each></list>',
    ),
  },
  W312: { markup: screen('<screen id="inner" weft="0.1"/>') },
  W401: { markup: screen('<fancy id="f"/>') },
  W402: { markup: screen('<text id="t" colour="red">x</text>') },
  W403: { markup: screen("", "0.2") },
  W404: { markup: screen("", "1.0") },
};

function run(c: Case, mode: "lenient" | "strict" = "lenient"): Diagnostic[] {
  const options = { catalog, mode, ...c.options };
  return c.markup !== undefined ? parse(c.markup, options).diagnostics : validate(c.json, options);
}

test("every registered code has a case", () => {
  assert.deepEqual(Object.keys(cases).toSorted(), Object.keys(DIAGNOSTIC_CODES).toSorted());
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
    '<screen id="s" weft="0.2">',
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
