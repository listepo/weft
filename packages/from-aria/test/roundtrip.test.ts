// The importers' done criterion: every corpus screen, rendered and read back, keeps its
// structure, roles, states and (from DOM) ids. What an importer cannot recover is removed from
// both sides by `skeleton`, one documented loss at a time, instead of loosening assertions.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { beforeAll, describe, test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse, validate, type Document } from "@weft/core";
import {
  diffAria,
  expandRoot,
  expectedTree,
  ordered,
  prop,
  renderPage,
  text,
  type Inst,
} from "@weft/render-react";
import { fromAriaSnapshot, fromDom, instanceId, type ImportResult } from "../src/index.ts";

const corpus = new URL("../../../corpus/", import.meta.url);
const catalog = coreCatalog;

type Screen = { name: string; document: Document; data: unknown };

function screens(): Screen[] {
  return readdirSync(corpus, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => {
      const dir = new URL(`${e.name}/`, corpus);
      const parsed = parse(readFileSync(new URL("screen.weft", dir), "utf8"), { catalog });
      assert.ok(parsed.document);
      const data: unknown = JSON.parse(readFileSync(new URL("data.json", dir), "utf8"));
      return { name: e.name, document: parsed.document, data };
    });
}

type Mode = "dom" | "aria";
type Skel = {
  kind: string;
  id?: string;
  states: Record<string, string | number | boolean>;
  children: Skel[];
};

const nodes = (list: (Inst | string)[]): Inst[] =>
  list.filter((c): c is Inst => typeof c !== "string");
const flag = (n: Inst, name: string) => {
  const v = prop(n, name).value;
  return v !== "false" && Boolean(v);
};

// A role-`none` kind (`stack`, `grid`, `text`) and an unnamed form or section add no node to the
// accessibility tree, so a snapshot cannot show them (loss "layout"; SPEC §5.1 for unnamed
// landmarks); their children are compared in their place. Text itself is compared by the
// "same accessibility tree" tests.
function dissolved(n: Inst, mode: Mode): boolean {
  if (mode === "dom") return false;
  if (n.kind === "text") return true;
  if (n.kind === "stack" || n.kind === "grid") return true;
  return (n.kind === "form" || n.kind === "section") && text(n, "label").trim() === "";
}

// States both importers read back. A snapshot has no `data-state`, so only ARIA-level states
// are compared there (loss "props").
function states(n: Inst, mode: Mode): Skel["states"] {
  const s: Skel["states"] = {};
  const set = (k: string, v: string | number | boolean) => {
    if (v !== false && v !== "") s[k] = v;
  };
  if (n.kind === "field") set("invalid", text(n, "state") === "invalid" || text(n, "error") !== "");
  else if (mode === "dom") set("state", text(n, "state"));
  set("disabled", flag(n, "disabled"));
  if (n.kind === "checkbox" || n.kind === "switch") set("checked", flag(n, "checked"));
  if (n.kind === "row") set("selected", flag(n, "selected"));
  if (n.kind === "heading") set("level", Number(text(n, "level")) || 2);
  if (n.kind === "column" && mode === "dom") set("sort", text(n, "sort"));
  // Generated ids differ, so a choice among children is compared by position.
  if (n.kind === "tabs") {
    const tabs = nodes(n.children).filter((c) => c.kind === "tab");
    s["chosen"] = Math.max(
      0,
      tabs.findIndex((t) => t.docId === text(n, "selected") || t.id === text(n, "selected")),
    );
  }
  if (n.kind === "radio-group") {
    const v = text(n, "value");
    s["chosen"] = v === "" ? -1 : nodes(n.children).findIndex((r) => text(r, "value") === v);
  }
  if (n.kind === "select") {
    const options = nodes(n.children).filter((c) => c.kind === "option");
    s["chosen"] = Math.max(
      0,
      options.findIndex((o) => text(o, "value") === text(n, "value")),
    );
  }
  return s;
}

// Children in rendered order; SPEC §5.1 decides where columns, tab panels and `empty` go.
function rendered(n: Inst, mode: Mode, selectedOnly: boolean): Inst[] {
  if (n.kind === "list" || n.kind === "table") {
    const content = nodes(n.children);
    const columns = content.filter((c) => c.kind === "column");
    const rest = content.filter((c) => c.kind !== "column");
    const showEmpty = rest.length === 0 || text(n, "state") === "empty";
    return [...columns, ...(showEmpty ? nodes(n.slots["empty"] ?? []) : rest)];
  }
  if (n.kind === "tab" && selectedOnly) return [];
  return nodes(ordered(n));
}

function skeleton(n: Inst, mode: Mode, selectedOnly = false): Skel[] {
  if (n.kind === "dialog" && !flag(n, "open")) return [];
  const kids = (list: Inst[], chosen?: number) =>
    list.flatMap((c, i) =>
      skeleton(c, mode, mode === "aria" && chosen !== undefined && i !== chosen),
    );
  if (n.kind === "tabs") {
    const tabs = rendered(n, mode, false);
    const chosen = states(n, mode)["chosen"] as number;
    return [
      { kind: n.kind, ...ids(n, mode), states: states(n, mode), children: kids(tabs, chosen) },
    ];
  }
  const children = kids(rendered(n, mode, selectedOnly));
  if (dissolved(n, mode)) return children;
  return [{ kind: n.kind, ...ids(n, mode), states: states(n, mode), children }];
}

// Instance ids `id[i]` come back as static siblings named `id-i` (loss "repetition"); a
// snapshot has no ids at all (loss "ids").
function ids(n: Inst, mode: Mode): { id?: string } {
  return mode === "dom" ? { id: instanceId(n.id) ?? n.id } : {};
}

function skeletonOf(document: Document, data: unknown, mode: Mode): Skel[] {
  const root = expandRoot(document.root, catalog, data);
  return root ? skeleton(root, mode) : [];
}

function assertValid(result: ImportResult): void {
  const errors = validate(result.document, { catalog, mode: "lenient" }).filter(
    (d) => d.severity === "error",
  );
  assert.deepEqual(errors, []);
  assert.deepEqual(
    result.diagnostics.filter((d) => d.severity === "error"),
    [],
  );
}

// Known gap: the `empty` slot of a table whose state is `empty` is not read back, so its text
// element is lost in both imports. Pinned with `test.fails` so a fix shows up as a failure here.
const TABLE_EMPTY_SLOT = new Set(["orders"]);

// Known gap: an accessibility snapshot carries a role and a name, not the markup around it. A
// segmented control reads back as a radio group, a stepper, date picker and colour picker as
// fields, a combobox as a select, and a slider loses the range its value was inside. The DOM
// import tells them apart by the hints the renderer writes, so only the snapshot import is
// pinned, with `test.fails` so a fix shows up as a failure here.
const SNAPSHOT_SHARED_ROLES = new Set(["booking", "appearance"]);

// Known gap: a `model` is an `img` with a name, exactly like an `image`, so a snapshot reads it
// back as an `image` whose accessibility tree is the same. The DOM import sees `<model-viewer>`.
// Only the structure check is pinned, with `test.fails` so a fix shows up as a failure here.
const SNAPSHOT_MODEL_IS_IMAGE = new Set(["showroom"]);

for (const s of screens()) {
  const structure = TABLE_EMPTY_SLOT.has(s.name) ? test.fails : test;
  // Vitest has no subtests: each check is a test of its own under the screen's name, and the import runs once per block in
  // `beforeAll` so a failing import fails its own tests instead of the whole file.
  describe(`round trip from DOM: ${s.name}`, () => {
    let result!: ImportResult;
    beforeAll(() => {
      result = fromDom(renderPage(s.document, { catalog, data: s.data }), { catalog });
    });
    test("valid", () => assertValid(result));
    structure("structure, roles, states and ids", () => {
      assert.deepEqual(
        skeletonOf(result.document, {}, "dom"),
        skeletonOf(s.document, s.data, "dom"),
      );
    });
    test("same accessibility tree", () => {
      const expected = expectedTree(s.document, { catalog, data: s.data });
      assert.deepEqual(diffAria(expected, expectedTree(result.document, { catalog })), []);
    });
  });

  const snapshotTest = SNAPSHOT_SHARED_ROLES.has(s.name) ? test.fails : test;
  const snapshotStructure =
    SNAPSHOT_SHARED_ROLES.has(s.name) || SNAPSHOT_MODEL_IS_IMAGE.has(s.name)
      ? test.fails
      : structure;
  describe(`round trip from an accessibility snapshot: ${s.name}`, () => {
    const tree = expectedTree(s.document, { catalog, data: s.data });
    let result!: ImportResult;
    beforeAll(() => {
      result = fromAriaSnapshot(tree, { catalog });
    });
    test("valid", () => assertValid(result));
    snapshotStructure("structure, roles and states", () => {
      assert.deepEqual(
        skeletonOf(result.document, {}, "aria"),
        skeletonOf(s.document, s.data, "aria"),
      );
    });
    snapshotTest("same accessibility tree", () => {
      assert.deepEqual(diffAria(tree, expectedTree(result.document, { catalog })), []);
    });
  });
}
