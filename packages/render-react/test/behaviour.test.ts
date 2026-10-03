// Bindings, repetition, slots, events, writable props, the URL trust boundary and SPEC §8 fallback.
import assert from "node:assert/strict";
import { test } from "node:test";
import fc from "fast-check";
import { coreCatalog } from "@weft/catalog";
import type { Document } from "@weft/core";
import { expectedTree, safeUrl, type ActionEvent } from "../src/index.ts";
import { b, doc, dom, el, html, nb, propsOf, tree } from "./helpers.ts";

test("bindings resolve against data; missing ones render empty", () => {
  const v = dom(
    doc(
      el("text", "a", { value: b("$.user.name") }),
      el("text", "m", { value: b("$.nope.deep") }),
      el("text", "n", { value: b("$.n") }),
    ),
    {
      data: { user: { name: "Ana" }, n: 3 },
    },
  );
  assert.equal(v.text(v.byId("a")), "Ana");
  assert.equal(v.text(v.byId("m")), "");
  assert.equal(v.text(v.byId("n")), "3");
});

test("paths never reach inherited properties", () => {
  const v = dom(
    doc(
      el("text", "a", { value: b("$.constructor.name") }),
      el("text", "p", { value: b("$.__proto__") }),
    ),
    { data: {} },
  );
  assert.equal(v.text(v.byId("a")), "");
  assert.equal(v.text(v.byId("p")), "");
});

test("a negated binding is the boolean negation of its value", () => {
  const d = doc(el("button", "x", { disabled: nb("$.email") }, ["Go"]));
  assert.equal(dom(d, { data: { email: "" } }).byId("x").attribs["disabled"], "");
  assert.equal(dom(d, { data: { email: "a@b.c" } }).byId("x").attribs["disabled"], undefined);
  assert.equal(dom(d, { data: {} }).byId("x").attribs["disabled"], "");
});

test("each expands per item with instance ids and loop variables, nested too", () => {
  const d = doc(
    el("list", "l", {}, [
      el("each", "e", { in: b("$.groups"), as: "g" }, [
        el("item", "i", {}, [
          el("text", "t", { value: b("$g.name") }),
          el("each", "e2", { in: b("$g.tags"), as: "tag" }, [
            el("text", "tag", { value: b("$tag") }),
          ]),
        ]),
      ]),
    ]),
  );
  const v = dom(d, {
    data: {
      groups: [
        { name: "A", tags: ["x", "y"] },
        { name: "B", tags: [] },
      ],
    },
  });
  assert.equal(v.text(v.byId("t[0]")), "A");
  assert.equal(v.text(v.byId("t[1]")), "B");
  assert.equal(v.text(v.byId("tag[0][1]")), "y");
  assert.equal(v.has("tag[1][0]"), false);
  assert.equal(v.has("e"), false, "each itself renders nothing");
});

test("each over a missing or non-array binding renders nothing", () => {
  const d = doc(
    el("list", "l", {}, [el("each", "e", { in: b("$.todos"), as: "t" }, [el("item", "i")])]),
  );
  assert.equal(dom(d, { data: { todos: "nope" } }).has("i[0]"), false);
  assert.equal(dom(d, { data: {} }).has("i[0]"), false);
});

test("hidden elements are not rendered", () => {
  const v = dom(
    doc(el("text", "a", { hidden: true }, ["x"]), el("text", "c", { hidden: b("$.h") }, ["y"])),
    { data: { h: false } },
  );
  assert.equal(v.has("a"), false);
  assert.equal(v.has("c"), true);
});

test("press, submit and close dispatch the named action with id and loop item", () => {
  const calls: ActionEvent[] = [];
  const actions = {
    "doc.run": (e: ActionEvent) => calls.push(e),
    "auth.submit": (e: ActionEvent) => calls.push(e),
  };
  const d = doc(
    el("menu", "m", { label: "M" }, [
      el("each", "e", { in: b("$.items"), as: "it" }, [
        el("menu-item", "mi", {}, ["Run"], { on: { press: "doc.run" } }),
      ]),
    ]),
    el("form", "f", {}, [], { on: { submit: "auth.submit" } }),
    el("button", "unbound", {}, ["x"], { on: { press: "not.there" } }),
  );
  const t = tree(d, { data: { items: [1, 2] }, actions });
  (propsOf(t, "mi[1]")["onClick"] as () => void)();
  let prevented = false;
  (propsOf(t, "f")["onSubmit"] as (e: unknown) => void)({
    preventDefault: () => (prevented = true),
  });
  (propsOf(t, "unbound")["onClick"] as () => void)();
  assert.deepEqual(calls, [
    { id: "mi[1]", action: "doc.run", item: "$.items.1" },
    { id: "f", action: "auth.submit" },
  ]);
  assert.ok(prevented);
});

test("actions are looked up as own properties only", () => {
  const d = doc(el("button", "x", {}, ["x"], { on: { press: "constructor" } }));
  assert.doesNotThrow(() => (propsOf(tree(d, { actions: {} }), "x")["onClick"] as () => void)());
});

test("keyboard activates pressable non-buttons", () => {
  const calls: string[] = [];
  const d = doc(el("list", "l", {}, [el("item", "i", {}, ["x"], { on: { press: "a.b" } })]));
  const onKeyDown = propsOf(tree(d, { actions: { "a.b": (e) => calls.push(e.id) } }), "i")[
    "onKeyDown"
  ] as (e: unknown) => void;
  const key = (k: string) => {
    const target = {};
    onKeyDown({ key: k, target, currentTarget: target, preventDefault() {} });
  };
  key("Enter");
  key(" ");
  key("a");
  assert.deepEqual(calls, ["i", "i"]);
});

test("writable props are controlled and report writes through onChange", () => {
  const writes: [string, unknown][] = [];
  const events: string[] = [];
  const onChange = (path: string, value: unknown) => writes.push([path, value]);
  const actions = { "x.change": (e: ActionEvent) => events.push(e.id) };
  const d = doc(
    el("field", "f", { label: "Name", value: b("$.name") }, [], { on: { change: "x.change" } }),
    el("checkbox", "c", { label: "On", checked: b("$.on") }),
    el(
      "radio-group",
      "g",
      { label: "G", value: b("$.plan") },
      [el("radio", "r", { value: "pro" }, ["Pro"])],
      { on: { change: "x.change" } },
    ),
    el("select", "s", { label: "S", value: b("$.size") }, [
      el("option", "o", { value: b("$.sizes.0") }, ["Small"]),
    ]),
    el("tabs", "t", { selected: b("$.tab") }, [
      el("tab", "a", { label: "A" }),
      el("tab", "bb", { label: "B" }),
    ]),
    el("dialog", "d", { label: "D", open: b("$.open") }, [], { on: { close: "x.change" } }),
    el("list", "l", {}, [
      el("each", "e", { in: b("$.rows"), as: "row" }, [
        el("item", "i", {}, [el("checkbox", "rc", { label: "Done", checked: b("$row.done") })]),
      ]),
    ]),
    el("checkbox", "ro", { label: "Read only", checked: nb("$.on") }),
  );
  const data = {
    name: "Ana",
    on: false,
    plan: "free",
    size: "",
    sizes: [7],
    tab: "a",
    open: true,
    rows: [{ done: false }, { done: true }],
  };
  const t = tree(d, { data, onChange, actions });
  const field = propsOf(t, "f");
  assert.equal(field["value"], "Ana");
  (field["onChange"] as (e: unknown) => void)({ currentTarget: { value: "Bo" } });
  const box = propsOf(t, "c");
  assert.equal(box["checked"], false);
  (box["onChange"] as (e: unknown) => void)({ currentTarget: { checked: true } });
  (propsOf(t, "r")["onChange"] as () => void)();
  (propsOf(t, "s")["onChange"] as (e: unknown) => void)({ currentTarget: { value: "7" } });
  (propsOf(t, "bb")["onClick"] as () => void)();
  (propsOf(t, "d")["onKeyDown"] as (e: unknown) => void)({ key: "Escape", preventDefault() {} });
  (propsOf(t, "rc[1]")["onChange"] as (e: unknown) => void)({ currentTarget: { checked: false } });
  (propsOf(t, "ro")["onChange"] as (e: unknown) => void)({ currentTarget: { checked: false } });
  assert.deepEqual(writes, [
    ["$.name", "Bo"],
    ["$.on", true],
    ["$.plan", "pro"],
    ["$.size", 7],
    ["$.tab", "bb"],
    ["$.open", false],
    ["$.rows.1.done", false],
  ]);
  assert.deepEqual(events, ["f", "g", "d"]);
});

test("literal values of writable props are uncontrolled defaults", () => {
  const t = tree(
    doc(
      el("field", "f", { label: "N", value: "Hi" }),
      el("checkbox", "c", { label: "C", checked: true }),
    ),
  );
  assert.equal(propsOf(t, "f")["defaultValue"], "Hi");
  assert.equal(propsOf(t, "f")["value"], undefined);
  assert.equal(propsOf(t, "c")["defaultChecked"], true);
});

test("safeUrl keeps http, https, mailto and relative URLs and drops everything else", () => {
  for (const ok of [
    "https://a.test/x",
    "http://a.test",
    "mailto:a@b.c",
    "/x",
    "x/y",
    "?q=1",
    "#top",
    "//cdn.test/a",
    "a:b/c".replace("a:b", "./a:b"),
  ]) {
    assert.notEqual(safeUrl(ok), undefined, ok);
  }
  for (const bad of [
    "javascript:alert(1)",
    "JavaScript:alert(1)",
    " javascript:x",
    "java\tscript:x",
    "java\nscript:x",
    "data:text/html,x",
    "vbscript:x",
    "file:///etc/passwd",
    "",
    "  ",
  ]) {
    assert.equal(safeUrl(bad), undefined, JSON.stringify(bad));
  }
});

test("unsafe link and image URLs are dropped from the markup", () => {
  const out = html(
    doc(
      el("link", "a", { href: "javascript:alert(1)" }, ["x"]),
      el("link", "c", { href: b("$.u") }, ["y"]),
      el("image", "i", { src: "data:image/svg+xml,<svg onload=alert(1)>", label: "I" }),
    ),
    { data: { u: "jav&#x09;ascript:alert(1)".replace("&#x09;", "\t") } },
  );
  assert.doesNotMatch(out, /javascript|data:|href=|src=/i);
});

test("text is escaped, never injected as markup", () => {
  const out = html(
    doc(
      el("text", "t", {}, ["<script>alert(1)</script>"]),
      el("heading", "h", { level: 1, value: b("$.x") }),
    ),
    {
      data: { x: "<img src=x onerror=alert(1)>" },
    },
  );
  assert.doesNotMatch(out, /<script|<img/);
  assert.match(out, /&lt;script&gt;/);
});

test("extension and unknown elements render their children in a container with their fallback role", () => {
  const v = dom(
    doc(
      el("x-acme-rating", "r", { role: "group", label: "Rating", "x-acme-stars": "4" }, [
        "Four",
        el("button", "b", {}, ["Rate"]),
      ]),
      el("x-acme-note", "n", { role: "note" }, ["Saved"]),
      el("x-acme-bad", "bad", { role: "javascript:alert(1)" }, ["Bad"]),
      el("carousel", "c", { label: "Featured" }, ["Slide"], { slots: { extra: ["More"] } }),
      el("button", "btn", { role: "link" }, ["Catalog role wins"]),
    ),
  );
  assert.equal(v.byId("r").attribs["role"], "group");
  assert.equal(v.byId("r").attribs["aria-label"], "Rating");
  assert.equal(v.byId("r").attribs["x-acme-stars"], undefined, "unknown attributes are ignored");
  assert.equal(v.byId("b").name, "button");
  assert.equal(v.byId("n").attribs["role"], "note");
  assert.equal(v.byId("bad").attribs["role"], "group");
  assert.equal(v.byId("c").attribs["role"], "group");
  assert.equal(v.text(v.byId("c")), "SlideMore");
  assert.equal(v.byId("btn").attribs["role"], undefined);
});

test("a kind missing from the supplied catalog is treated as unknown", () => {
  const catalog = { ...coreCatalog, components: { screen: coreCatalog.components["screen"]! } };
  const v = dom(doc(el("button", "b", {}, ["x"])), { catalog });
  assert.equal(v.byId("b").name, "div");
  assert.equal(v.byId("b").attribs["role"], "group");
});

const json = fc.letrec((tie) => ({
  value: fc.oneof(
    { depthSize: "small" },
    fc.string(),
    fc.integer(),
    fc.boolean(),
    fc.constant(null),
    fc.record({ bind: fc.constantFrom("$.a", "$x", "$", "bad", "$.a.0") }),
    fc.record({ token: fc.string() }),
    fc.array(tie("value"), { maxLength: 3 }),
    fc.dictionary(
      fc.constantFrom(
        "kind",
        "id",
        "props",
        "children",
        "slots",
        "on",
        "label",
        "in",
        "as",
        "value",
        "selected",
        "open",
        "level",
        "href",
      ),
      tie("value"),
      { maxKeys: 5 },
    ),
  ),
}));
const kinds = fc.constantFrom(
  ...Object.keys(coreCatalog.components),
  "each",
  "x-a-b",
  "slot",
  "unknown",
);
const node: fc.Arbitrary<unknown> = fc.letrec((tie) => ({
  node: fc.record(
    {
      kind: kinds,
      id: fc.oneof(fc.string(), json.value),
      props: fc.dictionary(
        fc.constantFrom(
          "label",
          "value",
          "in",
          "as",
          "hidden",
          "selected",
          "open",
          "level",
          "columns",
          "gap",
          "role",
          "checked",
          "href",
          "src",
          "type",
        ),
        json.value,
        { maxKeys: 4 },
      ),
      on: fc.oneof(json.value, fc.dictionary(fc.string(), fc.string(), { maxKeys: 2 })),
      children: fc.array(fc.oneof({ depthSize: "small" }, fc.string(), tie("node"), json.value), {
        maxLength: 4,
      }),
      slots: fc.oneof(
        json.value,
        fc.dictionary(fc.string(), fc.array(tie("node"), { maxLength: 2 }), { maxKeys: 2 }),
      ),
    },
    { requiredKeys: ["kind"] },
  ),
})).node;

test("rendering and expectedTree never throw on arbitrary documents", () => {
  fc.assert(
    fc.property(fc.oneof(node, json.value), json.value, (root, data) => {
      const d = { weft: "0.1", root } as unknown as Document;
      html(d, { data });
      expectedTree(d, { catalog: coreCatalog, data });
    }),
    { numRuns: 500 },
  );
  for (const bad of [
    null,
    undefined,
    1,
    "x",
    {},
    { root: null },
    { root: [] },
  ] as unknown as Document[]) {
    html(bad);
    expectedTree(bad, { catalog: coreCatalog });
  }
});

test("deeply nested documents do not overflow the stack", () => {
  let n = el("stack", "s0");
  for (let i = 1; i < 5000; i++) n = el("stack", `s${i}`, {}, [n]);
  assert.doesNotThrow(() => html(doc(n)));
});
