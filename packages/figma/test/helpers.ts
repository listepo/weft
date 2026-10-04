import { readdirSync, readFileSync } from "node:fs";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { parse, type Document } from "@weft/core";
import { buildScreen, ensureLibrary, readScreen } from "../src/index.ts";
import { FakeFigma, type FakeFrame } from "./fake-figma.ts";

export const tokens = loadTokens(
  JSON.parse(
    readFileSync(new URL("../../catalog/tokens/default.tokens.json", import.meta.url), "utf8"),
  ),
).tokens;

const corpus = new URL("../../../corpus/", import.meta.url);

export const corpusNames = readdirSync(corpus, { withFileTypes: true })
  .filter((e) => e.isDirectory())
  .map((e) => e.name)
  .sort();

export const corpusMarkup = (name: string): string =>
  readFileSync(new URL(`${name}/screen.weft`, corpus), "utf8");

export function parseStrict(markup: string): Document {
  const { document, diagnostics } = parse(markup, { catalog: coreCatalog, mode: "strict" });
  if (document === undefined || diagnostics.length > 0)
    throw new Error(`invalid test markup: ${JSON.stringify(diagnostics)}`);
  return document;
}

/** A fake file with the library and the screen built in it. */
export async function built(markup: string): Promise<{ figma: FakeFigma; frame: FakeFrame }> {
  const figma = new FakeFigma();
  const library = await ensureLibrary(figma, coreCatalog, tokens);
  const frame = (await buildScreen(figma, parseStrict(markup), {
    catalog: coreCatalog,
    library,
    tokens,
  })) as FakeFrame;
  return { figma, frame };
}

export const read = (figma: FakeFigma, frame: FakeFrame) =>
  readScreen(figma, frame, { catalog: coreCatalog, tokens });
