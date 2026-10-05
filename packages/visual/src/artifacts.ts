// Everything the browser renders for a corpus screen, produced in Node before the tests run: the
// reference renderer's markup, the generated React and SolidJS components compiled for the
// browser, the static HTML page, and the same targets after a round trip through each importer
// and design tool.
import { transformSync as babel } from "@babel/core";
import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { coreCatalog } from "@weft/catalog";
import { parse, serialize, type Document } from "@weft/core";
import { render } from "@weft/render-react";
import { toJsx } from "@weft/to-jsx";
import { transformSync } from "oxc-transform";
import { renderToStaticMarkup } from "react-dom/server";
import { built as figmaBuilt, read as figmaRead } from "../../figma/test/helpers.ts";
import { built as penpotBuilt, read as penpotRead } from "../../penpot/test/helpers.ts";
import { CORPUS, cli } from "./cli.ts";

// Babel loads a preset from a path; the preset ships no types to import it by.
const SOLID_PRESET = createRequire(import.meta.url).resolve("babel-preset-solid");

export type Framework = "react" | "solid";

/** What a screen renders as, per target; component code is an ES module for the browser. */
export type Screen = {
  name: string;
  data: unknown;
  /** The reference renderer's static markup, with the screen's data. */
  reference: string;
  /** The static HTML page, generated with the screen's data. */
  html: string;
  /** The generated components, compiled for the browser. */
  react: string;
  solid: string;
  /** Each target after a round trip through its importer or design tool, re-rendered. */
  back: {
    html: string;
    react: string;
    solid: string;
    figma: string;
    penpot: string;
  };
};

export function corpusNames(): string[] {
  return readdirSync(CORPUS, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort();
}

function parseMarkup(markup: string, what: string): Document {
  const { document, diagnostics } = parse(markup, { catalog: coreCatalog });
  if (document === undefined) throw new Error(`${what}: ${JSON.stringify(diagnostics)}`);
  return document;
}

function generate(document: Document, framework: Framework): string {
  return toJsx(document, { catalog: coreCatalog, framework });
}

/** Compiles generated JSX the way a host's browser build would. */
export function compile(source: string, framework: Framework): string {
  const jsx = framework === "react" ? { runtime: "automatic" as const } : ("preserve" as const);
  const stripped = transformSync("screen.jsx", source, { jsx });
  if (stripped.errors.length > 0) throw new Error(stripped.errors.map((e) => e.message).join("\n"));
  if (framework === "react") return stripped.code;
  const result = babel(stripped.code, {
    babelrc: false,
    configFile: false,
    filename: "screen.jsx",
    presets: [[SOLID_PRESET, { generate: "dom", hydratable: false }]],
  });
  if (typeof result?.code !== "string") throw new Error("babel-preset-solid produced no code");
  return result.code;
}

const referenceMarkup = (document: Document, data: unknown) =>
  renderToStaticMarkup(render(document, { catalog: coreCatalog, data }));

async function screen(name: string): Promise<Screen> {
  const markup = readFileSync(join(CORPUS, name, "screen.weft"), "utf8");
  const data: unknown = JSON.parse(readFileSync(join(CORPUS, name, "data.json"), "utf8"));
  const document = parseMarkup(markup, name);
  const dataFile = join(CORPUS, name, "data.json");
  const page = (weft: string) =>
    cli("html", "screen.weft", weft, "--no-source", "--data", dataFile);
  // Convention imports, without the source comment: what a hand-edited page or component reads
  // back as. A page with data is a picture, not a template, so the HTML round trip reads the
  // template page and fills it with the same data again.
  const template = cli("html", "screen.weft", markup, "--no-source");
  const htmlBack = parseMarkup(cli("import-html", "screen.html", template), `${name} from HTML`);
  const jsxBack = (framework: Framework) =>
    parseMarkup(
      cli(`import-${framework}`, "screen.jsx", generate(document, framework)),
      `${name} from ${framework}`,
    );
  const figma = await figmaBuilt(markup);
  const fromFigma = await figmaRead(figma.figma, figma.frame);
  const penpot = await penpotBuilt(markup);
  const fromPenpot = await penpotRead(penpot.root);
  return {
    name,
    data,
    reference: referenceMarkup(document, data),
    html: page(markup),
    react: compile(generate(document, "react"), "react"),
    solid: compile(generate(document, "solid"), "solid"),
    back: {
      html: page(serialize(htmlBack)),
      react: compile(generate(jsxBack("react"), "react"), "react"),
      solid: compile(generate(jsxBack("solid"), "solid"), "solid"),
      figma: referenceMarkup(fromFigma.document, data),
      penpot: referenceMarkup(fromPenpot.document, data),
    },
  };
}

export async function screens(): Promise<Screen[]> {
  return Promise.all(corpusNames().map(screen));
}
