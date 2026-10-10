// A design-tool plugin's work, apart from the tool's global. Both tools run a plugin in two places:
// a sandbox that reaches the file but has no WebAssembly, and a UI iframe, which is a browser page
// without the file. The Weft core is WebAssembly, so the UI parses, canonicalizes, validates and
// serializes (`plugin-ui.ts`), and the sandbox only builds and reads layers (this file). Messages
// cross a boundary the sandbox does not control, so each one is validated.
import type { Diagnostic } from "@weft/core";
import { DocumentSchema, type Document, type Entry } from "@weft/core";
import type { Token, TokenModifier } from "@weft/catalog";
import { MAX_DEPTH, type Loss } from "@weft/from-aria";
import { z } from "zod";
import { readContext, readSource, type PluginData, type Source } from "./keys.ts";
import { MAX_CONTEXTS, modifierOf } from "./modes.ts";
import type { ReadResult } from "./read.ts";
import type { Display } from "./view.ts";

/** A .weft file larger than this is refused before parsing; real screens are a few kB. */
export const MAX_MARKUP = 1_000_000;

/** More tokens than a design system has; the bound keeps a bad message from flooding the file. */
export const MAX_TOKENS = 10_000;

const TokensSchema = z
  .array(z.tuple([z.string(), z.object({ type: z.string(), value: z.unknown() })]))
  .max(MAX_TOKENS);

/** A resolver modifier (SPEC §10.3) whose contexts become the tool's modes. */
const ModifierSchema = z.object({
  name: z.string(),
  default: z.string(),
  contexts: z
    .array(z.tuple([z.string(), TokensSchema]))
    .min(1)
    .max(MAX_CONTEXTS),
});

const RequestSchema = z.discriminatedUnion("type", [
  z.object({
    type: z.literal("build"),
    document: DocumentSchema,
    display: z.record(z.string(), z.string()),
    tokens: TokensSchema,
    modifier: ModifierSchema.optional(),
  }),
  z.object({ type: z.literal("export"), tokens: TokensSchema }),
]);

/** What the UI sends the sandbox. Tokens travel as entries, which every message channel keeps. */
export type PluginRequest = z.infer<typeof RequestSchema>;

export type PluginReply =
  | {
      type: "built";
      id: string;
      /** What the build skipped without failing, such as modes the file's plan does not allow. */
      notes?: string[] | undefined;
    }
  | {
      type: "exported";
      /** As read from the layers: unfinished, typed text still raw (`finishExport` finishes it). */
      document: Document;
      losses: Loss[];
      diagnostics: Diagnostic[];
      /** The file's token modes as a DTCG resolver document, when it has more than one mode. */
      resolver?: Record<string, unknown> | undefined;
    }
  | { type: "error"; message: string; diagnostics?: Diagnostic[] | undefined }
  | {
      /** The context entries about the selected layer, sent whenever the selection changes. */
      type: "context";
      entries: Entry[];
    };

/** A layer as the context panel walks it: its plugin data and the layer that holds it. */
export type ContextLayer = PluginData & { readonly parent: ContextLayer | undefined };

/**
 * The entries about one selected layer, read-only, for the designer's panel: the screen's own on
 * the root, else those whose `for` is the nearest Weft element at or above the layer. The walk
 * stops at the root that keeps the context; a layer outside any built screen has none.
 */
export function layerContext(layer: ContextLayer): Entry[] {
  let element: Source | undefined;
  let at: ContextLayer | undefined = layer;
  for (let depth = 0; at !== undefined && depth <= MAX_DEPTH; depth++, at = at.parent) {
    const context = readContext(at);
    if (context !== undefined) {
      if (element === undefined) return context.filter((e) => e.for === undefined);
      const { id } = element;
      return id === undefined ? [] : context.filter((e) => e.for === id);
    }
    element ??= readSource(at);
  }
  return [];
}

/** The `context` reply for a selection: entries only when exactly one layer is selected. */
export function contextReply(selection: readonly ContextLayer[]): PluginReply {
  const [layer, ...rest] = selection;
  return {
    type: "context",
    entries: layer === undefined || rest.length > 0 ? [] : layerContext(layer),
  };
}

/**
 * The tool's side of a request: build into the file, or read the one selected layer and, when
 * the tool keeps token modes, the modes as a resolver document (`modes`).
 */
export type PluginTool<L> = {
  build(
    document: Document,
    display: Display,
    tokens: ReadonlyMap<string, Token>,
    modifier?: TokenModifier,
  ): Promise<{ readonly id: string; readonly notes?: readonly string[] }>;
  read(layer: L, tokens: ReadonlyMap<string, Token>): Promise<ReadResult>;
  modes?(tokens: ReadonlyMap<string, Token>): Promise<Record<string, unknown> | undefined>;
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
      const { document, display, modifier } = request.data;
      const root = await tool.build(
        document,
        display,
        tokens,
        modifier === undefined ? undefined : modifierOf(modifier),
      );
      return root.notes === undefined || root.notes.length === 0
        ? { type: "built", id: root.id }
        : { type: "built", id: root.id, notes: [...root.notes] };
    }
    const [layer, ...rest] = selection;
    if (layer === undefined || rest.length > 0)
      return { type: "error", message: "Select exactly one frame to export." };
    const read = await tool.read(layer, tokens);
    const resolver = await tool.modes?.(tokens);
    return resolver === undefined
      ? { type: "exported", ...read }
      : { type: "exported", ...read, resolver };
  } catch (error) {
    // The file the plugin runs in can hold anything; a failure is reported, never left to hang the UI.
    return { type: "error", message: error instanceof Error ? error.message : String(error) };
  }
}
