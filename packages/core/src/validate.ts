// Schema and semantic layers of SPEC §6, plus the compatibility rules of §8. Validation reports;
// it never throws and never changes the document. The checks run in the Rust core
// (crates/weft-core/src/validate.rs) through WebAssembly.
import type { Mode } from "./diagnostics.ts";
import type { Catalog, Diagnostic } from "./model.ts";
import { fromSourceMap, type SourceMap } from "./source.ts";
import { catalogHandle, options as wireOptions, toJson } from "./wasm.ts";

export type ValidateOptions = {
  catalog: Catalog;
  /** `lenient` (default) warns about unknown content; `strict` rejects it (SPEC §8). */
  mode?: Mode | undefined;
  /** Token path → DTCG `$type`. Token references are checked only when this is given. */
  tokens?: ReadonlyMap<string, string> | undefined;
  /** Known host actions. Action names are checked only when this is given. */
  actions?: readonly string[] | undefined;
  /** Positions from `parse`, so that diagnostics carry line and column. */
  source?: SourceMap | undefined;
};

export function validate(input: unknown, options: ValidateOptions): Diagnostic[] {
  const json = toJson(input);
  const wire = wireOptions(options);
  const sources =
    options.source === undefined || json === undefined
      ? undefined
      : JSON.stringify(fromSourceMap(input, options.source));
  return JSON.parse(catalogHandle(options.catalog).validate(json, wire, sources)) as Diagnostic[];
}
