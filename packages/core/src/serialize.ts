// Canonical markup (SPEC §3): the serializer emits exactly one text per canonical document.
// Written by the Rust core (crates/weft-core/src/serialize.rs).
import type { Document } from "./model.ts";
import { toJson, wasm } from "./wasm.ts";

export function serialize(document: Document): string {
  // A cycle or a value past the depth limit is the W200 list (SPEC §3), not markup of an empty screen.
  return wasm.serialize(toJson(document));
}
