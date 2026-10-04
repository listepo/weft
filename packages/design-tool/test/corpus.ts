// What the round-trip tests of every design tool read: the corpus screens and the default tokens.
// Shared so each tool package tests against the same inputs.
import { readdirSync, readFileSync } from "node:fs";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { parse, type Document } from "@weft/core";

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
