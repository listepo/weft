// The plugin's main-thread work, apart from the `figma` global: the shared request handler of
// @weft/design-tool with Figma's build and read. The UI half is `buildRequest`, `exportRequest`
// and `finishExport` of the same package.
import type { Catalog } from "@weft/core";
import { handleRequest as handleShared, type PluginReply } from "@weft/design-tool";
import type { FigmaApi, FNode } from "./api.ts";
import { buildScreen } from "./build.ts";
import { ensureLibrary } from "./library.ts";
import { readModes } from "./modes.ts";
import { readLayers } from "./read.ts";

export type PluginOptions = { catalog: Catalog };

export function handleRequest(
  api: FigmaApi,
  selection: readonly FNode[],
  message: unknown,
  options: PluginOptions,
): Promise<PluginReply> {
  const { catalog } = options;
  return handleShared(
    {
      async build(document, display, tokens, modifier) {
        const library = await ensureLibrary(api, catalog, tokens, modifier);
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
