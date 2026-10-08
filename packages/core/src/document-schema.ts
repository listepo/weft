// The JSON Schema of the canonical documents a catalog admits (SPEC §3.1), by the generator of
// crates/weft-catalog. It runs in the web module, which carries the generator, so it is an entry of
// its own (`@weft/core/document-schema`): `@weft/core` itself keeps loading only the core module,
// which the generator would grow by about 110 KB for a feature no browser validation needs.
import type { Catalog } from "./model.ts";
import { catalogHandle } from "./web.ts";

/**
 * The schema of the documents `catalog` admits, as compact JSON text: byte for byte what the Rust
 * generator writes and the MCP tool `weft_schema` returns (`weft schema` prints the same schema
 * indented). Text, not an object: `JSON.stringify` would write a bound such as `1.0` as `1`.
 * Throws when `catalog` is not a catalog. Unlike `documentJsonSchema()` of `@weft/core`, which
 * knows no catalog and admits any kind and prop, this one admits only what the catalog declares.
 */
export function documentSchema(catalog: Catalog): string {
  return catalogHandle(catalog).documentSchema();
}
