import { parseA2ui } from "./adapters/a2ui.ts";
import { parseHtml } from "./adapters/html.ts";
import { parseJsx } from "./adapters/jsx.ts";
import type { Parsed } from "./adapters/types.ts";
import { parseWeft, parseWeftJson } from "./adapters/weft.ts";
import type { CorpusFormat, Format } from "./neutral.ts";
import { coreCatalog } from "@weft/catalog";
import { documentSchema } from "@weft/core/document-schema";

export const FILE_NAME: Record<CorpusFormat, string> = {
  weft: "screen.weft",
  html: "screen.html",
  jsx: "screen.jsx",
  a2ui: "screen.a2ui.json",
};

export const parsers: Record<Format, (src: string) => Parsed> = {
  weft: parseWeft,
  html: parseHtml,
  jsx: parseJsx,
  a2ui: parseA2ui,
  "weft-json": parseWeftJson,
  "weft-json-constrained": parseWeftJson,
};

export const FENCE_LANG: Record<Format, string> = {
  weft: "xml",
  html: "html",
  jsx: "jsx",
  a2ui: "json",
  "weft-json": "json",
  "weft-json-constrained": "json",
};

let schema: object | undefined;

/**
 * The schema a format's document replies are held to: the core catalog's document schema for
 * `weft-json-constrained`, nothing for the rest. Built once; the generator is deterministic.
 */
export function schemaFor(format: Format): object | undefined {
  if (format !== "weft-json-constrained") return undefined;
  return (schema ??= JSON.parse(documentSchema(coreCatalog)) as object);
}

/** Models usually wrap the document in a code fence; take its body, else the whole reply. */
export function extractDocument(reply: string): string {
  const fenced = /```[\w-]*\n([\s\S]*?)```/.exec(reply);
  return (fenced ? (fenced[1] as string) : reply).trim();
}
