// The plugin's work, apart from the `figma` global: the plugin's main code passes in the API, the
// selection and each message its UI sends, and posts the reply back. The UI is a web page the
// plugin does not control, so every message is validated before anything is built from it.
import type { Catalog, Diagnostic } from "@weft/core";
import { parse, serialize } from "@weft/core";
import type { Token } from "@weft/catalog";
import type { Loss } from "@weft/from-aria";
import { z } from "zod";
import type { FigmaApi, FNode } from "./api.ts";
import { buildScreen } from "./build.ts";
import { ensureLibrary } from "./library.ts";
import { readScreen } from "./read.ts";

/** A .weft file larger than this is refused before parsing; real screens are a few kB. */
export const MAX_MARKUP = 1_000_000;

const RequestSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("build"), markup: z.string().max(MAX_MARKUP) }),
  z.object({ type: z.literal("export") }),
]);

export type PluginRequest = z.infer<typeof RequestSchema>;

export type PluginReply =
  | { type: "built"; id: string; diagnostics: Diagnostic[] }
  | {
      type: "exported";
      fileName: string;
      markup: string;
      losses: Loss[];
      diagnostics: Diagnostic[];
    }
  | { type: "error"; message: string; diagnostics?: Diagnostic[] | undefined };

export type PluginOptions = {
  catalog: Catalog;
  tokens: ReadonlyMap<string, Token>;
};

export async function handleRequest(
  api: FigmaApi,
  selection: readonly FNode[],
  message: unknown,
  options: PluginOptions,
): Promise<PluginReply> {
  const request = RequestSchema.safeParse(message);
  if (!request.success)
    return { type: "error", message: "The plugin UI sent a malformed message." };
  try {
    if (request.data.type === "build") {
      const { document, diagnostics } = parse(request.data.markup, { catalog: options.catalog });
      if (document === undefined)
        return { type: "error", message: "The markup has errors; nothing was built.", diagnostics };
      const library = await ensureLibrary(api, options.catalog, options.tokens);
      const frame = await buildScreen(api, document, { ...options, library });
      return { type: "built", id: frame.id, diagnostics };
    }
    const [layer, ...rest] = selection;
    if (layer === undefined || rest.length > 0)
      return { type: "error", message: "Select exactly one frame to export." };
    const result = await readScreen(api, layer, options);
    const root = result.document.root;
    return {
      type: "exported",
      fileName: `${root.id ?? "screen"}.weft`,
      markup: serialize(result.document),
      losses: result.losses,
      diagnostics: result.diagnostics,
    };
  } catch (error) {
    // The file the plugin runs in can hold anything; a failure is reported, never left to hang the UI.
    return { type: "error", message: error instanceof Error ? error.message : String(error) };
  }
}
