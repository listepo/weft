// Patches (SPEC §7): an agent edits a document by id instead of rewriting it. The whole list is
// applied to a copy and validated, so a rejected list leaves no trace and the caller's document
// is never touched. The edits run in the Rust core (crates/weft-core/src/patch.rs).
import type { Diagnostic, Document } from "./model.ts";
import type { ValidateOptions } from "./validate.ts";
import { catalogHandle, options as wireOptions, toJson } from "./wasm.ts";

export type ApplyOptions = Omit<ValidateOptions, "source">;

export type PatchResult = {
  /** Canonical result; absent when anything was rejected. Holds no errors, possibly warnings. */
  document?: Document | undefined;
  diagnostics: Diagnostic[];
};

export function applyPatches(
  document: Document,
  patches: unknown,
  options: ApplyOptions,
): PatchResult {
  const out = catalogHandle(options.catalog).applyPatches(
    toJson(document),
    toJson(patches),
    wireOptions(options),
  );
  return JSON.parse(out) as PatchResult;
}
