// One attribute value at a time (SPEC §2.1), for tools that read or show values outside markup,
// such as text a designer types in Figma. The rules are the Rust core's (crates/weft-core/src/values.rs).
import type { Value } from "./model.ts";
import { toJson, wasm } from "./wasm.ts";

export type ReadValue = { ok: true; value: Value } | { ok: false; message: string; hint: string };

/** Reads a value as written in an attribute (decoded, before typing by the catalog). */
export function readValue(raw: string): ReadValue {
  return JSON.parse(wasm.readValue(raw)) as ReadValue;
}

/** Writes a value as attribute text, before XML escaping. */
export function formatValue(value: Value): string {
  return wasm.formatValue(toJson(value));
}
