import { coreCatalog } from "@weft/catalog";
import { buildScreen, displayTexts, ensureLibrary, readScreen } from "../src/index.ts";
import { parseStrict, tokens } from "../../design-tool/test/corpus.ts";
import { FakeFigma, type FakeFrame } from "./fake-figma.ts";

export {
  contextMarkup,
  corpusMarkup,
  corpusNames,
  parseStrict,
  tokens,
} from "../../design-tool/test/corpus.ts";

/**
 * Known gaps of the Figma build and read, found by the corpus screens. The builder keeps each
 * element's source but not the document's fragment definitions, so a screen that places a
 * fragment (`<use>`) reads back as uses with `W801`. Remove an entry when the gap is fixed.
 */
export const FIGMA_FRAGMENT_GAPS = new Set(["receipt"]);

/** A fake file with the library and the screen built in it. */
export async function built(markup: string): Promise<{ figma: FakeFigma; frame: FakeFrame }> {
  const figma = new FakeFigma();
  const library = await ensureLibrary(figma, coreCatalog, tokens);
  const document = parseStrict(markup);
  const { root: frame } = await buildScreen(figma, document, {
    catalog: coreCatalog,
    library,
    tokens,
    display: displayTexts(document),
  });
  return { figma, frame: frame as FakeFrame };
}

export const read = (figma: FakeFigma, frame: FakeFrame) =>
  readScreen(figma, frame, { catalog: coreCatalog, tokens });
