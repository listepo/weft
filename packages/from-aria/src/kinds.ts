// Role → kind: everything the catalog can answer comes from the catalog's own `role` fields; the
// tables below hold only what the catalog cannot express.
import type { Catalog, ComponentDef } from "@weft/core";

// Roles a renderer derives from a kind plus a prop (SPEC §5.1 notes), or that imply a prop value
// because the role is only exposed in that state.
export const ROLE_REFINEMENTS: Readonly<
  Record<string, { kind: string; props?: Readonly<Record<string, string>> }>
> = {
  spinbutton: { kind: "field", props: { type: "number" } },
  searchbox: { kind: "field", props: { type: "search" } },
  paragraph: { kind: "text" },
  // A dialog is in the accessibility tree only while it is shown.
  dialog: { kind: "dialog", props: { open: "true" } },
};

// Roles that add no element of their own: ARIA's generic and presentational roles, and the
// header and body row groups a renderer emits for a table (SPEC §5.1 notes).
export const DISSOLVED_ROLES: ReadonlySet<string> = new Set([
  "generic",
  "none",
  "presentation",
  "rowgroup",
]);

// Structural inversions, applied in build.ts where the tree is assembled:
//   tablist + following tabpanels   → tabs with one tab per tab, holding its matched panel
//   table row of only columnheaders → the table's column children
//   checked radio / selected option → the enclosing component's writable `value`
//   selected tab                    → `tabs.selected`

export type Resolved = {
  kind: string;
  def: ComponentDef;
  preset: Readonly<Record<string, string>>;
};

export type KindIndex = {
  catalog: Catalog;
  byRole: ReadonlyMap<string, string>;
  // The role-less text kind that wraps loose text runs where only elements may go.
  text: string | undefined;
};

export function kindIndex(catalog: Catalog): KindIndex {
  const byRole = new Map<string, string>();
  let text: string | undefined;
  for (const [kind, def] of Object.entries(catalog.components)) {
    if (def.role === "none") {
      if (def.content === "text") text ??= kind;
      continue;
    }
    if (!byRole.has(def.role)) byRole.set(def.role, kind);
  }
  return { catalog, byRole, text };
}

export function component(index: KindIndex, kind: string): ComponentDef | undefined {
  const all = index.catalog.components;
  return Object.hasOwn(all, kind) ? all[kind] : undefined;
}

export function resolveKind(
  index: KindIndex,
  role: string,
  named: string | undefined,
): Resolved | undefined {
  if (named !== undefined) {
    const def = component(index, named);
    return def ? { kind: named, def, preset: {} } : undefined;
  }
  if (Object.hasOwn(ROLE_REFINEMENTS, role)) {
    const r = ROLE_REFINEMENTS[role]!;
    const def = component(index, r.kind);
    if (def) return { kind: r.kind, def, preset: r.props ?? {} };
  }
  const kind = index.byRole.get(role);
  const def = kind === undefined ? undefined : component(index, kind);
  return kind !== undefined && def ? { kind, def, preset: {} } : undefined;
}
