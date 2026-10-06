// One server-render test per `weft-core` kind: element, role, accessible name and states.
import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { b, doc, dom, el, html } from "./helpers.ts";

test("every catalog kind is covered by this file", () => {
  const covered = [
    "screen",
    "stack",
    "grid",
    "section",
    "heading",
    "text",
    "image",
    "model",
    "link",
    "button",
    "form",
    "field",
    "checkbox",
    "switch",
    "radio-group",
    "radio",
    "select",
    "option",
    "list",
    "item",
    "table",
    "column",
    "row",
    "cell",
    "tabs",
    "tab",
    "dialog",
    "alert",
    "menu",
    "menu-item",
    "combobox",
    "slider",
    "stepper",
    "date-picker",
    "color-picker",
    "segmented-control",
    "segment",
  ];
  assert.deepEqual([...covered].sort(), Object.keys(coreCatalog.components).sort());
});

test("screen is a named main landmark with its state", () => {
  const d = doc();
  d.root.props = { label: "Home", state: "loading" };
  const main = dom(d).byId("root");
  assert.equal(main.name, "main");
  assert.equal(main.attribs["aria-label"], "Home");
  assert.equal(main.attribs["aria-busy"], "true");
  assert.equal(main.attribs["data-state"], "loading");
});

test("stack and grid are plain layout containers styled from tokens", () => {
  const { tokens } = loadTokens({
    space: { $type: "dimension", md: { $value: { value: 16, unit: "px" } } },
  });
  const d = doc(
    el("stack", "s", {
      direction: "row",
      gap: { token: "space.md" },
      align: "center",
      wrap: true,
      label: "x",
    }),
    el("grid", "g", { columns: 3, gap: { token: "space.unknown" } }),
  );
  const v = dom(d, { tokens });
  const s = v.byId("s");
  assert.equal(s.name, "div");
  assert.equal(s.attribs["role"], undefined);
  assert.equal(s.attribs["aria-label"], undefined, "role none takes no name");
  assert.equal(
    s.attribs["style"],
    "gap:16px;display:flex;flex-direction:row;align-items:center;flex-wrap:wrap",
  );
  assert.equal(
    v.byId("g").attribs["style"],
    "gap:var(--weft-space-unknown);display:grid;grid-template-columns:repeat(3, minmax(0, 1fr))",
  );
});

test("a row without align centres its children, a column and an explicit align do not change", () => {
  const d = doc(
    el("stack", "row", { direction: "row" }),
    el("stack", "end", { direction: "row", align: "end" }),
    el("stack", "bad", { direction: "row", align: "middle" }),
    el("stack", "column", {}),
  );
  const v = dom(d);
  assert.equal(
    v.byId("row").attribs["style"],
    "display:flex;flex-direction:row;align-items:center",
  );
  assert.equal(
    v.byId("end").attribs["style"],
    "display:flex;flex-direction:row;align-items:flex-end",
  );
  assert.equal(
    v.byId("bad").attribs["style"],
    "display:flex;flex-direction:row;align-items:center",
  );
  assert.equal(v.byId("column").attribs["style"], "display:flex;flex-direction:column");
});

test("section is a region only when labelled, with its header slot first", () => {
  const d = doc(
    el("section", "s", { label: "Profile" }, [el("text", "body", {}, ["Body"])], {
      slots: { header: [el("heading", "h", { level: 2 }, ["Title"])] },
    }),
  );
  const out = html(d);
  assert.match(
    out,
    /<section data-weft-id="s" aria-label="Profile"><div data-weft-slot="header"><h2 data-weft-id="h">Title<\/h2><\/div><div data-weft-id="body">Body<\/div><\/section>/,
  );
});

test("heading maps level to h1-h6 and takes text from content or the text prop", () => {
  const v = dom(
    doc(
      el("heading", "a", { level: 1 }, ["One"]),
      el("heading", "c", { level: 3, text: b("$.t") }),
      el("heading", "bad", { level: "x" }, ["x"]),
      el("heading", "high", { level: b("$.high") }, ["x"]),
      el("heading", "low", { level: b("$.low") }, ["x"]),
    ),
    { data: { t: "Three", high: 9, low: 0.2 } },
  );
  assert.equal(v.byId("a").name, "h1");
  assert.equal(v.byId("c").name, "h3");
  assert.equal(v.text(v.byId("c")), "Three");
  assert.equal(v.byId("bad").name, "h2", "a level that is no number falls back to ARIA's 2");
  assert.equal(v.byId("high").name, "h6", "bound numbers are clamped to the declared range");
  assert.equal(v.byId("low").name, "h1");
});

test("text is a generic run of text with its tone", () => {
  const t = dom(doc(el("text", "t", { tone: "muted" }, ["Hi"]))).byId("t");
  assert.equal(t.name, "div");
  assert.equal(t.attribs["data-tone"], "muted");
});

test("image is an img named by alt", () => {
  const i = dom(doc(el("image", "i", { src: "https://x.test/a.png", label: "Logo" }))).byId("i");
  assert.equal(i.name, "img");
  assert.equal(i.attribs["alt"], "Logo");
  assert.equal(i.attribs["src"], "https://x.test/a.png");
});

test("link keeps a safe href; without one it keeps the link role and focus", () => {
  const v = dom(
    doc(
      el("link", "a", { href: "/help" }, ["Help"]),
      el("link", "b", {}, ["Reset"], { on: { press: "nav.reset" } }),
    ),
  );
  assert.equal(v.byId("a").attribs["href"], "/help");
  assert.equal(v.byId("a").attribs["role"], undefined);
  assert.equal(v.byId("b").attribs["role"], "link");
  assert.equal(v.byId("b").attribs["tabindex"], "0");
});

test("button is a native button with disabled and busy state; submit buttons submit", () => {
  const btn = dom(
    doc(el("button", "go", { variant: "primary", disabled: true, state: "busy" }, ["Go"])),
  ).byId("go");
  assert.equal(btn.name, "button");
  assert.equal(btn.attribs["type"], "button");
  assert.equal(btn.attribs["disabled"], "");
  assert.equal(btn.attribs["aria-busy"], "true");
  assert.equal(btn.attribs["data-variant"], "primary");
  const v = dom(
    doc(
      el("form", "f", {}, [
        el("button", "s", { submit: true }, ["Send"]),
        el("button", "bound", { submit: b("$.yes") }, ["Bound"]),
      ]),
    ),
    { data: { yes: true } },
  );
  assert.equal(v.byId("s").attribs["type"], "submit");
  assert.equal(v.byId("bound").attribs["type"], "button", "submit is literal-only");
});

test("form is named by label, footer after the body, submitting is busy", () => {
  const out = html(
    doc(
      el("form", "f", { label: "Login", state: "submitting" }, [el("text", "t", {}, ["Body"])], {
        slots: { footer: [el("button", "go", {}, ["Go"])] },
      }),
    ),
  );
  assert.match(
    out,
    /<form data-weft-id="f" data-state="submitting" aria-busy="true" aria-label="Login"><div data-weft-id="t">Body<\/div><div data-weft-slot="footer"><button/,
  );
});

test("field refines its role from type and maps required, disabled, invalid and error", () => {
  const v = dom(
    doc(
      el("field", "e", {
        label: "Email",
        type: "email",
        required: true,
        error: "Bad",
        value: "a@b.c",
      }),
      el("field", "n", { label: "Age", type: "number" }),
      el("field", "s", { label: "Find", type: "search", disabled: true }),
      el("field", "m", { label: "Note", type: "multiline", state: "invalid" }),
    ),
  );
  const e = v.byId("e");
  assert.equal(e.name, "input");
  assert.equal(e.attribs["type"], "email");
  assert.equal(e.attribs["aria-label"], "Email");
  assert.equal(e.attribs["required"], "");
  assert.equal(e.attribs["aria-invalid"], "true");
  const err = v.find((x) => x.attribs["id"] === e.attribs["aria-describedby"]);
  assert.ok(err);
  assert.equal(v.text(err), "Bad");
  assert.equal(v.byId("n").attribs["type"], "number");
  assert.equal(v.byId("s").attribs["type"], "search");
  assert.equal(v.byId("s").attribs["disabled"], "");
  assert.equal(v.byId("m").name, "textarea");
  assert.equal(v.byId("m").attribs["aria-invalid"], "true");
});

test("checkbox and switch are checkbox inputs, the switch with role switch", () => {
  const v = dom(
    doc(
      el("checkbox", "c", { label: "Accept", checked: true }),
      el("switch", "s", { label: "Push", disabled: true }),
    ),
  );
  assert.equal(v.byId("c").attribs["type"], "checkbox");
  assert.equal(v.byId("c").attribs["checked"], "");
  assert.equal(v.byId("c").attribs["aria-label"], "Accept");
  assert.equal(v.byId("s").attribs["role"], "switch");
  assert.equal(v.byId("s").attribs["disabled"], "");
  assert.equal(v.byId("s").attribs["checked"], undefined);
});

test("radio-group is a radiogroup whose value checks one radio", () => {
  const v = dom(
    doc(
      el("radio-group", "g", { label: "Plan", value: "pro" }, [
        el("radio", "r1", { value: "free" }, ["Free"]),
        el("radio", "r2", { value: "pro", disabled: true }, ["Pro"]),
      ]),
    ),
  );
  assert.equal(v.byId("g").attribs["role"], "radiogroup");
  assert.equal(v.byId("g").attribs["aria-label"], "Plan");
  const caption = v.find((e) => e.attribs["aria-hidden"] === "true");
  assert.ok(caption && v.text(caption) === "Plan", "the label is also the visible caption");
  const [r1, r2] = [v.byId("r1"), v.byId("r2")];
  assert.equal(r1.attribs["type"], "radio");
  assert.equal(r1.attribs["aria-label"], "Free");
  assert.equal(r1.attribs["name"], r2.attribs["name"]);
  assert.equal(r1.attribs["checked"], undefined);
  assert.equal(r2.attribs["checked"], "");
  assert.equal(r2.attribs["disabled"], "");
});

test("select is a combobox of options with the bound value selected", () => {
  const v = dom(
    doc(
      el("select", "s", { label: "Country", value: b("$.c") }, [
        el("option", "pt", { value: "pt" }, ["Portugal"]),
        el("option", "es", { value: "es" }, ["Spain"]),
      ]),
    ),
    { data: { c: "es" } },
  );
  assert.equal(v.byId("s").name, "select");
  assert.equal(v.byId("s").attribs["aria-label"], "Country");
  const caption = v.find((e) => e.attribs["aria-hidden"] === "true");
  assert.ok(caption && v.text(caption) === "Country", "the label is also the visible caption");
  assert.equal(v.byId("pt").name, "option");
  assert.equal(v.byId("pt").attribs["selected"], undefined);
  assert.equal(v.byId("es").attribs["selected"], "");
});

test("list and item are ul/ol and li; loading is busy; pressable items are focusable", () => {
  const v = dom(
    doc(
      el("list", "l", { ordered: true, state: "loading" }, [
        el("item", "i", {}, ["One"], { on: { press: "a.b" } }),
      ]),
      el("list", "u"),
    ),
  );
  assert.equal(v.byId("l").name, "ol");
  assert.equal(v.byId("l").attribs["aria-busy"], "true");
  assert.equal(v.byId("u").name, "ul");
  assert.equal(v.byId("i").name, "li");
  assert.equal(v.byId("i").attribs["tabindex"], "0");
});

test("table puts columns in a header row and maps sort and selection", () => {
  const out = html(
    doc(
      el("table", "t", { label: "Users" }, [
        el("column", "c1", { sort: "ascending" }, ["Name"]),
        el("row", "r1", { selected: true }, [el("cell", "x", {}, ["Ana"])]),
        el("column", "c2", {}, ["Role"]),
      ]),
    ),
  );
  assert.equal(
    out,
    '<main data-weft-id="root" aria-label="Test"><table data-weft-id="t" aria-label="Users"><thead><tr>' +
      '<th data-weft-id="c1" scope="col" aria-sort="ascending">Name</th><th data-weft-id="c2" scope="col">Role</th></tr></thead>' +
      '<tbody><tr data-weft-id="r1" aria-selected="true"><td data-weft-id="x">Ana</td></tr></tbody></table></main>',
  );
});

test("tabs emit a tablist of tabs and one tabpanel per tab, only the selected one shown", () => {
  const v = dom(
    doc(
      el("tabs", "ts", { selected: "b", label: "Sections" }, [
        el("tab", "a", { label: "A" }, ["Panel A"]),
        el("tab", "b", { label: "B" }, ["Panel B"]),
      ]),
    ),
  );
  const list = v.byId("ts");
  assert.equal(list.attribs["role"], "tablist");
  assert.equal(list.attribs["aria-label"], "Sections");
  const [a, bTab] = [v.byId("a"), v.byId("b")];
  assert.equal(a.attribs["role"], "tab");
  assert.equal(a.attribs["aria-selected"], "false");
  assert.equal(a.attribs["tabindex"], "-1");
  assert.equal(bTab.attribs["aria-selected"], "true");
  assert.equal(bTab.attribs["tabindex"], "0");
  const panels = v.all((e) => e.attribs["role"] === "tabpanel");
  assert.equal(panels.length, 2);
  for (const [tab, panel] of [
    [a, panels[0]!],
    [bTab, panels[1]!],
  ] as const) {
    assert.equal(tab.attribs["aria-controls"], panel.attribs["id"]);
    assert.equal(panel.attribs["aria-labelledby"], tab.attribs["id"]);
  }
  assert.equal(panels[0]!.attribs["hidden"], "");
  assert.equal(panels[1]!.attribs["hidden"], undefined);
});

test("dialog renders only when open, modal by default", () => {
  const open = dom(
    doc(
      el("dialog", "d", { label: "Sure?", open: true }, ["Body"], {
        slots: { actions: [el("button", "ok", {}, ["OK"])] },
      }),
    ),
  ).byId("d");
  assert.equal(open.name, "dialog");
  assert.equal(open.attribs["open"], "");
  assert.equal(open.attribs["aria-modal"], "true");
  assert.equal(open.attribs["aria-label"], "Sure?");
  const modeless = dom(doc(el("dialog", "d", { label: "x", open: true, modal: false }))).byId("d");
  assert.equal(modeless.attribs["aria-modal"], undefined);
  assert.equal(dom(doc(el("dialog", "d", { label: "x", open: false }))).has("d"), false);
  assert.equal(dom(doc(el("dialog", "d", { label: "x" }))).has("d"), false);
});

test("alert has role alert and its tone", () => {
  const a = dom(doc(el("alert", "a", { tone: "danger" }, ["Failed"]))).byId("a");
  assert.equal(a.attribs["role"], "alert");
  assert.equal(a.attribs["data-tone"], "danger");
});

test("menu and menu-item are a named menu of menuitem buttons", () => {
  const v = dom(
    doc(
      el("menu", "m", { label: "Actions" }, [el("menu-item", "x", { disabled: true }, ["Delete"])]),
    ),
  );
  assert.equal(v.byId("m").attribs["role"], "menu");
  assert.equal(v.byId("m").attribs["aria-label"], "Actions");
  assert.equal(v.byId("x").name, "button");
  assert.equal(v.byId("x").attribs["role"], "menuitem");
  assert.equal(v.byId("x").attribs["disabled"], "");
});

test("list and table show their empty slot in place of items and rows", () => {
  const items = [
    el("each", "e", { in: b("$.items"), as: "it" }, [el("item", "i", { text: b("$it") })]),
  ];
  const empty = { slots: { empty: [el("text", "none", {}, ["Nothing"])] } };
  const list = (data: unknown, props = {}) =>
    dom(doc(el("list", "l", props, items, empty)), { data });
  assert.equal(list({ items: [] }).has("none"), true);
  assert.equal(list({}).has("none"), true, "a missing array iterates nothing");
  assert.equal(list({ items: ["a"] }).has("none"), false);
  assert.equal(list({ items: ["a"] }, { state: "empty" }).has("i[0]"), false);
  assert.match(
    html(doc(el("list", "l", {}, items, empty)), { data: { items: [] } }),
    /<ul data-weft-id="l"><li role="none" data-weft-slot="empty"><div data-weft-id="none">Nothing<\/div><\/li><\/ul>/,
  );
  // A static item counts even when hidden: the rule reads the document, not what is visible.
  assert.equal(
    dom(doc(el("list", "l", {}, [el("item", "h", { hidden: true }, ["x"])], empty))).has("none"),
    false,
  );
  const table = html(
    doc(
      el(
        "table",
        "t",
        { label: "T" },
        [
          el("column", "c1", {}, ["A"]),
          el("column", "c2", {}, ["B"]),
          ...items.map((e) => ({ ...e, children: [el("row", "r")] })),
        ],
        empty,
      ),
    ),
    { data: { items: [] } },
  );
  assert.match(
    table,
    /<\/thead><tbody><tr data-weft-slot="empty"><td colSpan="2"><div data-weft-id="none">Nothing<\/div><\/td><\/tr><\/tbody>/,
  );
});

test("dialog actions sit in their own bar after the body", () => {
  const out = html(
    doc(
      el("dialog", "d", { label: "Sure?", open: true }, [el("text", "t", {}, ["Body"])], {
        slots: { actions: [el("button", "ok", {}, ["OK"])] },
      }),
    ),
  );
  assert.match(out, /Body<\/div><div data-weft-slot="actions"><button data-weft-id="ok"/);
});

test("text-bearing kinds take bound text from the text prop; label overrides the name", () => {
  const v = dom(
    doc(
      el("button", "b", { text: b("$.cta") }),
      el("button", "x", { label: "Close" }, ["×"]),
      el("list", "l", {}, [el("item", "i", { text: b("$.cta") })]),
      el("text", "both", { text: "prop" }, ["content"]),
    ),
    { data: { cta: "Buy" } },
  );
  assert.equal(v.text(v.byId("b")), "Buy");
  assert.equal(v.text(v.byId("i")), "Buy");
  assert.equal(v.byId("x").attribs["aria-label"], "Close");
  assert.equal(v.text(v.byId("x")), "×");
  assert.equal(v.text(v.byId("both")), "content", "content wins when a lenient reader gets both");
});

test("slider is a range input whose value is clamped and snapped to its grid", () => {
  const v = dom(
    doc(
      el("slider", "a", { label: "Volume", value: b("$.v"), min: 0, max: 10, step: 2 }),
      el("slider", "b", { label: "Backwards", value: 5, min: 8, max: 2 }),
    ),
    { data: { v: 3 } },
  );
  const a = v.byId("a");
  assert.equal(a.name, "input");
  assert.equal(a.attribs["type"], "range");
  assert.equal(a.attribs["aria-label"], "Volume");
  assert.deepEqual(
    [a.attribs["min"], a.attribs["max"], a.attribs["step"], a.attribs["value"]],
    ["0", "10", "2", "4"],
  );
  const caption = v.find((e) => e.attribs["aria-hidden"] === "true");
  assert.ok(caption && v.text(caption) === "Volume", "the label is also the visible caption");
  const odd = v.byId("b");
  assert.deepEqual([odd.attribs["min"], odd.attribs["max"], odd.attribs["value"]], ["8", "8", "8"]);
});

test("stepper is a number input with minus and plus buttons kept out of the tree", () => {
  const v = dom(
    doc(el("stepper", "q", { label: "Guests", value: b("$.n"), min: 1, max: 4, disabled: true })),
    { data: { n: 9 } },
  );
  const q = v.byId("q");
  assert.equal(q.attribs["type"], "number");
  assert.equal(q.attribs["aria-label"], "Guests");
  assert.deepEqual([q.attribs["min"], q.attribs["max"], q.attribs["step"]], ["1", "4", "1"]);
  assert.equal(q.attribs["value"], "4", "a bound value outside the range is clamped");
  assert.equal(q.attribs["disabled"], "");
  const buttons = v.all((e) => e.name === "button");
  assert.deepEqual(
    buttons.map((e) => v.text(e)),
    ["\u2212", "+"],
  );
  for (const e of buttons) {
    assert.equal(e.attribs["aria-hidden"], "true");
    assert.equal(e.attribs["tabindex"], "-1");
    assert.equal(e.attribs["disabled"], "");
  }
  const free = dom(doc(el("stepper", "q", { label: "Free" }))).byId("q");
  assert.equal(free.attribs["min"], undefined);
  assert.equal(free.attribs["max"], undefined);
  assert.equal(free.attribs["value"], "0");
});

test("date-picker maps its type to the input type and drops values that are not that type", () => {
  const v = dom(
    doc(
      el("date-picker", "d", { label: "Due", value: "2026-10-05", min: "2026-01-01", max: "bad" }),
      el("date-picker", "t", { label: "At", type: "time", value: "09:30" }),
      el("date-picker", "dt", { label: "When", type: "datetime", value: "2026-10-05T09:30" }),
      el("date-picker", "x", { label: "Bad", value: "2026-02-30" }),
    ),
  );
  const d = v.byId("d");
  assert.equal(d.attribs["type"], "date");
  assert.deepEqual(
    [d.attribs["value"], d.attribs["min"], d.attribs["max"]],
    ["2026-10-05", "2026-01-01", undefined],
  );
  assert.equal(v.byId("t").attribs["type"], "time");
  assert.equal(v.byId("dt").attribs["type"], "datetime-local");
  assert.equal(v.byId("dt").attribs["value"], "2026-10-05T09:30");
  assert.equal(v.byId("x").attribs["value"], "");
});

test("color-picker shows a #rrggbb colour and falls back to black", () => {
  const v = dom(
    doc(
      el("color-picker", "a", { label: "Accent", value: "#3B82F6" }),
      el("color-picker", "b", { label: "Other", value: "blue" }),
    ),
  );
  assert.equal(v.byId("a").attribs["type"], "color");
  assert.equal(v.byId("a").attribs["value"], "#3b82f6");
  assert.equal(v.byId("b").attribs["value"], "#000000");
});

test("combobox is a text input linked to a datalist of its options", () => {
  const v = dom(
    doc(
      el("combobox", "c", { label: "Fruit", value: b("$.f"), placeholder: "Pick" }, [
        el("option", "o1", { value: "apple" }, ["Apple"]),
        el("option", "o2", { value: "pear" }, ["Pear"]),
      ]),
    ),
    { data: { f: "app" } },
  );
  const c = v.byId("c");
  assert.equal(c.name, "input");
  assert.equal(c.attribs["aria-label"], "Fruit");
  assert.equal(c.attribs["value"], "app");
  assert.equal(c.attribs["placeholder"], "Pick");
  const list = v.find((e) => e.name === "datalist");
  assert.ok(list);
  assert.equal(c.attribs["list"], list.attribs["id"]);
  assert.deepEqual(
    v.all((e) => e.name === "option").map((e) => [e.attribs["value"], v.text(e)]),
    [
      ["apple", "Apple"],
      ["pear", "Pear"],
    ],
  );
});

test("segmented-control is a radiogroup of segments sharing one name", () => {
  const v = dom(
    doc(
      el("segmented-control", "g", { label: "View", value: "week" }, [
        el("segment", "s1", { value: "day" }, ["Day"]),
        el("segment", "s2", { value: "week" }, ["Week"]),
      ]),
    ),
  );
  assert.equal(v.byId("g").attribs["role"], "radiogroup");
  assert.equal(v.byId("g").attribs["data-weft-segmented"], "");
  const [s1, s2] = [v.byId("s1"), v.byId("s2")];
  assert.equal(s1.attribs["type"], "radio");
  assert.equal(s1.attribs["aria-label"], "Day");
  assert.equal(s1.attribs["name"], s2.attribs["name"]);
  assert.equal(s1.attribs["checked"], undefined);
  assert.equal(s2.attribs["checked"], "");
});

test("a tilt is a transform in the order the generators write, and a dialog takes none", () => {
  const tilted = doc(
    el("section", "card", { "rotate-y": 25, perspective: 800, "rotate-z": -6 }),
    el("text", "plain", {}, ["x"]),
    el("dialog", "d", { open: true, label: "D", "rotate-x": 10 }),
  );
  const out = html(tilted);
  assert.match(out, /transform:perspective\(800px\) rotateY\(25deg\) rotateZ\(-6deg\)/);
  assert.doesNotMatch(out, /rotateX/);
  assert.doesNotMatch(dom(tilted).byId("plain").attribs["style"] ?? "", /transform/);
});

test("a tilt keeps the layout style of the element", () => {
  const d = doc(el("stack", "s", { direction: "row", "rotate-x": 20 }));
  const style = dom(d).byId("s").attribs["style"] ?? "";
  assert.match(style, /flex-direction/);
  assert.match(style, /transform:rotateX\(20deg\)/);
});
