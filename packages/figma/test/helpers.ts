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

/** A fake file with the library and the screen built in it. */
export async function built(markup: string): Promise<{ figma: FakeFigma; frame: FakeFrame }> {
  const figma = new FakeFigma();
  const library = await ensureLibrary(figma, coreCatalog, tokens);
  const document = parseStrict(markup);
  const frame = (await buildScreen(figma, document, {
    catalog: coreCatalog,
    library,
    tokens,
    display: displayTexts(document),
  })) as FakeFrame;
  return { figma, frame };
}

export const read = (figma: FakeFigma, frame: FakeFrame) =>
  readScreen(figma, frame, { catalog: coreCatalog, tokens });
