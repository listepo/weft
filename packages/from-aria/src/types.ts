import type { Catalog, Diagnostic, Document } from "@weft/core";

// What an import could not carry over from the running UI (SPEC §9, "From a running UI").
export type LossKind =
  | "ids"
  | "bindings"
  | "actions"
  | "tokens"
  | "layout"
  | "repetition"
  | "slots"
  | "hidden"
  | "props"
  | "values"
  | "names"
  | "kinds"
  | "text"
  | "structure";

export type Loss = { kind: LossKind; path: string; note: string };

export type ImportOptions = { catalog: Catalog };

export type ImportResult = { document: Document; losses: Loss[]; diagnostics: Diagnostic[] };

export type Scalar = string | number | boolean;

// The neutral tree both importers produce before the catalog is consulted: one entry per
// accessibility node (or text run, role "text" with the text in `name`).
export type Sem = {
  role: string;
  name: string;
  // ARIA states as a snapshot reports them: checked, disabled, selected, level, invalid, busy, …
  states: Record<string, Scalar>;
  // Values only an HTML source carries, keyed by the catalog prop they feed (type, value, href, …).
  props: Record<string, string>;
  // A kind the source names directly (Weft renderer markup for role-less kinds such as `text`).
  kind?: string | undefined;
  // A Weft id the source carries (`data-weft-id`), already checked to be valid and unique.
  id?: string | undefined;
  // The HTML id and `aria-labelledby` references, used to pair tab panels with their tabs.
  ref?: string | undefined;
  labelledBy?: readonly string[] | undefined;
  // Losses the source already knows about for this node; reported with the node's path.
  notes?: { kind: LossKind; note: string }[] | undefined;
  children: Sem[];
};
