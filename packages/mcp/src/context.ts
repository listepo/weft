import type { CatalogSource, KindSource, LimitName } from "@weft/catalog";
import type { Catalog, DataSchema, Mode } from "@weft/core";

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

/**
 * Default bounds on what a model can make the server chew on. Tool arguments are untrusted input; the
 * numbers are generous for real screens (the largest corpus screen is under 3,000 characters).
 */
export const LIMITS: Limits = {
  /** UTF-16 code units of one markup argument. */
  markupChars: 200_000,
  /** UTF-16 code units of the JSON text of the `data` argument of `weft_render`. */
  dataChars: 200_000,
  /** Patches in one `weft_patch` call. */
  patches: 100,
  /** UTF-16 code units of the JSON text of the whole patch list. */
  patchesChars: 200_000,
  /** UTF-16 code units of the JSON text of the `project` argument (SPEC §10). */
  projectChars: 500_000,
  /** Diagnostics returned in one result; the rest are counted, not listed. */
  diagnostics: 40,
  /** JSON values (objects, arrays, scalars) in one call's arguments, enforced by the SDK. */
  inputElements: 20_000,
};

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
