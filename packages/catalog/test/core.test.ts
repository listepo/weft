import assert from "node:assert/strict";
import { test } from "vitest";
import { CatalogSchema, type PropDef } from "@weft/core";
import { coreCatalog } from "../src/core.ts";

type Row = {
  role: string;
  content: string;
  children?: string[];
  parents?: string[];
  label?: true;
  // prop name -> required flag; enum values are checked separately.
  required?: string[];
  enums?: Record<string, string[]>;
  slots?: string[];
  states?: string[];
  events?: string[];
};

// Encoded by hand from SPEC §5.1 so that the test is an independent check of the data.
const expected: Record<string, Row> = {
  screen: {
    role: "main",
    content: "nodes",
    required: ["weft"],
    states: ["ready", "loading", "error"],
  },
  stack: {
    role: "none",
    content: "nodes",
    enums: { direction: ["column", "row"], align: ["start", "center", "end", "stretch"] },
  },
  grid: { role: "none", content: "nodes", required: ["columns"] },
  section: { role: "region", content: "nodes", label: true, slots: ["header"] },
  heading: { role: "heading", content: "text", required: ["level"] },
  text: {
    role: "none",
    content: "text",
    enums: { tone: ["default", "muted", "success", "warning", "danger"] },
  },
  image: { role: "img", content: "none", label: true, required: ["src"] },
  model: { role: "img", content: "none", label: true, required: ["src", "fallback"] },
  link: { role: "link", content: "text", events: ["press"] },
  button: {
    role: "button",
    content: "text",
    enums: { variant: ["primary", "secondary", "danger"] },
    states: ["idle", "busy"],
    events: ["press"],
  },
  form: {
    role: "form",
    content: "nodes",
    slots: ["footer"],
    states: ["idle", "submitting", "invalid"],
    events: ["submit"],
  },
  field: {
    role: "textbox",
    content: "none",
    label: true,
    enums: { type: ["text", "email", "password", "number", "search", "multiline"] },
    states: ["valid", "invalid"],
    events: ["change"],
  },
  checkbox: { role: "checkbox", content: "none", label: true, events: ["change"] },
  switch: { role: "switch", content: "none", label: true, events: ["change"] },
  "radio-group": {
    role: "radiogroup",
    content: "nodes",
    children: ["radio", "each"],
    label: true,
    events: ["change"],
  },
  radio: { role: "radio", content: "text", parents: ["radio-group"], required: ["value"] },
  select: {
    role: "combobox",
    content: "nodes",
    children: ["option", "each"],
    label: true,
    events: ["change"],
  },
  option: {
    role: "option",
    content: "text",
    parents: ["select", "combobox"],
    required: ["value"],
  },
  combobox: {
    role: "combobox",
    content: "nodes",
    children: ["option", "each"],
    label: true,
    events: ["change"],
  },
  slider: { role: "slider", content: "none", label: true, events: ["change"] },
  stepper: { role: "spinbutton", content: "none", label: true, events: ["change"] },
  "date-picker": {
    role: "textbox",
    content: "none",
    label: true,
    enums: { type: ["date", "time", "datetime"] },
    events: ["change"],
  },
  "color-picker": { role: "textbox", content: "none", label: true, events: ["change"] },
  "segmented-control": {
    role: "radiogroup",
    content: "nodes",
    children: ["segment", "each"],
    label: true,
    events: ["change"],
  },
  segment: {
    role: "radio",
    content: "text",
    parents: ["segmented-control"],
    required: ["value"],
  },
  list: {
    role: "list",
    content: "nodes",
    children: ["item", "each"],
    slots: ["empty"],
    states: ["ready", "loading", "empty"],
  },
  item: { role: "listitem", content: "mixed", parents: ["list"], events: ["press"] },
  table: {
    role: "table",
    content: "nodes",
    children: ["column", "row", "each"],
    label: true,
    slots: ["empty"],
    states: ["ready", "loading", "empty"],
  },
  column: {
    role: "columnheader",
    content: "text",
    parents: ["table"],
    enums: { sort: ["none", "ascending", "descending"] },
    events: ["press"],
  },
  row: { role: "row", content: "nodes", children: ["cell"], parents: ["table"], events: ["press"] },
  cell: { role: "cell", content: "mixed", parents: ["row"] },
  tabs: { role: "tablist", content: "nodes", children: ["tab"], events: ["change"] },
  tab: { role: "tab", content: "nodes", parents: ["tabs"], label: true },
  dialog: { role: "dialog", content: "nodes", label: true, slots: ["actions"], events: ["close"] },
  alert: {
    role: "alert",
    content: "mixed",
    enums: { tone: ["info", "success", "warning", "danger"] },
  },
  menu: { role: "menu", content: "nodes", children: ["menu-item", "each"], label: true },
  "menu-item": { role: "menuitem", content: "text", parents: ["menu"], events: ["press"] },
};

const propTypes: Record<string, string> = {
  "screen.weft": "string",
  "stack.wrap": "boolean",
  "stack.gap": "token",
  "grid.columns": "number",
  "grid.gap": "token",
  "heading.level": "number",
  "button.disabled": "boolean",
  "button.submit": "boolean",
  "field.required": "boolean",
  "field.disabled": "boolean",
  "checkbox.checked": "boolean",
  "switch.checked": "boolean",
  "list.ordered": "boolean",
  "row.selected": "boolean",
  "tabs.selected": "string",
  "dialog.modal": "boolean",
  "dialog.open": "boolean",
  "menu-item.disabled": "boolean",
  "slider.value": "number",
  "slider.min": "number",
  "slider.max": "number",
  "slider.step": "number",
  "stepper.value": "number",
  "stepper.min": "number",
  "stepper.max": "number",
  "stepper.step": "number",
  "date-picker.value": "string",
  "date-picker.min": "string",
  "date-picker.max": "string",
  "color-picker.value": "string",
  "combobox.value": "string",
  "combobox.placeholder": "string",
  "segmented-control.value": "string",
};

const writable = [
  "field.value",
  "checkbox.checked",
  "switch.checked",
  "radio-group.value",
  "select.value",
  "tabs.selected",
  "dialog.open",
  "slider.value",
  "stepper.value",
  "date-picker.value",
  "color-picker.value",
  "combobox.value",
  "segmented-control.value",
];

const defaults: Record<string, string | boolean | number> = {
  "stack.direction": "column",
  "button.variant": "secondary",
  "button.submit": false,
  "field.type": "text",
  "dialog.modal": true,
  "alert.tone": "info",
  "slider.min": 0,
  "slider.max": 100,
  "slider.step": 1,
  "stepper.step": 1,
  "date-picker.type": "date",
};

const sorted = (a: readonly string[] | undefined) => [...(a ?? [])].sort();

test("catalog parses under CatalogSchema", () => {
  assert.deepEqual(CatalogSchema.safeParse(coreCatalog).error, undefined);
  assert.equal(coreCatalog.name, "weft-core");
  assert.equal(coreCatalog.weft, "0.2");
});

test("catalog has exactly the SPEC §5.1 kinds", () => {
  assert.deepEqual(sorted(Object.keys(coreCatalog.components)), sorted(Object.keys(expected)));
  assert.ok(!("each" in coreCatalog.components) && !("slot" in coreCatalog.components));
});

for (const [kind, row] of Object.entries(expected)) {
  test(`${kind} matches SPEC §5.1`, () => {
    const def = coreCatalog.components[kind];
    assert.ok(def, `missing ${kind}`);
    assert.equal(def.role, row.role);
    assert.equal(def.content, row.content);
    assert.deepEqual(def.allowedChildren, row.children);
    assert.deepEqual(def.allowedParents, row.parents);
    assert.equal(def.requiresLabel, row.label);
    assert.deepEqual(sorted(def.states), sorted(row.states));
    assert.deepEqual(sorted(def.events), sorted(row.events));
    assert.deepEqual(sorted(Object.keys(def.slots ?? {})), sorted(row.slots));
    assert.deepEqual(
      sorted(
        Object.entries(def.props ?? {})
          .filter(([, p]) => p.required)
          .map(([n]) => n),
      ),
      sorted(row.required),
    );
    for (const [prop, values] of Object.entries(row.enums ?? {})) {
      assert.deepEqual(def.props?.[prop]?.values, values, `${kind}.${prop}`);
    }
    assert.ok(def.description.length > 0);
  });
}

test("prop types, writable flags and defaults match SPEC §5.1", () => {
  const prop = (key: string): PropDef | undefined => {
    const [kind, name] = key.split(".") as [string, string];
    return coreCatalog.components[kind]?.props?.[name];
  };
  for (const [key, type] of Object.entries(propTypes)) assert.equal(prop(key)?.type, type, key);
  for (const key of writable) assert.equal(prop(key)?.writable, true, key);
  for (const [key, value] of Object.entries(defaults)) assert.equal(prop(key)?.default, value, key);
  for (const [kind, def] of Object.entries(coreCatalog.components)) {
    for (const [name, p] of Object.entries(def.props ?? {})) {
      if (p.writable)
        assert.ok(writable.includes(`${kind}.${name}`), `${kind}.${name} not writable in SPEC`);
    }
  }
});

test("numeric ranges match SPEC §5.1", () => {
  const range = (kind: string, name: string) => {
    const p = coreCatalog.components[kind]?.props?.[name];
    return { integer: p?.integer, min: p?.min, max: p?.max };
  };
  assert.deepEqual(range("heading", "level"), { integer: true, min: 1, max: 6 });
  assert.deepEqual(range("grid", "columns"), { integer: true, min: 1, max: undefined });
});

test("enum props have values, token props have tokenType, everything is described", () => {
  for (const [kind, def] of Object.entries(coreCatalog.components)) {
    assert.ok(def.description.endsWith("."), `${kind} description`);
    for (const [name, p] of Object.entries(def.props ?? {})) {
      const at = `${kind}.${name}`;
      assert.ok(p.description.length > 0, `${at} description`);
      if (p.type === "enum") assert.ok(p.values && p.values.length > 0, `${at} values`);
      else assert.equal(p.values, undefined, `${at} stray values`);
      if (p.type !== "number")
        assert.ok(p.min === undefined && p.max === undefined && p.integer === undefined, at);
      if (p.type === "token") assert.ok(p.tokenType, `${at} tokenType`);
      else assert.equal(p.tokenType, undefined, `${at} stray tokenType`);
      if (p.default !== undefined && p.type === "enum")
        assert.ok(p.values?.includes(String(p.default)));
    }
    for (const [name, s] of Object.entries(def.slots ?? {})) {
      assert.ok(s.description.length > 0, `${kind} slot ${name}`);
    }
  }
});

test("exactly the text and mixed kinds declare the bindable string prop `text`", () => {
  for (const [kind, def] of Object.entries(coreCatalog.components)) {
    const text = def.props?.["text"];
    if (def.content === "text" || def.content === "mixed") {
      assert.equal(text?.type, "string", kind);
      assert.notEqual(text?.bindable, false, kind);
    } else assert.equal(text, undefined, kind);
  }
});

test("every kind named by allowedChildren and allowedParents exists", () => {
  const known = new Set([...Object.keys(coreCatalog.components), "each"]);
  for (const def of Object.values(coreCatalog.components)) {
    for (const k of [...(def.allowedChildren ?? []), ...(def.allowedParents ?? [])]) {
      assert.ok(known.has(k), k);
    }
  }
});
