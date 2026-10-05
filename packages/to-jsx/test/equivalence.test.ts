// The generated component against the reference renderer, on every corpus screen and catalog
// example. Both are server-rendered with the same data and read back with `fromDom`, which keeps
// exactly what the accessibility tree and the document ids carry (roles, names, states, ids,
// nesting, text) and drops what does not matter there (attribute order, event handlers, React's
// text-node markers). Comparing the imported documents is therefore comparing the
// accessibility-relevant structure. The markup itself is compared too, because the renderer's
// boxes that the tree does not show (named-slot wrappers, aria-hidden captions) are part of the
// structure a host styles. Each case runs for React and SolidJS, as JSX and as TSX.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { beforeAll, describe, test } from "vitest";
import { coreCatalog } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { fromDom } from "@weft/from-aria";
import { render } from "@weft/render-react";
import { parseSync } from "oxc-parser";
import { renderToStaticMarkup } from "react-dom/server";
import { toJsx } from "../src/index.ts";
import { load, loadSolid, markup, normalizeMarkup, type Props } from "./compile.ts";

const corpus = new URL("../../../corpus/", import.meta.url);
const examples = new URL("../../catalog/examples/", import.meta.url);

type Case = { name: string; document: Document; data: unknown };

function cases(): Case[] {
  const out: Case[] = [];
  for (const entry of readdirSync(corpus, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const dir = new URL(`${entry.name}/`, corpus);
    out.push({
      name: `corpus/${entry.name}`,
      document: read(new URL("screen.weft", dir)),
      data: JSON.parse(readFileSync(new URL("data.json", dir), "utf8")),
    });
  }
  for (const file of readdirSync(examples)
    .filter((f) => f.endsWith(".weft"))
    .sort()) {
    out.push({ name: `examples/${file}`, document: read(new URL(file, examples)), data: {} });
  }
  return out;
}

function read(url: URL): Document {
  const result = parse(readFileSync(url, "utf8"), { catalog: coreCatalog });
  assert.ok(result.document, `${url.pathname} parses`);
  return result.document;
}

type Variant = {
  label: string;
  framework: "react" | "solid";
  typescript: boolean;
  load: (source: string, typescript: boolean) => Promise<(props: Props) => string>;
};

const loadReact = async (source: string, typescript: boolean) => {
  const component = await load(source, typescript);
  return (props: Props) => markup(component, props);
};

const variants: Variant[] = [
  { label: "", framework: "react", typescript: false, load: loadReact },
  { label: " (React TSX)", framework: "react", typescript: true, load: loadReact },
  { label: " (SolidJS)", framework: "solid", typescript: false, load: loadSolid },
  { label: " (SolidJS TSX)", framework: "solid", typescript: true, load: loadSolid },
];

// Known gap: Solid's server renderer writes a bound `<textarea value>` as an attribute, which
// HTML ignores, so a multiline field's value is missing from the server markup (the DOM build sets
// the property and is correct). Pinned with `test.fails` so a fix shows up as a failure here.
const SOLID_SSR_TEXTAREA = new Set(["corpus/account", "corpus/inbox"]);

for (const c of cases()) {
  for (const v of variants) {
    const gap = v.framework === "solid" && SOLID_SSR_TEXTAREA.has(c.name);
    const same = gap ? test.fails : test;
    // Vitest has no subtests: the checks are tests of one block, sharing what the first renders.
    describe(`equivalence: ${c.name}${v.label}`, () => {
      let source: string;
      let reference: string;
      let generated: string;
      let expected: ReturnType<typeof fromDom>;
      let actual: ReturnType<typeof fromDom>;
      beforeAll(async () => {
        source = toJsx(c.document, {
          catalog: coreCatalog,
          framework: v.framework,
          typescript: v.typescript,
        });
        const component = await v.load(source, v.typescript);
        reference = renderToStaticMarkup(
          render(c.document, { catalog: coreCatalog, data: c.data }),
        );
        generated = component({ data: c.data });
        expected = fromDom(reference, { catalog: coreCatalog });
        actual = fromDom(generated, { catalog: coreCatalog });
      });
      test("the output parses without errors", () => {
        assert.deepEqual(parseSync(v.typescript ? "screen.tsx" : "screen.jsx", source).errors, []);
      });
      same("same accessible structure", () => {
        assert.deepEqual(actual.document, expected.document);
        assert.deepEqual(actual.losses, expected.losses);
      });
      same("same markup", () => {
        assert.equal(normalizeMarkup(generated), normalizeMarkup(reference));
      });
    });
  }
}
