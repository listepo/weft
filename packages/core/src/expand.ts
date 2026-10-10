// Expansion of fragment uses (SPEC §10.7): what a `<use>` means, for renderers and generators. The
// canonical document keeps its uses; expansion builds a separate document in the Rust core
// (crates/weft-core/src/expand.rs).
import type { Catalog, Diagnostic, Document } from "./model.ts";
import { catalogHandle, toJson } from "./wasm.ts";

export type Expanded = {
  /**
   * `document` with each use of a known fragment replaced by the fragment's body.
   * Absent when the input cannot be read (`W200`): a placeholder would be a blank screen.
   */
  document?: Document | undefined;
  /** `W805` (a cycle) and `W806` (past the element or depth limit), at the use leading to them. */
  diagnostics: Diagnostic[];
};

export function expand(document: Document, catalog: Catalog): Expanded {
  return JSON.parse(catalogHandle(catalog).expand(toJson(document))) as Expanded;
}
