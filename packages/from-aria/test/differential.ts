// Cases the Rust importers answer to. The fixtures were first written by the TypeScript importers
// and reproduced by the Rust ones (crates/weft-import/tests/build.rs, crates/weft-web/tests/
// from_dom.rs); the importers now run in Rust, so they pin results against unnoticed change and
// tie the package to the crate. helpers.json is still written by TypeScript: the helpers
// `@weft/figma` reuses without WebAssembly have a Rust twin in crates/weft-import, and
// crates/weft-import/tests/helpers.rs checks the two agree. Random cases use a fixed seed, so the
// fixtures only change with the code.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import fc from "fast-check";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { expectedTree, renderPage, type AriaNode } from "@weft/render-react";
import { snapshotSems } from "../src/aria.ts";
import { fillRequired, freshId, literal, slug, type IdState } from "../src/build.ts";
import { buildDocument } from "../src/engine.ts";
import { fromDom } from "../src/index.ts";
import type { Sem } from "../src/types.ts";

export const buildFixture = new URL(
  "../../../crates/weft-import/tests/fixtures/build.json",
  import.meta.url,
);
export const helpersFixture = new URL(
  "../../../crates/weft-import/tests/fixtures/helpers.json",
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

// `html` is the hand-written page of a benchmark screen; the coverage screens have none (corpus/README.md).
type Screen = { name: string; document: Document; data: unknown; html: string | undefined };

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
        html: existsSync(new URL("screen.html", at))
          ? readFileSync(new URL("screen.html", at), "utf8")
          : undefined,
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
        fc.constant<Sem[]>([]),
        fc.array(tie("sem"), { maxLength: 4 }),
      ),
    },
    { requiredKeys: ["role", "name", "states", "props", "children"] },
  ),
})).sem;

export type BuildCase = { sems: unknown; reserved: string[]; result: unknown };
export type DomCase = { html: string; result: unknown };

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
    if (s.html !== undefined) inputs.push(s.html);
  }
  for (const d of examples()) inputs.push(renderPage(d, { catalog, data: {} }));
  inputs.push(...HAND_WRITTEN);
  inputs.push(...fc.sample(html, { seed: SEED, numRuns: RUNS }));
  return inputs.map((h) => ({ html: h, result: json(fromDom(h, { catalog })) }));
}

const helperText = fc.oneof(
  fc.string(),
  fc.string({ unit: "binary" }),
  fc
    .array(
      fc.constantFrom("{", "$", "!$", "token.", "a", " ", "}", "\u0000", "é", "ﬁ", "-", "日"),
      {
        maxLength: 12,
      },
    )
    .map((parts) => parts.join("")),
);

export type HelperCases = {
  slug: [string, string][];
  literal: { text: string; out: string; losses: unknown }[];
  freshId: { used: string[]; calls: [string, string][]; ids: string[] }[];
  fillRequired: {
    kind: string;
    parent: string | null;
    root: boolean;
    name: string;
    props: unknown;
    losses: unknown;
  }[];
};

export function helperCases(): HelperCases {
  const texts = [
    "",
    "Save changes!",
    "  Crème brûlée ",
    "ﬁle",
    "--",
    "日本",
    "ab ".repeat(20),
    "{$.x} {$.y}",
    "a {$.x} and {token.y}",
    "a {!$.x}",
    ...fc.sample(helperText, { seed: SEED, numRuns: RUNS }),
  ];
  const literalCases = texts.map((text) => {
    const log = { losses: [] };
    return { text, out: literal(log, "/p", text), losses: json(log.losses) };
  });
  const idCalls = fc.sample(
    fc.record({
      used: fc.array(fc.constantFrom("a-x", "a-x-2", "a-1", "a-2", "b-save", "b"), {
        maxLength: 4,
      }),
      calls: fc.array(
        fc.tuple(
          fc.constantFrom("a", "b"),
          fc.oneof(fc.constantFrom("", "x", "X!", "save"), helperText),
        ),
        { maxLength: 8 },
      ),
    }),
    { seed: SEED, numRuns: RUNS },
  );
  const freshCases = idCalls.map(({ used, calls }) => {
    const state: IdState = { used: new Set(used), counters: new Map() };
    return { used, calls, ids: calls.map(([base, name]) => freshId(state, base, name)) };
  });
  const kinds = Object.keys(catalog.components);
  const fills: HelperCases["fillRequired"] = [];
  for (const kind of kinds) {
    for (const parent of [null, ...kinds]) {
      for (const [root, name] of [
        [false, "Pick me"],
        [true, ""],
      ] as const) {
        const log = { losses: [] };
        const props = {};
        const def = catalog.components[kind]!;
        const parentDef = parent === null ? undefined : catalog.components[parent];
        fillRequired(log, props, def, parentDef, root, name, "/x");
        if (log.losses.length > 0)
          fills.push({ kind, parent, root, name, props, losses: json(log.losses) });
      }
    }
  }
  return {
    slug: texts.map((t) => [t, slug(t)]),
    literal: literalCases,
    freshId: freshCases,
    fillRequired: fills,
  };
}
