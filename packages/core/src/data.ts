// The data schema of SPEC §10.5: a JSON Schema (2020-12) subset, and a pass that checks every
// binding of a document against it. It is a pass of its own rather than a `validate` option so
// that the option types every binding constructs stay as they are; callers run it after `parse`
// or `validate`. The schema is untrusted: nothing here throws. Both run in the Rust core
// (crates/weft-core/src/data.rs) through WebAssembly.
import type { Catalog, Diagnostic, Document } from "./model.ts";
import { fromSourceMap, type SourceMap } from "./source.ts";
import { catalogHandle, toJson, wasm } from "./wasm.ts";

/**
 * A compiled data schema. Opaque: it carries the schema as JSON text, which the Rust core reads
 * again on every check, so the schema never has to cross the boundary in a second form.
 */
export type DataSchema = { readonly json: string | undefined };

export type DataSchemaProblem = {
  code: "W709" | "W710";
  /** JSON Pointer into the schema, "" for its root. */
  pointer: string;
  message: string;
};

export type DataCheckOptions = {
  catalog: Catalog;
  data: DataSchema;
  /** Positions from `parse`, so that diagnostics carry line and column. */
  source?: SourceMap | undefined;
};

export function compileDataSchema(json: unknown): {
  schema: DataSchema;
  problems: DataSchemaProblem[];
} {
  const text = toJson(json);
  const problems = JSON.parse(wasm.compileDataSchema(text)) as DataSchemaProblem[];
  return { schema: { json: text }, problems };
}

export function checkData(document: Document, options: DataCheckOptions): Diagnostic[] {
  const json = toJson(document);
  const sources =
    options.source === undefined || json === undefined
      ? undefined
      : JSON.stringify(fromSourceMap(document, options.source));
  return JSON.parse(
    catalogHandle(options.catalog).checkData(json, options.data.json, sources),
  ) as Diagnostic[];
}
