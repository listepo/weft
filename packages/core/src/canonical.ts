// Canonical JSON (SPEC §3): one byte sequence per document, so that tools can diff and hash.
// Computed by the Rust core (crates/weft-core/src/canonical.rs).
import type { Document } from "./model.ts";
import { toJson, wasm } from "./wasm.ts";

export function canonicalize(document: Document): Document {
  return JSON.parse(wasm.canonicalize(toJson(document))) as Document;
}

/** Canonical JSON text: two-space indentation and a final newline. */
export function stringify(document: Document): string {
  return wasm.stringify(toJson(document));
}
