// ARIA role facts the renderer and `expectedTree` both depend on.
import { label, type Inst } from "./expand.ts";

// Concrete (non-abstract) roles of WAI-ARIA 1.2. An extension's `role` outside this list would
// be ignored by browsers, so it falls back to `group` like an unknown element (SPEC §8).
const ARIA_ROLES = new Set(
  (
    "alert alertdialog application article banner blockquote button caption cell checkbox code " +
    "columnheader combobox complementary contentinfo definition deletion dialog document emphasis " +
    "feed figure form generic grid gridcell group heading img insertion link list listbox listitem " +
    "log main marquee math menu menubar menuitem menuitemcheckbox menuitemradio meter navigation " +
    "none note option paragraph presentation progressbar radio radiogroup region row rowgroup " +
    "rowheader scrollbar search searchbox separator slider spinbutton status strong subscript " +
    "superscript switch tab table tablist tabpanel term textbox time timer toolbar tooltip tree " +
    "treegrid treeitem"
  ).split(" "),
);

// Roles whose accessible name is computed from their content when no label is given (ARIA 1.2).
export const NAME_FROM_CONTENT = new Set([
  "button",
  "cell",
  "checkbox",
  "columnheader",
  "gridcell",
  "heading",
  "link",
  "menuitem",
  "menuitemcheckbox",
  "menuitemradio",
  "option",
  "radio",
  "row",
  "rowheader",
  "switch",
  "tab",
  "tooltip",
  "treeitem",
]);

// Roles that add nothing to the accessibility tree; their children take their place.
export const TRANSPARENT = new Set(["none", "presentation", "generic"]);

// The role of a container rendered for a kind the renderer has no mapping for: an extension's
// declared `role`, the catalog role of a kind known to the catalog but not to this renderer,
// or `group` (SPEC §8).
export function fallbackRole(n: Inst): string {
  const declared = n.def ? n.def.role : n.props["role"];
  if (typeof declared === "string" && ARIA_ROLES.has(declared)) return declared;
  return "group";
}

// `region` and `form` are landmarks only when named; unnamed, HTML-AAM maps them to `generic`.
export function exposedRole(role: string, n: Inst): string {
  return (role === "region" || role === "form") && label(n) === "" ? "generic" : role;
}
