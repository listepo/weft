// The importers in Rust (crates/weft-web, crates/weft-import) through the web WebAssembly module.
// Each call crosses as JSON text; this file is the only one in the package that loads the module.
import { catalogHandle, toJson, wasm, wellFormed } from "@weft/core/web";
import type { Catalog, Diagnostic } from "@weft/core";
import type { ImportResult, Sem } from "./types.ts";

export type BuildOptions = {
  catalog: Catalog;
  // Ids the source carries; generated ids avoid them.
  reserved?: Iterable<string>;
  // Diagnostics found before the build; validation appends to this array, which is returned.
  diagnostics?: Diagnostic[];
};

export type Built = ImportResult & { rootPath: string };

/** Role tree → document: the shared builder of crates/weft-import. */
export function buildDocument(top: readonly Sem[], options: BuildOptions): Built {
  const handle = catalogHandle(options.catalog);
  const built = JSON.parse(
    handle.buildDocument(toJson(top) ?? "[]", toJson([...(options.reserved ?? [])]) ?? "[]"),
  ) as Built;
  const diagnostics = options.diagnostics ?? [];
  diagnostics.push(...built.diagnostics);
  return { ...built, diagnostics };
}

/** The result of an import that could not start (W601). */
export function importFailure(message: string, expected: string, got?: string): ImportResult {
  return JSON.parse(wasm.importFailure(message, expected, got)) as ImportResult;
}

export function htmlImport(html: string, catalog: Catalog): ImportResult {
  return JSON.parse(catalogHandle(catalog).fromDom(wellFormed(html))) as ImportResult;
}

export function instanceIdOf(raw: string): string | undefined {
  return wasm.instanceId(wellFormed(raw));
}

type Tables = {
  implicitRoles: Record<string, string>;
  inputRoles: Record<string, string>;
  maxHtmlLength: number;
  dissolvedRoles: string[];
  roleRefinements: Record<string, { kind: string; props?: Record<string, string> }>;
};

/** The importers' constant tables, read once from Rust instead of copied. */
export const tables = JSON.parse(wasm.webTables()) as Tables;
