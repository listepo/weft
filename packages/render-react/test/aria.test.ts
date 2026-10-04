// expectedTree, the Playwright snapshot parser and the comparison helper, without a browser.
import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import type { Document } from "@weft/core";
import {
  diffAria,
  expectedTree,
  formatAriaSnapshot,
  normalizeAria,
  parseAriaSnapshot,
  type AriaNode,
} from "../src/index.ts";
import { b, doc, el, nb } from "./helpers.ts";

const expected = (d: Document, data: unknown = {}) =>
  expectedTree(d, { catalog: coreCatalog, data });

test("expectedTree declares roles, names and states of a login form", () => {
  const d = doc(
    el(
      "form",
      "f",
      { label: "Sign in" },
      [
        el("field", "email", { type: "email", label: "Email", value: b("$.email") }),
        el("checkbox", "keep", { label: "Keep me signed in", checked: true }),
      ],
      { slots: { footer: [el("button", "go", { disabled: nb("$.email") }, ["Sign in"])] } },
    ),
  );
  assert.deepEqual(expected(d, { email: "" }), {
    role: "fragment",
    name: "",
    children: [
      {
        role: "main",
        name: "Test",
        children: [
          {
            role: "form",
            name: "Sign in",
            children: [
              { role: "textbox", name: "Email" },
              { role: "checkbox", name: "Keep me signed in", states: { checked: true } },
              { role: "button", name: "Sign in", states: { disabled: true } },
            ],
          },
        ],
      },
    ],
  });
});

test("expectedTree expands each, builds the table header row and the selected tab panel", () => {
  const d = doc(
    el("table", "t", { label: "Orders" }, [
      el("column", "c", {}, ["Order"]),
      el("each", "e", { in: b("$.rows"), as: "r" }, [
        el("row", "row", { selected: b("$r.sel") }, [
          el("cell", "x", {}, [el("text", "tx", { text: b("$r.id") })]),
        ]),
      ]),
    ]),
    el("tabs", "tabs", { selected: "b" }, [
      el("tab", "a", { label: "A" }, ["Alpha"]),
      el("tab", "b", { label: "B" }, ["Beta"]),
    ]),
  );
  const main = expected(d, { rows: [{ id: "1", sel: true }, { id: "2" }] }).children![0]!;
  assert.deepEqual(main.children, [
    {
      role: "table",
      name: "Orders",
      children: [
        {
          role: "rowgroup",
          name: "",
          children: [
            { role: "row", name: "Order", children: [{ role: "columnheader", name: "Order" }] },
          ],
        },
        {
          role: "rowgroup",
          name: "",
          children: [
            {
              role: "row",
              name: "1",
              states: { selected: true },
              children: [{ role: "cell", name: "1" }],
            },
            { role: "row", name: "2", children: [{ role: "cell", name: "2" }] },
          ],
        },
      ],
    },
    {
      role: "tablist",
      name: "",
      children: [
        { role: "tab", name: "A" },
        { role: "tab", name: "B", states: { selected: true } },
      ],
    },
    { role: "tabpanel", name: "B", children: [{ role: "text", name: "Beta" }] },
  ]);
});

test("unnamed sections and forms are not landmarks; layout and unknown role none dissolve", () => {
  const d = doc(
    el("section", "s", {}, [
      el("form", "f", {}, [el("stack", "st", {}, [el("text", "t", {}, ["Hi"])])]),
    ]),
    el("x-acme-spacer", "sp", { role: "none" }, ["There"]),
  );
  assert.deepEqual(expected(d).children![0]!.children, [{ role: "text", name: "Hi There" }]);
});

test("expectedTree keeps a safe link url and drops an unsafe one", () => {
  const d = doc(
    el("link", "a", { href: "https://x.test" }, ["A"]),
    el("link", "c", { href: "javascript:x" }, ["C"]),
  );
  assert.deepEqual(expected(d).children![0]!.children, [
    { role: "link", name: "A", url: "https://x.test" },
    { role: "link", name: "C" },
  ]);
});

const SNAPSHOT = `- main "Home":
  - heading "Welcome" [level=1]
  - text: Pick up where you left off.
  - 'link "a: b"':
    - /url: https://x.test
  - textbox "Email" [invalid]:
    - /placeholder: you@example.com
    - text: ana@example.com
  - spinbutton "Age": "41"
  - checkbox "Accept" [checked] [disabled]
  - button "Override": Text
  - text: "\\"quoted\\": yes - [x]"`;

test("parseAriaSnapshot reads the YAML Playwright prints", () => {
  assert.deepEqual(parseAriaSnapshot(SNAPSHOT), {
    role: "fragment",
    name: "",
    children: [
      {
        role: "main",
        name: "Home",
        children: [
          { role: "heading", name: "Welcome", states: { level: 1 } },
          { role: "text", name: "Pick up where you left off." },
          { role: "link", name: "a: b", url: "https://x.test" },
          {
            role: "textbox",
            name: "Email",
            states: { invalid: true },
            children: [{ role: "text", name: "ana@example.com" }],
          },
          { role: "spinbutton", name: "Age", children: [{ role: "text", name: "41" }] },
          { role: "checkbox", name: "Accept", states: { checked: true, disabled: true } },
          { role: "button", name: "Override", children: [{ role: "text", name: "Text" }] },
          { role: "text", name: '"quoted": yes - [x]' },
        ],
      },
    ],
  });
  assert.throws(() => parseAriaSnapshot("not yaml"));
});

test("normalizeAria merges text, dissolves generic nodes and drops text equal to the name", () => {
  const raw: AriaNode = {
    role: "fragment",
    name: "",
    children: [
      { role: "button", name: " Go ", children: [{ role: "text", name: "Go" }] },
      {
        role: "generic",
        name: "",
        children: [
          { role: "text", name: "a" },
          { role: "text", name: " b  c" },
        ],
      },
      { role: "text", name: "d" },
    ],
  };
  assert.deepEqual(normalizeAria(raw), {
    role: "fragment",
    name: "",
    children: [
      { role: "button", name: "Go" },
      { role: "text", name: "a b c d" },
    ],
  });
});

test("diffAria lists role, name, state, url and child differences with their path", () => {
  const e: AriaNode = {
    role: "fragment",
    name: "",
    children: [
      {
        role: "main",
        name: "M",
        children: [
          { role: "button", name: "Go", states: { disabled: true } },
          { role: "link", name: "L", url: "/a" },
        ],
      },
    ],
  };
  const a: AriaNode = {
    role: "fragment",
    name: "",
    children: [
      {
        role: "main",
        name: "M",
        children: [{ role: "link", name: "Go!", states: { active: true } as never }],
      },
    ],
  };
  assert.deepEqual(diffAria(e, e), []);
  assert.deepEqual(diffAria(e, a), [
    'root > [0] main "M": expected 2 children [button "Go", link "L"], got 1 [link "Go!"]',
    'root > [0] main "M" > [0] button "Go": expected role button, got link',
    'root > [0] main "M" > [0] button "Go": expected name "Go", got "Go!"',
    'root > [0] main "M" > [0] button "Go": expected states {"disabled":true}, got {}',
  ]);
});

test("formatAriaSnapshot prints the YAML parseAriaSnapshot reads back", () => {
  const tree = parseAriaSnapshot(SNAPSHOT);
  assert.deepEqual(parseAriaSnapshot(formatAriaSnapshot(tree)), normalizeAria(tree));
  const tricky: AriaNode = {
    role: "fragment",
    name: "",
    children: [
      { role: "button", name: 'say "hi": now', states: { pressed: "mixed", disabled: true } },
      { role: "text", name: "42" },
      {
        role: "link",
        name: "x",
        url: "https://x.test/#a b",
        children: [{ role: "text", name: "y" }],
      },
      {
        role: "heading",
        name: "",
        states: { level: 2 },
        children: [{ role: "text", name: "- z" }],
      },
    ],
  };
  const yaml = formatAriaSnapshot(tricky);
  assert.equal(
    yaml,
    [
      `- 'button "say \\"hi\\": now" [disabled] [pressed=mixed]'`,
      '- text: "42"',
      '- link "x":',
      "  - /url: https://x.test/#a b",
      "  - text: y",
      '- heading [level=2]: "- z"',
    ].join("\n"),
  );
  assert.deepEqual(parseAriaSnapshot(yaml), normalizeAria(tricky));
});
