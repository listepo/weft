// Role → kind tables of the importers in Rust (crates/weft-import/src/kinds.rs), exported for
// callers that map roles themselves. Everything the catalog can answer comes from the catalog's
// own `role` fields; these hold only what the catalog cannot express.
import { tables } from "./engine.ts";

// Roles a renderer derives from a kind plus a prop (SPEC §5.1 notes), or that imply a prop value
// because the role is only exposed in that state.
export const ROLE_REFINEMENTS: Readonly<
  Record<string, { kind: string; props?: Readonly<Record<string, string>> }>
> = Object.freeze(tables.roleRefinements);

// Roles that add no element of their own: ARIA's generic and presentational roles, and the
// header and body row groups a renderer emits for a table (SPEC §5.1 notes).
export const DISSOLVED_ROLES: ReadonlySet<string> = new Set(tables.dissolvedRoles);
