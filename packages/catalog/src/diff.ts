// Classifies the difference between two catalog versions by the SPEC §8 rule, so that a catalog
// author learns whether the next version is a major, minor or no-op bump. Computed by the Rust
// catalog crate (crates/weft-catalog/src/diff.rs) through WebAssembly.
import type { Catalog } from "@weft/core";
import { toJson, wasm } from "@weft/core/wasm";

export type ChangeLevel = "none" | "minor" | "major";
export type CatalogChange = { path: string; level: ChangeLevel; message: string };
export type CatalogDiff = { level: ChangeLevel; changes: CatalogChange[] };

export function diffCatalogs(previous: Catalog, next: Catalog): CatalogDiff {
  return JSON.parse(wasm.diffCatalogs(toJson(previous), toJson(next))) as CatalogDiff;
}
