// A catalog from a Custom Elements Manifest (SPEC §9, "From a Custom Elements Manifest"), by the
// importer of crates/weft-import. It runs in the web module, which carries the importers, so it is
// an entry of its own (`@weft/core/cem`): `@weft/core` itself keeps loading only the core module.
import { diagnostic } from "./diagnostics.ts";
import type { Loss } from "./loss.ts";
import { WEFT_VERSION, type Catalog, type Diagnostic } from "./model.ts";
import { catalogHandle, toJson, wellFormed } from "./web.ts";

export type CemOptions = {
  /** The catalog the result extends; an element whose tag it already has is left out. */
  catalog: Catalog;
  name: string;
  version: string;
};

export type CemImport = { catalog: Catalog; losses: Loss[]; diagnostics: Diagnostic[] };

/** Never throws on the manifest: unreadable input is `W601`, input over a limit is cut (`W602`). */
export function importCem(manifest: string, options: CemOptions): CemImport {
  if (typeof manifest !== "string") {
    const unread = diagnostic("W601", {
      path: "#",
      message: "The manifest is not a string of JSON.",
      expected: "the text of a Custom Elements Manifest",
    });
    const { name, version } = options;
    const catalog = { weft: WEFT_VERSION, name, version, components: {} };
    return { catalog, losses: [], diagnostics: [unread] };
  }
  const wire = toJson({ name: options.name, version: options.version }) ?? "{}";
  const out = catalogHandle(options.catalog).importCem(wellFormed(manifest), wire);
  return JSON.parse(out) as CemImport;
}
