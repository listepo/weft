// Grammars and fixed vocabulary from SPEC §2–§4 and §8, shared by the parser, serializer and
// validator so that every layer agrees on what a valid name, id or reference is.
import type { PropDef } from "./model.ts";

/** Element and attribute names (SPEC §2). */
export const NAME = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
/** Extension names: `x-<vendor>-<name>` (SPEC §8). */
export const EXTENSION_NAME = /^x-[a-z0-9]+-[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const ID = /^[A-Za-z][A-Za-z0-9_-]*$/;
export const ACTION = /^[a-z][A-Za-z0-9]*(?:\.[a-z][A-Za-z0-9]*)*$/;
export const LOOP_VARIABLE = /^[a-z][A-Za-z0-9]*$/;
const SEGMENT = "(?:[A-Za-z_][A-Za-z0-9_]*|0|[1-9][0-9]*)";
export const BINDING = new RegExp(
  `^\\$(?:(?:\\.${SEGMENT})+|[A-Za-z_][A-Za-z0-9_]*(?:\\.${SEGMENT})*)$`,
);
export const TOKEN = /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*$/;
export const VERSION = /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/;
export const JSON_NUMBER = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?$/;
/** A reference written after other text in one value, e.g. `Hello {$.name}` (SPEC §2.1). */
export const EMBEDDED_REFERENCE = /\{!?\$|\{token\./;
/** Characters XML 1.0 cannot carry at all, not even as character references. */
export const NON_XML_CHAR = /[^\t\n\r\x20-\uD7FF\uE000-\uFFFD\u{10000}-\u{10FFFF}]/u;

/** Deeper trees are rejected so that recursive walkers cannot exhaust the stack on hostile input. */
export const MAX_DEPTH = 256;

export const SLOT = "slot";
export const EACH = "each";

/** Universal attributes of SPEC §2.2 other than `id` and `on-*`; `state` values come from the component. */
export const UNIVERSAL_PROPS: Readonly<Record<string, PropDef>> = {
  label: { description: "Accessible name.", type: "string" },
  hidden: { description: "Not rendered and not exposed.", type: "boolean" },
  state: { description: "One of the component's states.", type: "enum" },
  role: { description: "ARIA role of an extension element.", type: "string", bindable: false },
};

/** Non-abstract roles of WAI-ARIA 1.2, https://www.w3.org/TR/wai-aria-1.2/#role_definitions */
export const ARIA_ROLES: readonly string[] = [
  "alert",
  "alertdialog",
  "application",
  "article",
  "banner",
  "blockquote",
  "button",
  "caption",
  "cell",
  "checkbox",
  "code",
  "columnheader",
  "combobox",
  "complementary",
  "contentinfo",
  "definition",
  "deletion",
  "dialog",
  "document",
  "emphasis",
  "feed",
  "figure",
  "form",
  "generic",
  "grid",
  "gridcell",
  "group",
  "heading",
  "img",
  "insertion",
  "link",
  "list",
  "listbox",
  "listitem",
  "log",
  "main",
  "marquee",
  "math",
  "menu",
  "menubar",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "meter",
  "navigation",
  "none",
  "note",
  "option",
  "paragraph",
  "presentation",
  "progressbar",
  "radio",
  "radiogroup",
  "region",
  "row",
  "rowgroup",
  "rowheader",
  "scrollbar",
  "search",
  "searchbox",
  "separator",
  "slider",
  "spinbutton",
  "status",
  "strong",
  "subscript",
  "superscript",
  "switch",
  "tab",
  "table",
  "tablist",
  "tabpanel",
  "term",
  "textbox",
  "time",
  "timer",
  "toolbar",
  "tooltip",
  "tree",
  "treegrid",
  "treeitem",
];

/**
 * Own-property lookup: documents and catalogs are untrusted, and a kind such as `constructor`
 * must not resolve to something inherited from `Object.prototype`.
 */
export function own<T>(
  record: Readonly<Record<string, T>> | undefined,
  key: string,
): T | undefined {
  return record !== undefined && Object.hasOwn(record, key) ? record[key] : undefined;
}
