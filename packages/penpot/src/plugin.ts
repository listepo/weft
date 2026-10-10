// The plugin's sandbox work, apart from the `penpot` global: the shared request handler of
// @weft/design-tool with Penpot's build and read. The UI half is `buildRequest`, `exportRequest`
// and `finishExport` of the same package.
import type { Catalog } from "@weft/core";
import {
  contextReply,
  handleRequest as handleShared,
  type ContextLayer,
  type PluginReply,
} from "@weft/design-tool";
import type { PenpotApi, PShape, PSharedData } from "./api.ts";
import { buildScreen } from "./build.ts";
import { ensureLibrary } from "./library.ts";
import { dataOf } from "./layer.ts";
import { readThemes } from "./modes.ts";
import { readLayers } from "./read.ts";

/**
 * What the plugin is configured with. Extension point for weft.json: Penpot has no option of its
 * own yet, so the reserved `export.penpot` and `import.penpot` sections of the settings table
 * (`crates/weft-catalog/src/settings.rs`) stay empty. A new option is added there and here in the
 * same change.
 */
export type PluginOptions = { catalog: Catalog };

export function handleRequest(
  api: PenpotApi,
  selection: readonly PShape[],
  message: unknown,
  options: PluginOptions,
): Promise<PluginReply> {
  const { catalog } = options;
  return handleShared(
    {
      async build(document, display, tokens, modifier) {
        const library = await ensureLibrary(api, catalog, tokens, modifier);
        return buildScreen(api, document, { catalog, library, tokens, display });
      },
      read: (shape, tokens) => readLayers(shape, { catalog, tokens }),
      modes: async (tokens) =>
        readThemes(api.library.local.tokens, tokens, dataOf(api.library.local)),
    },
    selection,
    message,
  );
}

/** A shape and the shapes above it, as far as the context panel reads them. */
export type PContextShape = PSharedData & { readonly parent: PContextShape | null | undefined };

const contextLayer = (shape: PContextShape): ContextLayer => ({
  ...dataOf(shape),
  get parent(): ContextLayer | undefined {
    return shape.parent === null || shape.parent === undefined
      ? undefined
      : contextLayer(shape.parent);
  },
});

/** The `context` reply for the current selection: what the panel lists for the designer. */
export const selectionContext = (selection: readonly PContextShape[]): PluginReply =>
  contextReply(selection.map(contextLayer));
