// Rendered HTML → Weft (SPEC §9, "From a running UI"): pages from a Weft renderer, whose
// `data-weft-id` attributes give the ids back, or any semantic HTML. The importer runs in Rust
// (crates/weft-web/src/dom.rs), which parses with the WHATWG tree builder, so a page is read as a
// browser builds it. The HTML is untrusted: it is only parsed, never run, and size, depth and node
// count are bounded.
import { htmlImport, importFailure, instanceIdOf, tables } from "./engine.ts";
import type { ImportOptions, ImportResult } from "./types.ts";

/** In UTF-16 code units; longer input is cut there and reported (W602). */
export const MAX_HTML_LENGTH: number = tables.maxHtmlLength;

// HTML-AAM implicit roles of the elements user interfaces commonly use. Elements not listed (and
// the text-level ones such as strong or code) are generic: their content is read in place.
export const IMPLICIT_ROLES: Readonly<Record<string, string>> = Object.freeze(tables.implicitRoles);

// `<input>` roles by `type`; any other type (text, email, password, tel, url, …) is a textbox.
export const INPUT_ROLES: Readonly<Record<string, string>> = Object.freeze(tables.inputRoles);

// The id a rendered instance `id[2][0]` gets in the import: the indexes appended with hyphens.
export function instanceId(raw: string): string | undefined {
  return instanceIdOf(raw);
}

export function fromDom(html: string, options: ImportOptions): ImportResult {
  if (typeof html !== "string")
    return importFailure("The input is not a string of HTML.", "HTML text");
  try {
    return htmlImport(html, options.catalog);
  } catch (error) {
    // Only a malformed catalog can get here; any HTML string parses.
    return importFailure(
      "The HTML could not be mapped with this catalog.",
      "a catalog with the shape of SPEC §5",
      error instanceof Error ? error.message.slice(0, 200) : undefined,
    );
  }
}
