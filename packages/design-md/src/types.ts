export type LossKind =
  | "component"
  | "converted"
  | "invalid-name"
  | "prose"
  | "theme"
  | "unresolved-alias"
  | "unsupported-section"
  | "unsupported-value";

/** What the mapping could not carry over, or carried in another form. */
export type Loss = { kind: LossKind; path: string; note: string };

/** A DTCG 2025.10 token tree (https://www.designtokens.org/tr/2025.10/format/). */
export type TokenTree = { [name: string]: unknown };

export type Mapped = {
  tokens: TokenTree;
  losses: Loss[];
  /** Reasons nothing, or not everything, could be read. Mapping never throws on bad input. */
  problems: string[];
  /** The design system's own name, when its source states one. */
  name?: string;
};

/** The longest source the mappers read, in UTF-16 code units. */
export const MAX_SOURCE_LENGTH = 1_000_000;
