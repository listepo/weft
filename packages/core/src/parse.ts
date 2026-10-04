// Markup → canonical JSON (SPEC §2–§4): the tokenizer checks syntax, the builder types literals
// by the catalog and lifts `<slot>` into `slots`, and validation adds the schema and semantic layers.
// The work runs in the Rust core (crates/weft-core/src/parse.rs) through WebAssembly.
import type { Catalog, Diagnostic, Document } from "./model.ts";
import { toSourceMap, type SourceMap, type WireSource } from "./source.ts";
import type { ValidateOptions } from "./validate.ts";
import { catalogHandle, options as wireOptions, wasm, wellFormed } from "./wasm.ts";

/** Without a catalog only the syntax layer runs and every literal stays a string. */
export type ParseOptions = Omit<ValidateOptions, "source" | "catalog"> & {
  catalog?: Catalog | undefined;
};

export type ParseResult = {
  /** Absent when the markup has syntax errors. */
  document?: Document | undefined;
  diagnostics: Diagnostic[];
  source?: SourceMap | undefined;
};

type Parsed = { document?: Document; diagnostics: Diagnostic[]; sources?: (WireSource | null)[] };

export function parse(markup: string, options: ParseOptions = {}): ParseResult {
  const text = wellFormed(markup);
  const wire = wireOptions(options);
  const out = JSON.parse(
    options.catalog === undefined
      ? wasm.parse(text, wire)
      : catalogHandle(options.catalog).parse(text, wire),
  ) as Parsed;
  const { document, diagnostics } = out;
  if (document === undefined) return { diagnostics };
  const result: ParseResult = { document, diagnostics };
  // Built on first read: most callers never look at positions.
  let source: SourceMap | undefined;
  Object.defineProperty(result, "source", {
    enumerable: true,
    get: () => (source ??= toSourceMap(document.root, out.sources ?? [])),
    set: (value: SourceMap | undefined) => {
      source = value;
    },
  });
  return result;
}
