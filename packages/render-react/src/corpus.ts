// Reads the reference screens of `corpus/` (see corpus/README.md) for the corpus tests and the
// gallery. Not part of the package API: it reads files of this repository.
import { readdirSync, readFileSync } from "node:fs";
import { coreCatalog } from "@weft/catalog";
import { parse, type Diagnostic, type Document } from "@weft/core";

export const CORPUS_DIR = new URL("../../../corpus/", import.meta.url);

export type CorpusScreen = {
  name: string;
  document: Document | undefined;
  diagnostics: Diagnostic[];
  data: unknown;
};

export function corpusScreens(): CorpusScreen[] {
  return readdirSync(CORPUS_DIR, { withFileTypes: true })
    .filter((e) => e.isDirectory())
    .map((e) => e.name)
    .sort()
    .map((name) => {
      const read = (file: string) => readFileSync(new URL(`${name}/${file}`, CORPUS_DIR), "utf8");
      const { document, diagnostics } = parse(read("screen.weft"), {
        catalog: coreCatalog,
        mode: "strict",
      });
      return { name, document, diagnostics, data: JSON.parse(read("data.json")) as unknown };
    });
}
