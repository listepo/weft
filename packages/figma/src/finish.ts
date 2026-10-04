// The steps of a read that need the WebAssembly core. They run wherever the core does: in Node,
// and in the plugin's UI iframe, never in Figma's main thread.
import { tokenTypes, type Token } from "@weft/catalog";
import { canonicalize, validate, type Catalog, type Document } from "@weft/core";
import type { FigmaApi, FNode } from "./api.ts";
import { readLayers, type ReadOptions, type ReadResult } from "./read.ts";

/** Canonicalizes what `readLayers` returned and adds the validation diagnostics. */
export function finishRead(
  read: ReadResult,
  options: { catalog: Catalog; tokens?: ReadonlyMap<string, Token> | undefined },
): ReadResult {
  const document: Document = canonicalize(read.document);
  const tokens = options.tokens === undefined ? undefined : tokenTypes(new Map(options.tokens));
  const diagnostics = [
    ...read.diagnostics,
    ...validate(document, { catalog: options.catalog, tokens }),
  ];
  return { document, losses: read.losses, diagnostics };
}

/** Reads a layer back into a canonical, validated Weft document. */
export async function readScreen(
  api: FigmaApi,
  layer: FNode,
  options: ReadOptions,
): Promise<ReadResult> {
  return finishRead(await readLayers(api, layer, options), options);
}
