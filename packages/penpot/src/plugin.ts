// The plugin's sandbox work, apart from the `penpot` global: the shared request handler of
// @weft/design-tool with Penpot's build and read. The UI half is `buildRequest`, `exportRequest`
// and `finishExport` of the same package.
import type { Catalog } from "@weft/core";
import { handleRequest as handleShared, type PluginReply } from "@weft/design-tool";
import type { PenpotApi, PShape } from "./api.ts";
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
