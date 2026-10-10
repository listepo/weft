import { coreCatalog } from "@weft/catalog";
import { buildScreen, displayTexts, ensureLibrary, readScreen } from "../src/index.ts";
import { parseStrict, tokens } from "../../design-tool/test/corpus.ts";
import { FakePenpot, type FakeBoard } from "./fake-penpot.ts";

export {
  contextMarkup,
  corpusMarkup,
  corpusNames,
  parseStrict,
  tokens,
} from "../../design-tool/test/corpus.ts";

/** A fake file with the library and the screen built in it. */
export async function built(markup: string): Promise<{ penpot: FakePenpot; root: FakeBoard }> {
  const penpot = new FakePenpot();
  const library = await ensureLibrary(penpot, coreCatalog, tokens);
  const document = parseStrict(markup);
  const root = await buildScreen(penpot, document, {
    catalog: coreCatalog,
    library,
    tokens,
    display: displayTexts(document),
  });
  return { penpot, root: root as FakeBoard };
}

export const read = (root: FakeBoard) => readScreen(root, { catalog: coreCatalog, tokens });
