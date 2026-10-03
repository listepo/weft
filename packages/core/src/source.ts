// Markup positions kept beside the model rather than inside it, so that canonical JSON stays
// free of presentation details and validation can still point at a line and column.
import type { Position } from "./diagnostics.ts";
import type { Node } from "./model.ts";
import { ID } from "./rules.ts";

export type ListSource = { pos: Position; children: (Position | undefined)[] };

export type NodeSource = {
  pos: Position;
  /** Keyed by attribute name as written: `id`, `weft`, `on-press`, … */
  attrs: Map<string, Position>;
  /** Index-aligned with `node.children`. */
  children: (Position | undefined)[];
  slots: Map<string, ListSource>;
};

export type SourceMap = WeakMap<Node, NodeSource>;

/**
 * One step of a diagnostic path (SPEC §6.1): `kind#id`, or `kind[index]` when the element has
 * no usable id and its position in the parent's list is known.
 */
export function pathSegment(kind: string, id: string | undefined, index?: number): string {
  if (id !== undefined && ID.test(id)) return `${kind}#${id}`;
  return index === undefined ? kind : `${kind}[${index}]`;
}
