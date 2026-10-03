// One server-render test per `weft-core` kind: element, role, accessible name and states.
import assert from "node:assert/strict";
import { test } from "node:test";
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
  ];
  assert.deepEqual([...covered].sort(), Object.keys(coreCatalog.components).sort());
});

test("screen is a named main landmark with its state", () => {
  const d = doc();
  d.root.props = { weft: "0.1", label: "Home", state: "loading" };
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

test("section is a region only when labelled, with its header slot first", () => {
  const d = doc(
    el("section", "s", { label: "Profile" }, [el("text", "body", {}, ["Body"])], {
      slots: { header: [el("heading", "h", { level: 2 }, ["Title"])] },
    }),
  );
  const out = html(d);
  assert.match(
    out,
    /<section data-weft-id="s" aria-label="Profile"><h2 data-weft-id="h">Title<\/h2><div data-weft-id="body">Body<\/div><\/section>/,
  );
});

test("heading maps level to h1-h6 and takes text from content or value", () => {
  const v = dom(
    doc(
      el("heading", "a", { level: 1 }, ["One"]),
      el("heading", "c", { level: 3, value: b("$.t") }),
      el("heading", "bad", { level: 9 }, ["x"]),
    ),
    { data: { t: "Three" } },
  );
  assert.equal(v.byId("a").name, "h1");
  assert.equal(v.byId("c").name, "h3");
  assert.equal(v.text(v.byId("c")), "Three");
  assert.equal(v.byId("bad").name, "h2");
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

test("button is a native button with disabled and busy state", () => {
  const btn = dom(
    doc(el("button", "go", { variant: "primary", disabled: true, state: "busy" }, ["Go"])),
  ).byId("go");
  assert.equal(btn.name, "button");
  assert.equal(btn.attribs["type"], "button");
  assert.equal(btn.attribs["disabled"], "");
  assert.equal(btn.attribs["aria-busy"], "true");
  assert.equal(btn.attribs["data-variant"], "primary");
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
    /<form data-weft-id="f" data-state="submitting" aria-busy="true" aria-label="Login"><div data-weft-id="t">Body<\/div><button/,
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
