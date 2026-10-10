// The plugin's main-thread work, apart from the `figma` global: the shared request handler of
// @weft/design-tool with Figma's build and read. The UI half is `buildRequest`, `exportRequest`
// and `finishExport` of the same package.
import type { Catalog } from "@weft/core";
import {
  contextReply,
  handleRequest as handleShared,
  type ContextLayer,
  type LibrarySources,
  type PluginReply,
} from "@weft/design-tool";
import type { FigmaApi, FNode, FPluginData } from "./api.ts";
import { buildScreen } from "./build.ts";
import { dataOf } from "./data.ts";
import { ensureLibrary } from "./library.ts";
import { readModes } from "./modes.ts";
import { readLayers } from "./read.ts";

/** `sources` names the catalogs merged into `catalog`, so the library groups its kinds by catalog. */
export type PluginOptions = { catalog: Catalog; sources?: LibrarySources | undefined };

export function handleRequest(
  api: FigmaApi,
  selection: readonly FNode[],
  message: unknown,
  options: PluginOptions,
): Promise<PluginReply> {
  const { catalog, sources } = options;
  return handleShared(
    {
      async build(document, display, tokens, modifier) {
        const library = await ensureLibrary(api, catalog, tokens, modifier, sources);
        const root = await buildScreen(api, document, { catalog, library, tokens, display });
        return { id: root.id, notes: library.notes };
      },
      read: (layer, tokens) => readLayers(api, layer, { catalog, tokens }),
      modes: (tokens) => readModes(api, tokens),
    },
    selection,
    message,
  );
}

/** A node and the nodes above it, as far as the context panel reads them. */
export type FContextNode = FPluginData & { readonly parent: FContextNode | null | undefined };

const contextLayer = (node: FContextNode): ContextLayer => ({
  ...dataOf(node),
  get parent(): ContextLayer | undefined {
    return node.parent === null || node.parent === undefined
      ? undefined
      : contextLayer(node.parent);
  },
});

/** The `context` reply for the current selection: what the panel lists for the designer. */
export const selectionContext = (selection: readonly FContextNode[]): PluginReply =>
  contextReply(selection.map(contextLayer));
