import assert from "node:assert/strict";
import { test } from "node:test";
import { coreCatalog } from "@weft/catalog";
import { validate, type Diagnostic, type Document } from "@weft/core";
import type { AriaNode } from "@weft/render-react";
import fc from "fast-check";
import {
  fromAriaSnapshot,
  fromDom,
  instanceId,
  MAX_HTML_LENGTH,
  MAX_SNAPSHOT_LENGTH,
  type ImportResult,
} from "../src/index.ts";

const catalog = coreCatalog;
type N = {
  kind: string;
  id: string;
  props?: Record<string, unknown>;
  children?: (N | string)[];
  slots?: Record<string, (N | string)[]>;
};
const root = (r: ImportResult) => r.document.root as N;
const kids = (n: N) => (n.children ?? []).filter((c): c is N => typeof c !== "string");
const codes = (d: Diagnostic[]) => d.map((x) => x.code);

function assertValid(r: ImportResult): void {
  const errors = validate(r.document, { catalog, mode: "lenient" }).filter(
    (d) => d.severity === "error",
  );
  assert.deepEqual(errors, [], JSON.stringify(r.document));
}

function find(n: N, kind: string): N | undefined {
  if (n.kind === kind) return n;
  for (const c of [
    ...kids(n),
    ...Object.values(n.slots ?? {})
      .flat()
      .filter((c): c is N => typeof c !== "string"),
  ]) {
    const hit = find(c, kind);
    if (hit) return hit;
  }
  return undefined;
}

const aria = (children: AriaNode[]): AriaNode => ({ role: "fragment", name: "", children });

test("snapshot YAML: roles map to kinds, special roles to their refinements", () => {
  const yaml = [
    '- main "Shop":',
    '  - heading "Cart" [level=2]',
    '  - spinbutton "Quantity"',
    '  - searchbox "Find"',
    '  - checkbox "Gift wrap" [checked]',
    '  - button "Pay" [disabled]',
    '  - marquee "Sale"',
  ].join("\n");
  const r = fromAriaSnapshot(yaml, { catalog });
  assertValid(r);
  const main = root(r);
  assert.equal(main.kind, "screen");
  assert.equal(main.props?.["label"], "Shop");
  const [heading, quantity, find, gift, pay, marquee] = kids(main);
  assert.deepEqual([heading?.kind, heading?.props?.["level"]], ["heading", 2]);
  assert.deepEqual([quantity?.kind, quantity?.props?.["type"]], ["field", "number"]);
  assert.deepEqual([find?.kind, find?.props?.["type"]], ["field", "search"]);
  assert.deepEqual([gift?.kind, gift?.props?.["checked"]], ["checkbox", true]);
  assert.deepEqual([pay?.kind, pay?.props?.["disabled"]], ["button", true]);
  assert.deepEqual([marquee?.kind, marquee?.props?.["role"]], ["x-aria-marquee", "marquee"]);
  assert.ok(
    r.losses.some((l) => l.kind === "kinds" && l.path.endsWith("x-aria-marquee#marquee-sale")),
  );
  assert.ok(r.losses.some((l) => l.kind === "ids"));
});

test("snapshot: tablist and tabpanel become tabs, the header row becomes columns", () => {
  const r = fromAriaSnapshot(
    aria([
      {
        role: "main",
        name: "",
        children: [
          {
            role: "tablist",
            name: "",
            children: [
              { role: "tab", name: "One", states: { selected: false } },
              { role: "tab", name: "Two", states: { selected: true } },
            ],
          },
          { role: "tabpanel", name: "Two", children: [{ role: "button", name: "Go" }] },
          {
            role: "table",
            name: "People",
            children: [
              {
                role: "rowgroup",
                name: "",
                children: [
                  { role: "row", name: "Name", children: [{ role: "columnheader", name: "Name" }] },
                ],
              },
              {
                role: "rowgroup",
                name: "",
                children: [{ role: "row", name: "Ada", children: [{ role: "cell", name: "Ada" }] }],
              },
            ],
          },
        ],
      },
    ]),
    { catalog },
  );
  assertValid(r);
  const tabs = find(root(r), "tabs");
  assert.ok(tabs);
  const [one, two] = kids(tabs);
  assert.equal(tabs.props?.["selected"], two?.id);
  assert.equal(kids(two as N)[0]?.kind, "button");
  assert.deepEqual(kids(one as N), []);
  assert.ok(r.losses.some((l) => l.kind === "hidden"));
  const table = find(root(r), "table");
  assert.deepEqual(
    kids(table as N).map((c) => c.kind),
    ["column", "row"],
  );
});

test("DOM: data-weft-id ids are kept and instance ids become static siblings", () => {
  const html = `<main data-weft-id="s" aria-label="S"><ul data-weft-id="l">
    <li data-weft-id="it[0]">A</li><li data-weft-id="it[1]">B</li></ul>
    <section aria-label="X"><button type="button">Plain</button></section></main>`;
  const r = fromDom(html, { catalog });
  assertValid(r);
  const list = find(root(r), "list");
  assert.deepEqual(
    kids(list as N).map((c) => c.id),
    ["it-0", "it-1"],
  );
  assert.equal(root(r).id, "s");
  assert.ok(r.losses.some((l) => l.kind === "repetition"));
  assert.equal(instanceId("row[0][12]"), "row-0-12");
});

test("DOM: implicit roles, labels and states", () => {
  const html = `<main aria-label="Form">
    <form aria-label="Sign in"><label for="e">Email</label><input id="e" type="email" required aria-invalid="true">
    <label><input type="checkbox" checked> Remember</label>
    <label><span aria-hidden="true">Plan</span><select aria-label="Plan"><option value="a">A</option><option value="b" selected>B</option></select></label>
    <button type="submit">Go</button></form>
    <h3>Title</h3><a href="javascript:alert(1)">bad</a><img src="x.png" alt="">
    <div hidden><button>Hidden</button></div></main>`;
  const r = fromDom(html, { catalog });
  assertValid(r);
  const form = find(root(r), "form") as N;
  const [field, checkbox, select, button] = kids(form);
  assert.deepEqual(
    [field?.kind, field?.props?.["label"], field?.props?.["type"], field?.props?.["required"]],
    ["field", "Email", "email", true],
  );
  assert.equal(field?.props?.["state"], "invalid");
  assert.deepEqual(
    [checkbox?.kind, checkbox?.props?.["label"], checkbox?.props?.["checked"]],
    ["checkbox", "Remember", true],
  );
  assert.deepEqual([select?.kind, select?.props?.["value"]], ["select", "b"]);
  // A wrapping label names the select; the option text inside it is still the options' own.
  assert.deepEqual(
    kids(select as N).map((o) => o.children),
    [["A"], ["B"]],
  );
  assert.deepEqual([button?.kind, button?.props?.["submit"]], ["button", true]);
  assert.equal(find(root(r), "heading")?.props?.["level"], 3);
  assert.equal(JSON.stringify(r.document).includes("Hidden"), false);
});

test("unreadable input is a W601 diagnostic, never an exception", () => {
  for (const bad of [42, null, [], "- not a [snapshot", '- button "x', { role: 7 }]) {
    const r = fromAriaSnapshot(bad as never, { catalog });
    assertValid(r);
    if (typeof bad !== "object" || bad === null || Array.isArray(bad))
      assert.ok(codes(r.diagnostics).includes("W601"), String(bad));
  }
  const r = fromDom(5 as never, { catalog });
  assert.deepEqual(codes(r.diagnostics), ["W601"]);
  assertValid(r);
});

test("oversized input is cut with a W602 diagnostic", () => {
  const yaml = `${'- button "x"\n'.repeat(Math.ceil(MAX_SNAPSHOT_LENGTH / 13) + 10)}`;
  const big = fromAriaSnapshot(yaml, { catalog });
  assert.ok(codes(big.diagnostics).includes("W602"));
  assertValid(big);
  const html = `<main>${"<p>x</p>".repeat(Math.ceil(MAX_HTML_LENGTH / 8) + 10)}</main>`;
  const dom = fromDom(html, { catalog });
  assert.ok(codes(dom.diagnostics).includes("W602"));
  assertValid(dom);
});

test("deep nesting stops at the depth limit", () => {
  const deep = `${"<div role=group>".repeat(5000)}x${"</div>".repeat(5000)}`;
  const r = fromDom(deep, { catalog });
  assert.ok(codes(r.diagnostics).includes("W602"));
  assertValid(r);
  let node: AriaNode = { role: "group", name: "" };
  for (let i = 0; i < 5000; i++) node = { role: "group", name: "", children: [node] };
  const s = fromAriaSnapshot(node, { catalog });
  assert.ok(codes(s.diagnostics).includes("W602"));
  assertValid(s);
});

test("hostile text stays literal and never forms a binding or token", () => {
  const r = fromAriaSnapshot(
    aria([
      { role: "button", name: "{$.secret} {token.x}" },
      { role: "text", name: "a\u0000b {!$.x}" },
    ]),
    { catalog },
  );
  assertValid(r);
  const json = JSON.stringify(r.document);
  assert.ok(!json.includes('"bind"') && !json.includes('"token"'));
  assert.ok(!json.includes("\\u0000"));
});

test("ids from the DOM that are invalid or duplicated are replaced", () => {
  const r = fromDom(
    `<main><button data-weft-id="1bad">a</button><button data-weft-id="x">b</button><button data-weft-id="x">c</button></main>`,
    { catalog },
  );
  assertValid(r);
  const ids = kids(root(r)).map((c) => c.id);
  assert.equal(new Set(ids).size, ids.length);
  assert.ok(r.losses.some((l) => l.kind === "ids"));
});

const roles = fc.constantFrom(
  "main",
  "button",
  "link",
  "list",
  "listitem",
  "table",
  "row",
  "cell",
  "columnheader",
  "rowgroup",
  "tablist",
  "tab",
  "tabpanel",
  "checkbox",
  "radio",
  "radiogroup",
  "combobox",
  "option",
  "textbox",
  "heading",
  "text",
  "dialog",
  "generic",
  "none",
  "img",
  "form",
  "region",
  "menu",
  "menuitem",
  "x",
  "__proto__",
);
const ariaNode: fc.Arbitrary<unknown> = fc.letrec<{ node: unknown }>((tie) => ({
  node: fc.oneof(
    { depthSize: "small" },
    fc.record(
      {
        role: fc.oneof(roles, fc.string()),
        name: fc.oneof(fc.string(), fc.constant("{$.x}")),
        states: fc.dictionary(
          fc.constantFrom(
            "checked",
            "disabled",
            "selected",
            "level",
            "expanded",
            "pressed",
            "invalid",
          ),
          fc.oneof(fc.boolean(), fc.integer(), fc.string()),
        ),
        url: fc.string(),
        children: fc.array(tie("node"), { maxLength: 4 }),
      },
      { requiredKeys: ["role"] },
    ),
    fc.anything(),
  ),
})).node;

test("property: any snapshot imports to a document that validates without errors", () => {
  fc.assert(
    fc.property(ariaNode, (node) => {
      const r = fromAriaSnapshot(node as AriaNode, { catalog });
      assertValid(r);
      assert.equal((r.document as Document).weft, "0.1");
    }),
    { numRuns: 300 },
  );
});

const tags = fc.constantFrom(
  "main",
  "div",
  "span",
  "button",
  "a",
  "ul",
  "ol",
  "li",
  "table",
  "thead",
  "tbody",
  "tr",
  "th",
  "td",
  "form",
  "input",
  "select",
  "option",
  "label",
  "h2",
  "p",
  "section",
  "dialog",
  "img",
  "textarea",
);
const attrs = fc.array(
  fc.tuple(
    fc.constantFrom(
      "role",
      "aria-label",
      "data-weft-id",
      "href",
      "type",
      "checked",
      "hidden",
      "id",
      "for",
      "aria-labelledby",
      "data-state",
      "style",
      "value",
    ),
    fc.string(),
  ),
  { maxLength: 3 },
);
const html: fc.Arbitrary<string> = fc.letrec<{ el: string }>((tie) => ({
  el: fc.oneof(
    { depthSize: "small" },
    fc.string(),
    fc
      .tuple(tags, attrs, fc.array(tie("el"), { maxLength: 4 }))
      .map(
        ([t, a, c]) =>
          `<${t}${a.map(([k, v]) => ` ${k}="${v.replaceAll('"', "&quot;")}"`).join("")}>${c.join("")}</${t}>`,
      ),
  ),
})).el;

test("property: any markup imports to a document that validates without errors", () => {
  fc.assert(
    fc.property(html, (markup) => {
      assertValid(fromDom(markup, { catalog }));
    }),
    { numRuns: 300 },
  );
});
