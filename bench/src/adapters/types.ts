import type { NNode } from "../neutral.ts";

export interface Parsed {
  /** Absent when the source could not be read at all. */
  tree?: NNode;
  /** Empty when the source is valid for its format. */
  errors: string[];
}

export const collapse = (s: string): string => s.replace(/\s+/g, " ").trim();

export const node = (kind: string, rest: Partial<NNode> = {}): NNode => ({
  kind,
  on: {},
  children: [],
  ...rest,
});

/**
 * Binding paths are compared in one notation: "$.a.b" for the data model and "$*.a" for a field
 * of the current repetition item, because formats name the loop variable differently or not at all.
 */
export function normPath(path: string, loopVars: readonly string[]): string {
  if (path.startsWith("$.")) return path;
  const m = /^\$([A-Za-z_]\w*)(\..*)?$/.exec(path);
  if (m && loopVars.includes(m[1] as string)) return "$*" + (m[2] ?? "");
  return path;
}
