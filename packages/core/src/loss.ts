// What an importer could not carry over into Weft (SPEC §9), one entry per place: the shape every
// importer of crates/weft-import returns, from a running UI, code or a Custom Elements Manifest.
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
