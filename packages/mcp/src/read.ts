import { checkData, hasErrors, parse, type Diagnostic, type Document, type Mode } from "@weft/core";
import type { Context } from "./context.ts";

/**
 * Markup → document with the context's catalog, its bindings checked against the data schema when
 * there is one; `document` is absent when the syntax is broken.
 */
export function readMarkup(
  markup: string,
  context: Context,
  mode: Mode,
): { document?: Document | undefined; diagnostics: Diagnostic[]; ok: boolean } {
  const { data, ...options } = context;
  const parsed = parse(markup, { ...options, mode });
  const { document } = parsed;
  const diagnostics =
    document === undefined || data === undefined
      ? parsed.diagnostics
      : [
          ...parsed.diagnostics,
          ...checkData(document, { catalog: context.catalog, data, source: parsed.source }),
        ];
  return { document, diagnostics, ok: document !== undefined && !hasErrors(diagnostics) };
}
