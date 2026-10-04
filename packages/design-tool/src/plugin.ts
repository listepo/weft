// A design-tool plugin's work, apart from the tool's global. Both tools run a plugin in two places:
// a sandbox that reaches the file but has no WebAssembly, and a UI iframe, which is a browser page
// without the file. The Weft core is WebAssembly, so the UI parses, canonicalizes, validates and
// serializes (`plugin-ui.ts`), and the sandbox only builds and reads layers (this file). Messages
// cross a boundary the sandbox does not control, so each one is validated.
import type { Diagnostic } from "@weft/core";
import { DocumentSchema, type Document } from "@weft/core";
import type { Token } from "@weft/catalog";
import type { Loss } from "@weft/from-aria";
import { z } from "zod";
import type { ReadResult } from "./read.ts";
import type { Display } from "./view.ts";

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

/** What the UI sends the sandbox. Tokens travel as entries, which every message channel keeps. */
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

/** The tool's side of a request: build into the file, or read the one selected layer. */
export type PluginTool<L> = {
  build(
    document: Document,
    display: Display,
    tokens: ReadonlyMap<string, Token>,
  ): Promise<{ readonly id: string }>;
  read(layer: L, tokens: ReadonlyMap<string, Token>): Promise<ReadResult>;
};

export async function handleRequest<L>(
  tool: PluginTool<L>,
  selection: readonly L[],
  message: unknown,
): Promise<PluginReply> {
  const request = RequestSchema.safeParse(message);
  if (!request.success)
    return { type: "error", message: "The plugin UI sent a malformed message." };
  const tokens = new Map<string, Token>(request.data.tokens);
  try {
    if (request.data.type === "build") {
      const root = await tool.build(request.data.document, request.data.display, tokens);
      return { type: "built", id: root.id };
    }
    const [layer, ...rest] = selection;
    if (layer === undefined || rest.length > 0)
      return { type: "error", message: "Select exactly one frame to export." };
    return { type: "exported", ...(await tool.read(layer, tokens)) };
  } catch (error) {
    // The file the plugin runs in can hold anything; a failure is reported, never left to hang the UI.
    return { type: "error", message: error instanceof Error ? error.message : String(error) };
  }
}
