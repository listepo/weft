import type { CatalogSource, KindSource, LimitName } from "@weft/catalog";
import { coreCatalog } from "@weft/catalog";
import type { Catalog, DataSchema, Mode } from "@weft/core";
import { toolHost } from "@weft/core/tools";

/** What every tool needs to judge markup; the host decides, never the model. */
export type Context = {
  catalog: Catalog;
  /** The catalogs merged over the core, in load order (SPEC §10.4); absent when the host gives one catalog. */
  catalogs?: readonly CatalogSource[] | undefined;
  /** For every kind of `catalog`, the catalog that defined it and those that widened it. */
  kinds?: Readonly<Record<string, KindSource>> | undefined;
  /** Token path → DTCG `$type`. Token references are checked only when given (SPEC §6). */
  tokens?: ReadonlyMap<string, string> | undefined;
  /** Known host actions. Action names are checked only when given. */
  actions?: readonly string[] | undefined;
  /** The data model's schema. Bindings are checked against it only when given (SPEC §10.5). */
  data?: DataSchema | undefined;
};

export type Limits = Readonly<Record<LimitName, number>>;

// The tools, their texts and their bounds live in crates/weft-binding (`tools`), which the cox
// plugin calls too; these are read from there so the server keeps no copy.
const constants = toolHost({ catalog: coreCatalog }).constants();

/**
 * Default bounds on what a model can make the server chew on: tool arguments are untrusted
 * input. Lengths count UTF-16 code units of the argument or of its JSON text.
 */
export const LIMITS = constants.limits as Limits;
/** The primer `weft_primer` returns with the default limits. */
export const PRIMER = constants.primer;
/** Names `weft_capabilities` lists per category. */
export const LISTED = constants.listed;
/** Sent with every `weft_context` result, so no entry can pass for the tool's own words. */
export const CONTEXT_NOTICE = constants.contextNotice;

/**
 * How one server runs. Its host sets this (`weft-mcp --project`, SPEC §10.6); a tool argument,
 * `project` included, never does, so a model cannot lift its own bounds.
 */
export type ServerSettings = {
  limits: Limits;
  /** The default of `weft_validate`'s `strict`. Writers' tools always check strictly. */
  mode: Mode;
  /** Whether `weft_patch` may change context (`mcp.context`); `weft_context` reads either way. */
  context: "read-write" | "read-only";
};
