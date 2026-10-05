// The UI iframe's half of the plugin (see `plugin.ts`): the steps that need the WebAssembly core.
import type { Token, TokenModifier } from "@weft/catalog";
import { parse, serialize, type Catalog, type Diagnostic } from "@weft/core";
import type { Loss } from "@weft/from-aria";
import { displayTexts, finishRead } from "./finish.ts";
import { modifierEntries } from "./modes.ts";
import { MAX_MARKUP, type PluginReply, type PluginRequest } from "./plugin.ts";

export type UiOptions = {
  catalog: Catalog;
  tokens: ReadonlyMap<string, Token>;
  /**
   * The resolver modifier whose contexts become the file's modes (SPEC §10.3), such as the
   * project's appearance modifier. Without it the file has one mode, as before.
   */
  modifier?: TokenModifier | undefined;
};

/** The build request for pasted markup, or the reason there is none. */
export function buildRequest(
  markup: string,
  options: UiOptions,
): { request?: PluginRequest; diagnostics: Diagnostic[]; message?: string } {
  if (markup.length > MAX_MARKUP)
    return { diagnostics: [], message: `The file is larger than ${MAX_MARKUP} characters.` };
  const { document, diagnostics } = parse(markup, { catalog: options.catalog });
  if (document === undefined)
    return { diagnostics, message: "The markup has errors; nothing was built." };
  const request: PluginRequest = {
    type: "build",
    document,
    display: displayTexts(document),
    tokens: [...options.tokens],
    ...(options.modifier === undefined ? {} : { modifier: modifierEntries(options.modifier) }),
  };
  return { request, diagnostics };
}

export function exportRequest(options: UiOptions): PluginRequest {
  return { type: "export", tokens: [...options.tokens] };
}

export type Exported = {
  fileName: string;
  markup: string;
  losses: Loss[];
  diagnostics: Diagnostic[];
  /** The file's token modes as a resolver document (`tokens.resolver.json`), when it has them. */
  resolver?: string | undefined;
};

/** Turns the main thread's raw read into `.weft` markup. */
export function finishExport(
  reply: Extract<PluginReply, { type: "exported" }>,
  options: UiOptions,
): Exported {
  const read = finishRead(reply, options);
  return {
    fileName: `${read.document.root.id ?? "screen"}.weft`,
    markup: serialize(read.document),
    losses: read.losses,
    diagnostics: read.diagnostics,
    ...(reply.resolver === undefined
      ? {}
      : { resolver: `${JSON.stringify(reply.resolver, null, 2)}\n` }),
  };
}
