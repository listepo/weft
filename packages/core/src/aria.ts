// The accessibility tree a document declares (SPEC §9), as the reference renderer exposes it, in
// the shape of a Playwright aria snapshot. Built by the Rust core (crates/weft-core/src/aria), so the
// MCP tools, the cox plugin and the renderer's own tests read one tree.
import type { Catalog, Document } from "./model.ts";
import { catalogHandle, toJson, wasm } from "./wasm.ts";

/** Only the states a Playwright aria snapshot reports. */
export type AriaStates = {
  checked?: boolean | "mixed";
  disabled?: boolean;
  expanded?: boolean;
  invalid?: boolean;
  level?: number;
  pressed?: boolean | "mixed";
  selected?: boolean;
};

/**
 * `role` is an ARIA role, "text" for a run of text (its `name` is the text), or "fragment" for the
 * root, which holds the top-level nodes like a snapshot of `<body>` does.
 */
export type AriaNode = {
  role: string;
  name: string;
  states?: AriaStates;
  url?: string;
  children?: AriaNode[];
};

/** The normalized tree the reference renderer's page exposes for `document` with `data`. */
export function ariaTree(document: Document, catalog: Catalog, data?: unknown): AriaNode {
  return JSON.parse(catalogHandle(catalog).ariaTree(toJson(document), toJson(data))) as AriaNode;
}

/**
 * A tree in the form a Playwright snapshot prints: transparent roles dissolved, adjacent text
 * merged, whitespace collapsed, and text that only repeats the name dropped.
 */
export function normalizeAria(tree: AriaNode): AriaNode {
  return JSON.parse(wasm.normalizeAria(toJson(tree) ?? "null")) as AriaNode;
}

/** The YAML `locator.ariaSnapshot()` prints for a tree; empty when nothing is exposed. */
export function formatAriaSnapshot(tree: AriaNode): string {
  return JSON.parse(wasm.formatAria(toJson(tree) ?? "null")) as string;
}
