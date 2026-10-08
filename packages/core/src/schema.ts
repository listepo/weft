// JSON Schema exports for tools that do not run TypeScript, generated from the shared Zod model.
// They know no catalog: `documentJsonSchema()` admits any kind and any prop, the shape of §3 alone.
// The schema a constrained writer needs, built from a catalog (SPEC §3.1), is `documentSchema` of
// `@weft/core/document-schema`.
import { z } from "zod";
import { CatalogSchema, DocumentSchema } from "./model.ts";

export function catalogJsonSchema() {
  return z.toJSONSchema(CatalogSchema);
}

export function documentJsonSchema() {
  return z.toJSONSchema(DocumentSchema);
}
