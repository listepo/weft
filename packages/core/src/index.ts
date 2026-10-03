export * from "./model.ts";
export { canonicalize, stringify } from "./canonical.ts";
export {
  DIAGNOSTIC_CODES,
  diagnostic,
  didYouMean,
  hasErrors,
  type DiagnosticCode,
  type Mode,
  type Position,
  type Severity,
} from "./diagnostics.ts";
export { applyPatches, type ApplyOptions, type PatchResult } from "./patch.ts";
export { parse, type ParseOptions, type ParseResult } from "./parse.ts";
export { catalogJsonSchema, documentJsonSchema } from "./schema.ts";
export {
  ACTION,
  ARIA_ROLES,
  BINDING,
  EMBEDDED_REFERENCE,
  ID,
  NON_XML_CHAR,
  TOKEN,
} from "./rules.ts";
export { serialize } from "./serialize.ts";
export type { ListSource, NodeSource, SourceMap } from "./source.ts";
export { validate, type ValidateOptions } from "./validate.ts";
