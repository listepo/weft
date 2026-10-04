// Markup positions kept beside the model rather than inside it, so that canonical JSON stays
// free of presentation details and validation can still point at a line and column. The Rust
// core sends them as a list in node pre-order (crates/weft-wasm/src/sources.rs); these helpers
// pair that list with the JavaScript nodes, which are the keys of the map.
import type { Position } from "./diagnostics.ts";
import type { Node } from "./model.ts";

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

type Pos = [line: number, column: number];
/** `[pos, attrs, children, slots]`, as the Rust side writes it. */
export type WireSource = [
  Pos,
  [name: string, line: number, column: number][],
  (Pos | null)[],
  [name: string, pos: Pos, children: (Pos | null)[]][],
];

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null;

/** Nodes in pre-order: a node, its child elements, then the elements of each slot in order. */
function preOrder(root: unknown): Record<string, unknown>[] {
  const out: Record<string, unknown>[] = [];
  const stack: unknown[] = [root];
  for (let node = stack.pop(); node !== undefined; node = stack.pop()) {
    if (!isRecord(node)) continue;
    out.push(node);
    const lists = [
      node["children"],
      ...(isRecord(node["slots"]) ? Object.values(node["slots"]) : []),
    ];
    const next = lists.flatMap((list) =>
      Array.isArray(list) ? list.filter((c) => typeof c !== "string") : [],
    );
    stack.push(...next.reverse());
  }
  return out;
}

const position = ([line, column]: Pos): Position => ({ line, column });
const optional = (p: Pos | null) => (p === null ? undefined : position(p));
const wire = (p: Position): Pos => [p.line, p.column];

export function toSourceMap(root: Node, sources: readonly (WireSource | null)[]): SourceMap {
  const map: SourceMap = new WeakMap();
  for (const [i, node] of preOrder(root).entries()) {
    const s = sources[i];
    if (s === undefined || s === null) continue;
    const [pos, attrs, children, slots] = s;
    map.set(node as Node, {
      pos: position(pos),
      attrs: new Map(attrs.map(([name, line, column]) => [name, { line, column }])),
      children: children.map(optional),
      slots: new Map(
        slots.map(([name, at, list]) => [
          name,
          { pos: position(at), children: list.map(optional) },
        ]),
      ),
    });
  }
  return map;
}

/** The positions `map` holds for the nodes of `root`, an untrusted value, in pre-order. */
export function fromSourceMap(root: unknown, map: SourceMap): (WireSource | null)[] {
  return preOrder(root).map((node): WireSource | null => {
    const s = map.get(node as Node);
    if (s === undefined) return null;
    return [
      wire(s.pos),
      [...s.attrs].map(([name, p]): [string, number, number] => [name, p.line, p.column]),
      s.children.map((p) => (p === undefined ? null : wire(p))),
      [...s.slots].map(([name, l]): [string, Pos, (Pos | null)[]] => [
        name,
        wire(l.pos),
        l.children.map((p) => (p === undefined ? null : wire(p))),
      ]),
    ];
  });
}
