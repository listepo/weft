import assert from "node:assert/strict";
import { test } from "node:test";
import fc from "fast-check";
import {
  DIAGNOSTIC_CODES,
  applyPatches,
  canonicalize,
  hasErrors,
  parse,
  serialize,
  validate,
  type Diagnostic,
  type Document,
  type Node,
} from "../src/index.ts";
import { catalog, tokens } from "./catalog.ts";

const options = { catalog, tokens };

const BASE = `<screen id="s" weft="0.1">
  <stack id="main">
    <field id="email" label="Email" type="email" value="{$.email}"/>
    <field id="pw" label="Password" type="password"/>
    <form id="f" on-submit="auth.submit">
      <button id="go" variant="primary" on-press="auth.submit">Sign in</button>
      <slot name="footer">
        <link id="reset" on-press="nav.reset">Forgot password?</link>
      </slot>
    </form>
    <list id="l">
      <item id="i1">One</item>
      <item id="i2">Two</item>
      <each id="e" as="x" in="{$.xs}">
        <item id="i3">Three</item>
      </each>
    </list>
    <dialog id="dlg" label="Sure?">
      <slot name="actions">
        <button id="ok">OK</button>
      </slot>
    </dialog>
    <x-acme-box id="box" role="group"/>
  </stack>
</screen>
`;

function load(markup: string): Document {
  const { document, diagnostics } = parse(markup, { ...options, mode: "strict" });
  assert.ok(document, JSON.stringify(diagnostics));
  assert.deepEqual(diagnostics, []);
  return document;
}
const base = load(BASE);

function ok(patches: unknown, from: Document = base): Document {
  const result = applyPatches(from, patches, options);
  assert.ok(result.document, JSON.stringify(result.diagnostics, null, 2));
  assert.equal(hasErrors(result.diagnostics), false);
  return result.document;
}
function rejected(patches: unknown, code: string, from: Document = base): Diagnostic {
  const result = applyPatches(from, patches, options);
  assert.equal(result.document, undefined, "nothing may be applied");
  const hit = result.diagnostics.find((d) => d.code === code);
  assert.ok(hit, `${code} expected, got ${JSON.stringify(result.diagnostics, null, 2)}`);
  assert.ok(hit.expected !== undefined && hit.expected.length > 0, "expected is required");
  assert.match(hit.path, /^[/#]/);
  return hit;
}
const text = (document: Document) => serialize(document);

/** The element reached by following child indexes from `node`. */
function child(node: Node, ...path: number[]): Node {
  let current = node;
  for (const index of path) {
    const next = current.children?.[index];
    assert.ok(typeof next === "object", `no element at ${index}`);
    current = next;
  }
  return current;
}

// One patch list per patch code; the registry test below keeps this table complete.
const failures: Record<string, [unknown, Document?]> = {
  W501: [[{ op: "explode" }]],
  W502: [[{ op: "remove", id: "emial" }]],
  W503: [[{ op: "set", id: "go", prop: "id", value: "x" }]],
  W504: [[{ op: "insert", parent: "main", slot: "footer", markup: '<text id="t">x</text>' }]],
  W505: [[{ op: "insert", parent: "main", index: 99, markup: '<text id="t">x</text>' }]],
  W506: [[{ op: "move", id: "f", parent: "go" }]],
  W507: [[{ op: "remove", id: "s" }]],
  W508: [[{ op: "insert", parent: "main", markup: "just text" }]],
  W509: [[{ op: "insert", parent: "main", markup: '<text id="go">x</text>' }]],
};

test("every patch code has a failing case", () => {
  const registered = Object.keys(DIAGNOSTIC_CODES).filter((code) => code.startsWith("W5"));
  assert.deepEqual(Object.keys(failures).toSorted(), registered.toSorted());
  for (const [code, [patches, from]] of Object.entries(failures)) rejected(patches, code, from);
});

test("set: literal, binding, token and removal", () => {
  const out = ok([
    { op: "set", id: "go", prop: "variant", value: "danger" },
    { op: "set", id: "go", prop: "disabled", value: { bind: "$.busy", not: true } },
    { op: "set", id: "main", prop: "gap", value: { token: "space.md" } },
    { op: "set", id: "email", prop: "value", value: null },
  ]);
  const s = text(out);
  assert.match(s, /<button id="go" disabled="\{!\$\.busy\}" variant="danger"/);
  assert.match(s, /<stack id="main" gap="\{token\.space\.md\}">/);
  assert.match(s, /<field id="email" label="Email" type="email"\/>/);
});

test("set text: content-form text is replaced in place; prop-form text is set", () => {
  const swapped = text(ok([{ op: "set", id: "go", prop: "text", value: "Log in" }]));
  assert.match(
    swapped,
    /<button id="go" variant="primary" on-press="auth.submit">Log in<\/button>/,
  );
  const bound = text(ok([{ op: "set", id: "i1", prop: "text", value: { bind: "$.first" } }]));
  assert.match(bound, /<item id="i1" text="\{\$\.first\}"\/>/);
  const cleared = text(ok([{ op: "set", id: "reset", prop: "text", value: null }]));
  assert.match(cleared, /<link id="reset" on-press="nav.reset"\/>/);
  // Once the text is a prop, a literal stays a prop: set writes the spelling already in use.
  const relit = text(
    ok([
      { op: "set", id: "i1", prop: "text", value: { bind: "$.first" } },
      { op: "set", id: "i1", prop: "text", value: "One again" },
    ]),
  );
  assert.match(relit, /<item id="i1" text="One again"\/>/);
  // An element without text takes it as the prop, like any other prop.
  assert.match(
    text(
      ok([{ op: "set", id: "ok", prop: "text", value: "Fine" }], load(BASE.replace(">OK<", "><"))),
    ),
    /<button id="ok" text="Fine"\/>/,
  );
});

test("set text: a mixed component with element content keeps it, so both texts are W310", () => {
  const withChild = load(
    BASE.replace('<item id="i2">Two</item>', '<item id="i2"><text id="t2">Two</text></item>'),
  );
  rejected([{ op: "set", id: "i2", prop: "text", value: "Two" }], "W310", withChild);
  // On a kind that holds no text, text is an unknown attribute.
  const strict = { ...options, mode: "strict" as const };
  const unknown = applyPatches(base, [{ op: "set", id: "main", prop: "text", value: "x" }], strict);
  assert.equal(unknown.document, undefined);
  assert.equal(unknown.diagnostics[0]?.code, "W402");
});

test("set: removing an absent prop is a no-op", () => {
  assert.equal(text(ok([{ op: "set", id: "go", prop: "disabled", value: null }])), text(base));
});

test("set: typed values are checked, so a wrong type is rejected", () => {
  const d = rejected([{ op: "set", id: "go", prop: "disabled", value: "yes" }], "W204");
  assert.equal(d.path, "/screen#s/stack#main/form#f/button#go/@disabled");
});

test("set: on-<event> binds and unbinds an action", () => {
  const out = ok([
    { op: "set", id: "go", prop: "on-press", value: "auth.retry" },
    { op: "set", id: "f", prop: "on-submit", value: null },
  ]);
  const s = text(out);
  assert.match(s, /id="go" variant="primary" on-press="auth\.retry"/);
  assert.doesNotMatch(s, /on-submit/);
  rejected([{ op: "set", id: "go", prop: "on-hover", value: "x" }], "W206");
  rejected([{ op: "set", id: "go", prop: "on-press", value: 3 }], "W503");
  rejected([{ op: "set", id: "go", prop: "on-press", value: { bind: "$.x" } }], "W503");
});

test("set: id, the version and malformed names are refused", () => {
  const id = rejected([{ op: "set", id: "go", prop: "id", value: "other" }], "W503");
  assert.equal(id.path, "#/patches/0/prop");
  assert.match(id.hint ?? "", /insert it again/);
  rejected([{ op: "set", id: "s", prop: "weft", value: "0.2" }], "W503");
  rejected([{ op: "set", id: "go", prop: "__proto__", value: "x" }], "W503");
  rejected([{ op: "set", id: "go", prop: "Variant", value: "x" }], "W503");
  rejected([{ op: "set", id: "go", prop: "on-", value: "x" }], "W503");
});

test("set: an unknown id points at the nearest one", () => {
  const d = rejected([{ op: "set", id: "emial", prop: "label", value: "x" }], "W502");
  assert.equal(d.path, "#/patches/0/id");
  assert.equal(d.hint, 'did you mean "email"?');
  assert.equal(d.got, "emial");
});

test("insert: appends by default and honours index", () => {
  const appended = ok([{ op: "insert", parent: "main", markup: '<text id="t1">Hi</text>' }]);
  assert.match(
    text(appended),
    /<x-acme-box id="box" role="group"\/>\n {4}<text id="t1">Hi<\/text>/,
  );

  const first = ok([{ op: "insert", parent: "main", index: 0, markup: '<text id="t1">Hi</text>' }]);
  const firstKids = child(first.root, 0).children as Node[];
  assert.equal(firstKids[0]?.id, "t1");

  const middle = ok([
    { op: "insert", parent: "main", index: 1, markup: '<text id="t1">Hi</text>' },
  ]);
  const ids = (child(middle.root, 0).children as Node[]).map((n) => n.id);
  assert.deepEqual(ids.slice(0, 3), ["email", "t1", "pw"]);
});

test("insert: several elements keep their order; literals are typed by the catalog", () => {
  const out = ok([
    {
      op: "insert",
      parent: "main",
      index: 0,
      markup:
        '<heading id="h" level="2">T</heading>\n<!-- gap -->\n<button id="b" disabled="true">B</button>',
    },
  ]);
  const stack = child(out.root, 0);
  const [h, b] = stack.children as Node[];
  assert.deepEqual(h?.props, { level: 2 });
  assert.deepEqual(b?.props, { disabled: true });
});

test("insert: into a named slot, an existing one or a declared empty one", () => {
  const footer = ok([
    { op: "insert", parent: "f", slot: "footer", index: 0, markup: '<link id="ln">L</link>' },
  ]);
  const form = child(footer.root, 0, 2);
  assert.deepEqual(
    form.slots?.["footer"]?.map((n) => (n as Node).id),
    ["ln", "reset"],
  );

  const extra = ok([
    { op: "insert", parent: "dlg", slot: "actions", markup: '<button id="no">No</button>' },
  ]);
  assert.match(text(extra), /<button id="ok">OK<\/button>\n\s+<button id="no">No<\/button>/);
});

test("insert: any slot name is allowed on extension elements, none on <each>", () => {
  const out = ok([
    { op: "insert", parent: "box", slot: "aside", markup: '<text id="t1">x</text>' },
  ]);
  assert.match(text(out), /<slot name="aside">/);
  const d = rejected(
    [{ op: "insert", parent: "e", slot: "footer", markup: '<item id="n">x</item>' }],
    "W504",
  );
  assert.match(d.expected ?? "", /omit `slot`/);
});

test("insert: undeclared slot lists the declared ones", () => {
  const d = rejected(
    [{ op: "insert", parent: "f", slot: "footr", markup: '<link id="ln">L</link>' }],
    "W504",
  );
  assert.equal(d.expected, 'one of: "footer"');
  assert.equal(d.hint, 'did you mean "footer"?');
  assert.equal(d.path, "#/patches/0/slot");
});

test("insert: index must lie inside the list, text counts as an entry", () => {
  const d = rejected(
    [{ op: "insert", parent: "i1", index: 2, markup: '<text id="t1">x</text>' }],
    "W505",
  );
  assert.equal(d.expected, "an integer from 0 to 1 (text counts as an entry)");
  ok([{ op: "insert", parent: "i1", index: 1, markup: '<text id="t1">x</text>' }]);
});

test("insert: markup that is not a list of elements", () => {
  for (const markup of [
    "",
    "   ",
    "text",
    '<text id="a">x</text> loose',
    '<slot name="footer"/>',
  ]) {
    const d = rejected([{ op: "insert", parent: "main", markup }], "W508");
    assert.equal(d.path, "#/patches/0/markup");
  }
});

test("insert: syntax errors keep their code and locate the problem inside the markup", () => {
  const d = rejected(
    [{ op: "insert", parent: "main", markup: '<text id="a">x</text>\n<button id="b">' }],
    "W109",
  );
  assert.equal(d.path, "#/patches/0/markup/button#b");
  assert.equal(d.line, 2);
  const cols = rejected([{ op: "insert", parent: "main", markup: "<text id=a>x</text>" }], "W106");
  assert.equal(cols.line, 1);
  assert.equal(cols.column, 10);
  rejected([{ op: "insert", parent: "main", markup: '</x-weft-fragment><text id="a"/>' }], "W114");
});

test("insert: ids must be new, within the markup too", () => {
  const d = rejected([{ op: "insert", parent: "main", markup: '<text id="go">x</text>' }], "W509");
  assert.equal(d.hint, 'use "go-2"');
  assert.equal(d.got, "go");
  rejected(
    [{ op: "insert", parent: "main", markup: '<text id="a">x</text><text id="a">y</text>' }],
    "W301",
  );
  rejected(
    [{ op: "insert", parent: "main", markup: '<stack id="a"><text id="reset">x</text></stack>' }],
    "W509",
  );
});

test("insert: a validator rejection of the result applies nothing", () => {
  rejected([{ op: "insert", parent: "main", markup: '<item id="x">x</item>' }], "W303");
  rejected([{ op: "insert", parent: "main", markup: '<field id="x"/>' }], "W205");
});

test("insert: unknown content is a warning leniently and an error strictly", () => {
  const patches = [{ op: "insert", parent: "main", markup: '<fancy id="fz"/>' }];
  const lenient = applyPatches(base, patches, options);
  assert.ok(lenient.document);
  assert.deepEqual(
    lenient.diagnostics.map((d) => d.code),
    ["W401"],
  );
  const strict = applyPatches(base, patches, { ...options, mode: "strict" });
  assert.equal(strict.document, undefined);
});

test("remove: deletes the element and its subtree, in any slot", () => {
  const out = ok([
    { op: "remove", id: "reset" },
    { op: "remove", id: "i2" },
    { op: "remove", id: "e" },
  ]);
  const s = text(out);
  assert.doesNotMatch(s, /reset|"i2"|"i3"|<slot name="footer">/);
  assert.match(s, /<item id="i1">One<\/item>/);
});

test("remove: the root stays, and the result must still be valid", () => {
  const d = rejected([{ op: "remove", id: "s" }], "W507");
  assert.equal(d.path, "#/patches/0/id");
  rejected([{ op: "remove", id: "ok" }], "W208");
});

test("move: to another parent, into a slot, and inside one list", () => {
  const moved = ok([{ op: "move", id: "go", parent: "f", slot: "footer", index: 0 }]);
  const footer = child(moved.root, 0, 2).slots?.["footer"];
  assert.deepEqual(
    footer?.map((n) => (n as Node).id),
    ["go", "reset"],
  );

  // The index counts the list after the element left it.
  const reordered = ok([{ op: "move", id: "i1", parent: "l", index: 1 }]);
  const list = child(reordered.root, 0, 3);
  assert.deepEqual(
    list.children?.map((n) => (n as Node).id),
    ["i2", "i1", "e"],
  );
  rejected([{ op: "move", id: "i1", parent: "l", index: 3 }], "W505");
});

test("move: not into itself or its descendants, never the root", () => {
  const d = rejected([{ op: "move", id: "main", parent: "go" }], "W506");
  assert.equal(d.path, "#/patches/0/parent");
  rejected([{ op: "move", id: "f", parent: "f" }], "W506");
  rejected([{ op: "move", id: "s", parent: "main" }], "W507");
  rejected([{ op: "move", id: "go", parent: "nope" }], "W502");
  rejected([{ op: "move", id: "nope", parent: "main" }], "W502");
  rejected([{ op: "move", id: "go", parent: "f", slot: "bad" }], "W504");
});

test("patches apply in order, so later ones can use earlier results", () => {
  const out = ok([
    { op: "insert", parent: "main", markup: '<stack id="box2"/>' },
    { op: "insert", parent: "box2", markup: '<button id="b2">B</button>' },
    { op: "move", id: "go", parent: "box2", index: 0 },
    { op: "set", id: "b2", prop: "variant", value: "danger" },
    { op: "remove", id: "reset" },
  ]);
  const s = text(out);
  assert.match(s, /<stack id="box2">\n\s+<button id="go"/);
  assert.match(s, /<button id="b2" variant="danger">B<\/button>/);
});

test("atomic: a failing later patch leaves no trace", () => {
  const before = structuredClone(base);
  const result = applyPatches(
    base,
    [
      { op: "set", id: "go", prop: "variant", value: "danger" },
      { op: "remove", id: "i1" },
      { op: "remove", id: "i1" },
    ],
    options,
  );
  assert.equal(result.document, undefined);
  assert.deepEqual(
    result.diagnostics.map((d) => [d.code, d.path]),
    [["W502", "#/patches/2/id"]],
  );
  assert.deepEqual(base, before);
  // Patches that are fine one by one but break the document together.
  rejected(
    [
      { op: "set", id: "go", prop: "variant", value: "danger" },
      { op: "remove", id: "ok" },
    ],
    "W208",
  );
});

test("the input document is never mutated, even frozen", () => {
  const frozen = structuredClone(base);
  const freeze = (value: unknown): void => {
    if (typeof value !== "object" || value === null) return;
    for (const v of Object.values(value)) freeze(v);
    Object.freeze(value);
  };
  freeze(frozen);
  const out = ok(
    [
      { op: "set", id: "go", prop: "variant", value: "danger" },
      { op: "insert", parent: "f", slot: "footer", markup: '<link id="ln">L</link>' },
      { op: "move", id: "i1", parent: "l", index: 1 },
      { op: "remove", id: "pw" },
    ],
    frozen,
  );
  assert.deepEqual(frozen, base);
  assert.notEqual(out.root, frozen.root);
});

test("a result is canonical and survives a markup round trip", () => {
  const out = ok([{ op: "insert", parent: "main", markup: '<text id="t1" tone="muted">x</text>' }]);
  assert.deepEqual(canonicalize(out), out);
  assert.deepEqual(load(serialize(out)), out);
});

test("shape errors name the patch and show the expected form", () => {
  const notList = rejected({ op: "remove", id: "go" }, "W501");
  assert.equal(notList.path, "#/patches");
  const bad = rejected(
    [
      { op: "remove", id: "go" },
      { op: "insert", parent: "main", index: -1, markup: "<x/>" },
      { op: "set", id: "go", prop: "variant" },
      { op: "remove", id: "go", extra: 1 },
      "remove go",
      null,
    ],
    "W501",
  );
  assert.match(bad.path, /^#\/patches\/1\/index$/);
  assert.match(bad.expected ?? "", /"op":"insert"/);
  const all = applyPatches(base, [{ op: "move" }, 7], options).diagnostics;
  assert.ok(all.some((d) => d.path.startsWith("#/patches/0")));
  assert.ok(all.some((d) => d.path.startsWith("#/patches/1")));
  assert.match(all.find((d) => d.path === "#/patches/1")?.hint ?? "", /"op" must be/);
});

test("an empty list is a valid no-op", () => {
  assert.equal(text(ok([])), text(base));
});

test("hostile input never throws", () => {
  fc.assert(
    fc.property(fc.anything({ withNullPrototype: true, withBigInt: false }), (junk) => {
      const result = applyPatches(base, junk, options);
      assert.ok(result.document !== undefined || hasErrors(result.diagnostics));
    }),
    { numRuns: 300 },
  );
});

// ---- property test: random patch sequences over generated documents ----

type Spec = { kind: "stack"; children: Spec[] } | { kind: "button" } | { kind: "link" };

const specArb: fc.Arbitrary<Spec> = fc.letrec<{ spec: Spec }>((tie) => ({
  spec: fc.oneof(
    { depthSize: "small", withCrossShrink: true },
    fc.constant<Spec>({ kind: "button" }),
    fc.constant<Spec>({ kind: "link" }),
    fc.record({
      kind: fc.constant("stack" as const),
      children: fc.array(tie("spec"), { maxLength: 3 }),
    }),
  ),
})).spec;

function build(specs: Spec[]): Document {
  let n = 0;
  const make = (spec: Spec): Node => {
    const id = `n${n++}`;
    if (spec.kind === "stack") return { kind: "stack", id, children: spec.children.map(make) };
    return { kind: spec.kind, id, children: [id] };
  };
  const document = {
    weft: "0.1",
    root: { kind: "screen", id: "root", children: specs.map(make) },
  };
  assert.deepEqual(validate(document, options), []);
  return canonicalize(document);
}

const allIds = (node: Node, out: string[] = []): string[] => {
  if (node.id !== undefined) out.push(node.id);
  for (const child of [...(node.children ?? []), ...Object.values(node.slots ?? {}).flat()])
    if (typeof child !== "string") allIds(child, out);
  return out;
};

// A target is an index into the live ids; -1 names an id that does not exist.
const target = fc.oneof(
  { weight: 9, arbitrary: fc.nat(40) },
  { weight: 1, arbitrary: fc.constant(-1) },
);

const patchArb = fc.oneof(
  fc.record({
    op: fc.constant("set"),
    id: target,
    prop: fc.constantFrom("variant", "disabled", "label", "href", "on-press", "id", "wrap"),
    value: fc.oneof(
      fc.constant(null),
      fc.constantFrom("primary", "x", true, 7),
      fc.constant({ bind: "$.a" }),
    ),
  }),
  fc.record({
    op: fc.constant("insert"),
    parent: target,
    slot: fc.option(fc.constantFrom("footer", "aside"), { nil: undefined }),
    index: fc.option(fc.nat(5), { nil: undefined }),
    markup: fc.constantFrom(
      '<button id="k1">Go</button>',
      '<stack id="k2"><link id="k3">L</link></stack>',
      '<button id="k1">A</button><button id="k4">B</button>',
      "<broken",
    ),
  }),
  fc.record({ op: fc.constant("remove"), id: target }),
  fc.record({
    op: fc.constant("move"),
    id: target,
    parent: target,
    slot: fc.option(fc.constantFrom("footer"), { nil: undefined }),
    index: fc.option(fc.nat(5), { nil: undefined }),
  }),
);

type Raw = { id?: number; parent?: number } & Record<string, unknown>;
function resolve(raw: Raw, ids: string[]): Record<string, unknown> {
  const pick = (n: number) => (n < 0 ? "missing" : (ids[n % ids.length] as string));
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(raw)) {
    if (value === undefined) continue;
    out[key] = key === "id" || key === "parent" ? pick(value as number) : value;
  }
  return out;
}

test("property: random patch sequences never throw and give a valid document or diagnostics", () => {
  let accepted = 0;
  let refused = 0;
  fc.assert(
    fc.property(
      fc.array(specArb, { minLength: 1, maxLength: 3 }),
      fc.array(patchArb, { maxLength: 6 }),
      (specs, raws) => {
        const document = build(specs);
        const snapshot = structuredClone(document);
        let current = document;
        const applied: unknown[] = [];
        for (const raw of raws) {
          const patch = resolve(raw as Raw, allIds(current.root));
          const result = applyPatches(current, [patch], options);
          if (result.document === undefined) {
            refused++;
            assert.ok(hasErrors(result.diagnostics), "a rejection carries an error");
            assert.ok(result.diagnostics.every((d) => d.expected !== undefined));
            continue;
          }
          assert.equal(hasErrors(validate(result.document, options)), false);
          assert.deepEqual(canonicalize(result.document), result.document);
          const ids = allIds(result.document.root);
          assert.equal(new Set(ids).size, ids.length, "ids stay unique");
          accepted++;
          applied.push(patch);
          current = result.document;
        }
        assert.deepEqual(document, snapshot);
        // The same accepted patches as one list reach the same document.
        const together = applyPatches(document, applied, options);
        assert.deepEqual(together.document, current);
      },
    ),
    { numRuns: 400 },
  );
  // Guards against a generator that only ever produces one of the two outcomes.
  assert.ok(accepted > 100 && refused > 100, `${accepted} accepted, ${refused} refused`);
});
