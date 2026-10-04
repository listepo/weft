// Cases for the TypeScript–Rust differential check of the importers. The package computes the
// expected results into two fixtures that the Rust crates reproduce byte for byte:
// crates/weft-import/tests/fixtures/build.json (role tree → document) and
// crates/weft-web/tests/fixtures/from-dom.json (HTML → document). Random cases use a fixed seed,
// so the fixtures only change with the code.
import { readdirSync, readFileSync } from "node:fs";
import fc from "fast-check";
import { parseDocument } from "htmlparser2";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { expectedTree, renderPage, type AriaNode } from "@weft/render-react";
import { snapshotSems } from "../src/aria.ts";
import { buildDocument } from "../src/build.ts";
import { fromDom } from "../src/index.ts";
import type { Sem } from "../src/types.ts";

export const buildFixture = new URL(
  "../../../crates/weft-import/tests/fixtures/build.json",
  import.meta.url,
);
export const domFixture = new URL(
  "../../../crates/weft-web/tests/fixtures/from-dom.json",
  import.meta.url,
);

const SEED = 20261005;
const RUNS = 250;
const catalog = coreCatalog;
const repository = new URL("../../../", import.meta.url);

/** Only what survives a JSON round trip reaches the Rust side, so expectations use the same. */
const json = (value: unknown): unknown => JSON.parse(JSON.stringify(value) ?? "null");

type Screen = { name: string; document: Document; data: unknown; html: string };

function corpus(): Screen[] {
  const dir = new URL("corpus/", repository);
  return readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .toSorted()
    .map((name) => {
      const at = new URL(`${name}/`, dir);
      const parsed = parse(readFileSync(new URL("screen.weft", at), "utf8"), { catalog });
      if (!parsed.document) throw new Error(`corpus/${name} does not parse`);
      return {
        name,
        document: parsed.document,
        data: JSON.parse(readFileSync(new URL("data.json", at), "utf8")) as unknown,
        html: readFileSync(new URL("screen.html", at), "utf8"),
      };
    });
}

function examples(): Document[] {
  const dir = new URL("packages/catalog/examples/", repository);
  return readdirSync(dir)
    .filter((f) => f.endsWith(".weft"))
    .toSorted()
    .flatMap((f) => parse(readFileSync(new URL(f, dir), "utf8"), { catalog }).document ?? []);
}

// The inputs of test/importers.test.ts, so the Rust crate answers to the same cases.
const HAND_WRITTEN = [
  `<main data-weft-id="s" aria-label="S"><ul data-weft-id="l">
    <li data-weft-id="it[0]">A</li><li data-weft-id="it[1]">B</li></ul>
    <section aria-label="X"><button type="button">Plain</button></section></main>`,
  `<main aria-label="Form">
    <form aria-label="Sign in"><label for="e">Email</label><input id="e" type="email" required aria-invalid="true">
    <label><input type="checkbox" checked> Remember</label>
    <label><span aria-hidden="true">Plan</span><select aria-label="Plan"><option value="a">A</option><option value="b" selected>B</option></select></label>
    <button type="submit">Go</button></form>
    <h3>Title</h3><a href="javascript:alert(1)">bad</a><img src="x.png" alt="">
    <div hidden><button>Hidden</button></div></main>`,
  `<main><button data-weft-id="1bad">a</button><button data-weft-id="x">b</button><button data-weft-id="x">c</button></main>`,
  `${"<div role=group>".repeat(5000)}x${"</div>".repeat(5000)}`,
  `<main><div role="tablist" aria-label="T"><button role="tab" id="a" aria-selected="true">A</button><button role="tab" id="b">B</button></div>
    <div role="tabpanel" aria-labelledby="a"><p>One</p></div><div role="tabpanel" aria-labelledby="b" hidden><p>Two</p></div>
    <table aria-label="People"><caption>People</caption><thead><tr><th aria-sort="ascending">Name</th></tr></thead><tbody><tr><td>Ada</td></tr></tbody></table>
    <form><input type="submit" value="Send"><input type="hidden" value="h"><textarea aria-describedby="err">text</textarea><p id="err">Too short</p>
    <input type="number" value="4" aria-invalid="true"><input type="search" placeholder="Find"><input type="range"></form>
    <div data-weft-id="g" style="display: grid; grid-template-columns: repeat(3, 1fr); gap: var(--weft-space-sm)"><p>a</p></div>
    <div data-weft-id="r" style="display:flex;flex-direction:row;align-items:center;flex-wrap:wrap;gap:4px"><span>b</span></div>
    <span data-weft-id="t" style="gap: 1px">c<br>d</span><ol><li>one</li></ol><dialog open aria-modal="true" aria-label="D"><p>x</p></dialog>
    <a href="/x" title="Go">Go</a><img alt="Logo" src="l.png"><select multiple><option>m</option></select><h7>no</h7><nav aria-busy="true">n</nav>
    <!-- a comment --><script>alert(1)</script><style>p{}</style><template><p>t</p></template></main>`,
  "",
  "plain text",
  "<html><head><title>T</title></head><body><main aria-label='M'>x</main></body></html>",
];

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
    fc.oneof(
      fc.string(),
      fc.constantFrom("tab", "tablist", "tabpanel", "a", "b", "x[0]", "checkbox", "submit"),
    ),
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

const ROLES = [
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
  "spinbutton",
  "searchbox",
  "paragraph",
  "x",
  "__proto__",
];
const scalar = fc.oneof(fc.boolean(), fc.integer({ min: -2, max: 8 }), fc.string({ maxLength: 4 }));
const sem: fc.Arbitrary<Sem> = fc.letrec<{ sem: Sem }>((tie) => ({
  sem: fc.record(
    {
      role: fc.oneof({ arbitrary: fc.constantFrom(...ROLES), weight: 5 }, fc.string()),
      name: fc.oneof(fc.constantFrom("", "One", "Two", "{$.x} y", " a  b "), fc.string()),
      states: fc.dictionary(
        fc.constantFrom("checked", "disabled", "selected", "level", "invalid", "busy", "expanded"),
        scalar,
        { maxKeys: 3 },
      ),
      props: fc.dictionary(
        fc.constantFrom(
          "state",
          "variant",
          "tone",
          "type",
          "value",
          "href",
          "submit",
          "required",
          "columns",
          "direction",
          "error",
          "modal",
          "text",
        ),
        fc.oneof(fc.constantFrom("true", "primary", "email", "2", "row", "busy"), fc.string()),
        { maxKeys: 3 },
      ),
      kind: fc.constantFrom(...Object.keys(catalog.components), "screen", "x-nope"),
      id: fc.oneof(fc.constantFrom("a", "b", "screen", "1bad", "tab-one"), fc.string()),
      ref: fc.constantFrom("a", "b", "c"),
      labelledBy: fc.array(fc.constantFrom("a", "b", "c"), { maxLength: 2 }),
      notes: fc.array(
        fc.record({ kind: fc.constantFrom("ids", "layout", "tokens"), note: fc.string() }),
        { maxLength: 1 },
      ),
      children: fc.oneof(
        { depthSize: "small" },
        fc.constant([]),
        fc.array(tie("sem"), { maxLength: 4 }),
      ),
    },
    { requiredKeys: ["role", "name", "states", "props", "children"] },
  ),
})).sem;

export type BuildCase = { sems: unknown; reserved: string[]; result: unknown };
export type DomCase = { html: string; sample?: true; tree: unknown; result: unknown };

type HNode = {
  type: string;
  name?: string;
  data?: string;
  attribs?: Record<string, string>;
  children?: HNode[];
};
const isElement = (n: HNode): boolean => ["tag", "script", "style"].includes(n.type);

// The tree the importer reads, in a form both parsers can print: htmlparser2 here, html5ever in
// Rust. They agree on well-formed markup and differ on what the HTML standard repairs, so the Rust
// test compares results only where the trees agree.
// Deeper than the import reads, a subtree is only marked, so deep cases fit the stack.
const DUMP_DEPTH = 256;

function dump(nodes: HNode[], depth = 0): unknown[] {
  if (depth > DUMP_DEPTH) return ["…"];
  const out: unknown[] = [];
  for (const n of nodes) {
    if (n.type === "text") out.push(n.data ?? "");
    else if (isElement(n))
      out.push([n.name, Object.entries(n.attribs ?? {}), dump(n.children ?? [], depth + 1)]);
  }
  // A browser moves whitespace after </body> into the body; it never changes a result.
  while (typeof out.at(-1) === "string" && (out.at(-1) as string).trim() === "") out.pop();
  return out;
}

function tree(html: string): unknown {
  const root = parseDocument(html) as unknown as HNode;
  const top = (root.children ?? []).filter(isElement);
  const htmlEl = top.find((n) => n.name === "html");
  const inner = htmlEl ? (htmlEl.children ?? []).filter(isElement) : [];
  const body = [...top, ...inner].find((n) => n.name === "body");
  const head = [...top, ...inner].find((n) => n.name === "head");
  return {
    head: dump(head?.children ?? []),
    body: dump(body ? (body.children ?? []) : (root.children ?? [])),
  };
}

function buildCase(sems: readonly Sem[], reserved: string[]): BuildCase {
  const built = buildDocument(sems, { catalog, reserved });
  return { sems: json(sems), reserved, result: json(built) };
}

export function buildCases(): BuildCase[] {
  const out: BuildCase[] = [];
  for (const s of corpus()) {
    const tree = expectedTree(s.document, { catalog, data: s.data });
    out.push(buildCase(snapshotSems(tree as unknown as Record<string, unknown>).sems, []));
  }
  for (const d of examples()) {
    const tree: AriaNode = expectedTree(d, { catalog, data: {} });
    out.push(buildCase(snapshotSems(tree as unknown as Record<string, unknown>).sems, []));
  }
  const samples = fc.sample(fc.tuple(fc.array(sem, { maxLength: 3 }), fc.boolean()), {
    seed: SEED,
    numRuns: RUNS,
  });
  for (const [sems, reserve] of samples) out.push(buildCase(sems, reserve ? ["a", "screen"] : []));
  return out;
}

export function domCases(): DomCase[] {
  const inputs: string[] = [];
  for (const s of corpus()) {
    inputs.push(renderPage(s.document, { catalog, data: s.data }));
    inputs.push(s.html);
  }
  for (const d of examples()) inputs.push(renderPage(d, { catalog, data: {} }));
  inputs.push(...HAND_WRITTEN);
  const cases: DomCase[] = inputs.map((h) => ({
    html: h,
    tree: tree(h),
    result: json(fromDom(h, { catalog })),
  }));
  for (const h of fc.sample(html, { seed: SEED, numRuns: RUNS }))
    cases.push({ html: h, sample: true, tree: tree(h), result: json(fromDom(h, { catalog })) });
  return cases;
}
