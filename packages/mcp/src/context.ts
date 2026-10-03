import type { Catalog } from "@weft/core";

/** What every tool needs to judge markup; the host decides, never the model. */
export type Context = {
  catalog: Catalog;
  /** Token path → DTCG `$type`. Token references are checked only when given (SPEC §6). */
  tokens?: ReadonlyMap<string, string> | undefined;
  /** Known host actions. Action names are checked only when given. */
  actions?: readonly string[] | undefined;
};

/**
 * Bounds on what a model can make the server chew on. Tool arguments are untrusted input; the
 * numbers are generous for real screens (the largest corpus screen is under 3,000 characters).
 */
export const LIMITS = {
  /** UTF-16 code units of one markup argument. */
  markupChars: 200_000,
  /** UTF-16 code units of the JSON text of the `data` argument of `weft_render`. */
  dataChars: 200_000,
  /** Patches in one `weft_patch` call. */
  patches: 100,
  /** UTF-16 code units of the JSON text of the whole patch list. */
  patchesChars: 200_000,
  /** Diagnostics returned in one result; the rest are counted, not listed. */
  diagnostics: 40,
  /** JSON values (objects, arrays, scalars) in one call's arguments, enforced by the SDK. */
  inputElements: 20_000,
} as const;
