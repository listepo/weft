// The plugin's work, apart from the `figma` global. Figma runs a plugin in two places: the main
// thread, which reaches the file but has no WebAssembly, and a UI iframe, which is a browser page
// without the file. The Weft core is WebAssembly, so the UI parses, canonicalizes, validates and
// serializes (`plugin-ui.ts`), and the main thread only builds and reads layers (this file).
// Messages cross a boundary the main thread does not control, so each one is validated.
import type { Diagnostic } from "@weft/core";
import { DocumentSchema, type Catalog, type Document } from "@weft/core";
import type { Token } from "@weft/catalog";
import type { Loss } from "@weft/from-aria";
import { z } from "zod";
import type { FigmaApi, FNode } from "./api.ts";
import { buildScreen } from "./build.ts";
import { ensureLibrary } from "./library.ts";
import { readLayers } from "./read.ts";

/** A .weft file larger than this is refused before parsing; real screens are a few kB. */
export const MAX_MARKUP = 1_000_000;

/** More tokens than a design system has; the bound keeps a bad message from flooding the file. */
export const MAX_TOKENS = 10_000;

const TokensSchema = z
  .array(z.tuple([z.string(), z.object({ type: z.string(), value: z.unknown() })]))
  .max(MAX_TOKENS);

const RequestSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("build"),
    document: DocumentSchema,
    display: z.record(z.string(), z.string()),
    tokens: TokensSchema,
  }),
  z.object({ type: z.literal("export"), tokens: TokensSchema }),
]);

/** What the UI sends the main thread. Tokens travel as entries, which every message channel keeps. */
export type PluginRequest = z.infer<typeof RequestSchema>;

export type PluginReply =
  | { type: "built"; id: string }
  | {
      type: "exported";
      /** As read from the layers: unfinished, typed text still raw (`finishExport` finishes it). */
      document: Document;
      losses: Loss[];
      diagnostics: Diagnostic[];
    }
  | { type: "error"; message: string; diagnostics?: Diagnostic[] | undefined };

export type PluginOptions = { catalog: Catalog };

export async function handleRequest(
  api: FigmaApi,
  selection: readonly FNode[],
  message: unknown,
  options: PluginOptions,
): Promise<PluginReply> {
  const request = RequestSchema.safeParse(message);
  if (!request.success)
    return { type: "error", message: "The plugin UI sent a malformed message." };
  const tokens = new Map<string, Token>(request.data.tokens);
  try {
    if (request.data.type === "build") {
      const library = await ensureLibrary(api, options.catalog, tokens);
      const frame = await buildScreen(api, request.data.document, {
        catalog: options.catalog,
        library,
        tokens,
        display: request.data.display,
      });
      return { type: "built", id: frame.id };
    }
    const [layer, ...rest] = selection;
    if (layer === undefined || rest.length > 0)
      return { type: "error", message: "Select exactly one frame to export." };
    const read = await readLayers(api, layer, { catalog: options.catalog, tokens });
    return { type: "exported", ...read };
  } catch (error) {
    // The file the plugin runs in can hold anything; a failure is reported, never left to hang the UI.
    return { type: "error", message: error instanceof Error ? error.message : String(error) };
  }
}
