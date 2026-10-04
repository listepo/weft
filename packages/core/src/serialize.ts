// Canonical markup (SPEC §3): the serializer emits exactly one text per canonical document.
// Written by the Rust core (crates/weft-core/src/serialize.rs).
import type { Document } from "./model.ts";
import { toJson, wasm } from "./wasm.ts";

export function serialize(document: Document): string {
  return wasm.serialize(toJson(document));
}
