// JSON Schema exports for tools that do not run TypeScript, generated from the shared Zod model.
import { z } from "zod";
import { CatalogSchema, DocumentSchema } from "./model.ts";

export function catalogJsonSchema() {
  return z.toJSONSchema(CatalogSchema);
}

export function documentJsonSchema() {
  return z.toJSONSchema(DocumentSchema);
}
