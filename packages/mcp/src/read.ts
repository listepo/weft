import { hasErrors, parse, type Diagnostic, type Document, type Mode } from "@weft/core";
import type { Context } from "./context.ts";

/** Markup → document with the host's catalog; `document` is absent when the syntax is broken. */
export function readMarkup(
  markup: string,
  context: Context,
  mode: Mode,
): { document?: Document | undefined; diagnostics: Diagnostic[]; ok: boolean } {
  const { document, diagnostics } = parse(markup, { ...context, mode });
  return { document, diagnostics, ok: document !== undefined && !hasErrors(diagnostics) };
}
