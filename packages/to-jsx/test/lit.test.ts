// The Lit target without a browser: `lit` is replaced by a stub whose `html` keeps the static
// strings and the values of a template apart, which is what decides whether a document string can
// become markup. The accessibility tree in a real browser is compared in `@weft/visual`.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { describe, test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { parseSync } from "oxc-parser";
import { RUNTIME, toJsx } from "../src/index.ts";

const lit = (document: unknown, componentName?: string) =>
  toJsx(document, {
    catalog: coreCatalog,
    framework: "lit",
    ...(componentName === undefined ? {} : { componentName }),
  });

type Template = { strings: string[]; values: unknown[] };
type Screen = { data?: unknown; actions?: unknown; onChange?: unknown; render(): unknown };

const STUB = `export class LitElement {}
export const nothing = Symbol("nothing");
export const html = (strings, ...values) => ({ strings: [...strings], values });
export const unsafeStatic = (value) => ({ unsafe: String(value) });
export const styleMap = (value) => ({ styleMap: value });`;

const STUBBED = new Set(["lit", "lit/static-html.js", "lit/directives/style-map.js"]);
const stubUrl = `data:text/javascript;base64,${Buffer.from(STUB).toString("base64")}`;

const registered: string[] = [];
Object.assign(globalThis, {
  customElements: { get: () => undefined, define: (name: string) => registered.push(name) },
});

async function load(source: string): Promise<new () => Screen> {
  const code = source.replace(/from "([^"]+)"/g, (_, specifier: string) => {
    assert.ok(STUBBED.has(specifier), `unexpected import ${specifier}`);
    return `from ${JSON.stringify(stubUrl)}`;
  });
  const url = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return ((await import(url)) as { default: new () => Screen }).default;
}

/** Every static string and every static value of a template, nested ones included. */
function statics(value: unknown, out: string[] = []): string[] {
  if (Array.isArray(value)) value.forEach((v) => statics(v, out));
  else if (typeof value === "object" && value !== null) {
    if ("strings" in value) {
      const t = value as Template;
      out.push(...t.strings);
      t.values.forEach((v) => statics(v, out));
    } else if ("unsafe" in value) out.push(String((value as { unsafe: string }).unsafe));
  }
  return out;
}

function render(Element: new () => Screen, data: unknown): unknown {
  const el = new Element();
  el.data = data;
  return el.render();
}

const corpus = new URL("../../../corpus/", import.meta.url);
const examples = new URL("../../catalog/examples/", import.meta.url);

function read(url: URL): Document {
  const result = parse(readFileSync(url, "utf8"), { catalog: coreCatalog });
  assert.ok(result.document, `${url.pathname} parses`);
  return result.document;
}

const cases = [
  ...readdirSync(corpus, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => ({
      name: `corpus/${e.name}`,
      document: read(new URL(`${e.name}/screen.weft`, corpus)),
      data: JSON.parse(readFileSync(new URL(`${e.name}/data.json`, corpus), "utf8")) as unknown,
    })),
  ...readdirSync(examples)
    .filter((f) => f.endsWith(".weft"))
    .map((f) => ({ name: `examples/${f}`, document: read(new URL(f, examples)), data: {} })),
];

// `price-row` → `PriceRow`: the function the generator emits for an inline fragment (SPEC §10.7).
const fragmentFunction = (name: string) =>
  name.replace(/(?:^|-)([a-z0-9])/g, (_, c: string) => c.toUpperCase());

// What a generated module may hold at the top level: the imports of `lit`, the runtime helpers,
// one function per inline fragment, the element and its registration. Anything else would be code
// a document smuggled in.
function assertShape(source: string, name = "WeftScreen", fragments: string[] = []): void {
  const { program, errors } = parseSync("screen.js", source);
  assert.deepEqual(errors, [], source);
  const helpers = new Set([...Object.keys(RUNTIME), ...fragments.map(fragmentFunction)]);
  for (const s of program.body) {
    if (s.type === "ImportDeclaration") assert.ok(STUBBED.has(String(s.source.value)));
    else if (s.type === "FunctionDeclaration")
      assert.ok(s.id && helpers.has(s.id.name), String(s.id?.name));
    else if (s.type === "ClassDeclaration") assert.equal(s.id?.name, name);
    else if (s.type === "IfStatement") assert.match(source, /customElements\.define\(/);
    else assert.equal(s.type, "ExportDefaultDeclaration");
  }
}

describe("every corpus screen and catalog example", () => {
  for (const c of cases) {
    test(`${c.name} renders templates with no whitespace between elements`, async () => {
      const source = lit(c.document);
      assertShape(source, undefined, Object.keys(c.document.fragments ?? {}));
      const markup = statics(render(await load(source), c.data)).join("");
      assert.doesNotMatch(markup, />\s+</);
    });
  }
});

const HOSTILE = [
  '"});globalThis.__weftPwned = 1;//',
  "`${globalThis.__weftPwned = 1}`",
  "</main>{globalThis.__weftPwned = 1}<main>",
  "' + (globalThis.__weftPwned = 1) + '",
  "&lt;script&gt;alert(1)&lt;/script&gt;",
  "  \\u0022 */ globalThis.__weftPwned = 1 /*",
  "\ud800 lone surrogate",
];

test("hostile strings stay values, never part of a template", async () => {
  for (const s of HOSTILE) {
    const d = {
      weft: "0.1",
      root: {
        kind: "screen",
        id: "root",
        props: { label: s },
        children: [
          { kind: "heading", id: s, props: { level: { bind: "$.level" }, label: s, text: s } },
          { kind: "text", id: "t", children: [s] },
          { kind: "button", id: "b", props: { variant: s }, on: { press: s }, children: [s] },
          { kind: "link", id: "l", props: { href: s, text: s } },
          {
            kind: "field",
            id: "f",
            props: { label: s, value: { bind: s }, placeholder: s, error: s, type: s },
          },
          { kind: "stack", id: "st", props: { gap: { token: s }, align: s, direction: s } },
          {
            kind: "list",
            id: "li",
            props: { ordered: { bind: "$.ordered" } },
            children: [
              { kind: "each", id: "e", props: { in: { bind: "$.items" }, as: s }, children: [] },
              {
                kind: "each",
                id: "h",
                props: { in: { bind: "$.items" }, as: "html" },
                children: [{ kind: "item", id: "i", props: { text: { bind: "$html.name" } } }],
              },
            ],
            slots: { [s]: [{ kind: "text", id: "x", props: { text: { bind: `$.a.${s}` } } }] },
          },
          { kind: s, id: "k", props: { role: s, label: s }, children: [s] },
        ],
      },
    };
    const source = lit(d);
    assertShape(source);
    const out = render(await load(source), { items: [{ name: s }], level: s, ordered: s });
    const fixed = statics(out).join("");
    assert.equal((globalThis as Record<string, unknown>)["__weftPwned"], undefined);
    // Text made of inert characters may stand in a template as it is; anything that could close
    // an attribute, open a tag, start an entity or end the template may not.
    for (const marker of ['"});', "`", "${", "</main>{", "&lt;script", "u0022"]) {
      assert.ok(!fixed.includes(marker), marker);
    }
  }
});

test("a dynamic tag is a static value from a closed set", async () => {
  const d = {
    weft: "0.1",
    root: {
      kind: "screen",
      id: "root",
      props: { label: "r" },
      children: [
        { kind: "heading", id: "h", props: { level: { bind: "$.level" }, text: "t" } },
        { kind: "list", id: "l", props: { ordered: { bind: "$.ordered" } } },
        { kind: "field", id: "f", props: { type: { bind: "$.type" }, label: "f" } },
      ],
    },
  };
  const source = lit(d);
  assert.match(source, /from "lit\/static-html\.js"/);
  const out = render(await load(source), { level: "<img>", ordered: true, type: "x" });
  const tags = statics(out).filter((s) => /^[a-z0-9]+$/.test(s));
  assert.deepEqual(
    tags.filter((t) => !["h2", "ol", "input"].includes(t)),
    [],
  );
});

test("the element is registered under a name with a hyphen", async () => {
  const d = { weft: "0.1", root: { kind: "screen", id: "r", props: { label: "r" } } };
  registered.length = 0;
  await load(lit(d, "Booking"));
  await load(lit(d, "TripCard"));
  assert.deepEqual(registered, ["weft-booking", "trip-card"]);
});

test("TypeScript and the source comment are refused, not ignored", () => {
  const d = { weft: "0.1", root: { kind: "screen", id: "r", props: { label: "r" } } };
  assert.throws(
    () => toJsx(d, { catalog: coreCatalog, framework: "lit", typescript: true }),
    /no typescript/,
  );
  assert.throws(
    () => toJsx(d, { catalog: coreCatalog, framework: "lit", source: true }),
    /no source/,
  );
  assert.throws(() => lit(d, "x; alert(1)"), TypeError);
});
