// Weft document → a self-contained React or SolidJS component (SPEC §9, "To JSX"), generated in
// Rust (crates/weft-web/src/jsx) through the web WebAssembly module. This file keeps the package's
// API; the mapping, its safety rules and the runtime helpers live in the crate.
import { catalogHandle, toJson, wasm } from "@weft/core/web";
import type { Catalog, Document } from "@weft/core";

export type ToJsxOptions = {
  catalog: Catalog;
  componentName?: string;
  /** React (the default) or SolidJS. */
  framework?: "react" | "solid";
  /** TSX: typed props and helpers. */
  typescript?: boolean;
  /**
   * Keep the canonical screen in a leading comment, so `weft import-react`/`import-solid` give it
   * back exactly instead of reading the code by convention.
   */
  source?: boolean;
};

type Tables = { maxDepth: number; runtime: Record<string, string> };

/** The generator's constants, read once from Rust instead of copied. */
export const tables = JSON.parse(wasm.jsxTables()) as Tables;

/** Same bound as the renderer's expansion: deeper content is not rendered. */
export const MAX_DEPTH = tables.maxDepth;

export function toJsx(document: Document | unknown, options: ToJsxOptions): string {
  const name = options.componentName;
  if (name !== undefined && typeof name !== "string") {
    throw new TypeError("componentName must be a string");
  }
  const result = JSON.parse(
    catalogHandle(options.catalog).toJsx(
      toJson(document),
      JSON.stringify({
        componentName: name,
        framework: options.framework,
        typescript: options.typescript,
        source: options.source,
      }),
    ),
  ) as { code: string } | { error: string };
  if ("error" in result) throw new TypeError(result.error);
  return result.code;
}
