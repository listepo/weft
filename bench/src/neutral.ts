// Format-neutral view of a screen. Every format is parsed into this tree so that one
// expectation vocabulary can judge Weft, HTML, JSX and A2UI output alike.

export type Format = "weft" | "html" | "jsx" | "a2ui";
export const FORMATS: readonly Format[] = ["weft", "html", "jsx", "a2ui"];

export interface NNode {
  kind: string;
  /** Literal accessible name or visible text, whitespace-collapsed. */
  name?: string;
  /** Path that supplies the name or text, e.g. "$.user.name"; "$*." marks the loop item. */
  nameBind?: string;
  /** The node's data binding: field value, checkbox state, image source, link target. */
  bind?: string;
  /** `true`, or the path that disables the node; a leading "!" negates it. */
  disabled?: string | boolean;
  hidden?: string | boolean;
  variant?: string;
  type?: string;
  required?: boolean;
  level?: number;
  sort?: string;
  value?: string;
  state?: string;
  /** For kind "each": the array path being repeated. */
  each?: string;
  on: Record<string, string>;
  children: NNode[];
}

export interface NodeSpec {
  kind?: string;
  name?: string;
  nameBind?: string;
  bind?: string;
  disabled?: string | boolean;
  hidden?: string | boolean;
  variant?: string;
  type?: string;
  required?: boolean;
  level?: number;
  value?: string;
  /** Subset of events that must map to these action names. */
  on?: Record<string, string>;
  /** The node sits inside a repetition of this array path. */
  each?: string;
}

/** A string is shorthand for `{ name }`. */
export type Ref = string | NodeSpec;

export type Assertion =
  | {
      has: NodeSpec;
      after?: Ref;
      before?: Ref;
      inside?: Ref;
      not_inside?: Ref;
      /** Exact number of matches; default is at least one. */
      count?: number;
    }
  | { absent: NodeSpec };

/** Kinds that A2UI's basic catalog cannot express natively, mapped to what it uses instead. */
const A2UI_KIND_ALIASES: Record<string, string[]> = {
  switch: ["checkbox"],
  "radio-group": ["choice"],
  select: ["choice"],
  radio: ["option"],
  "menu-item": ["link", "button"],
  menu: ["list"],
  alert: ["card"],
  section: ["card"],
  item: ["stack"],
};

export interface Entry {
  node: NNode;
  ancestors: NNode[];
  index: number;
}

export function walk(root: NNode): Entry[] {
  const out: Entry[] = [];
  const visit = (node: NNode, ancestors: NNode[]) => {
    out.push({ node, ancestors, index: out.length });
    for (const child of node.children) visit(child, [...ancestors, node]);
  };
  visit(root, []);
  return out;
}

export const norm = (s: string): string => s.replace(/\s+/g, " ").trim().toLowerCase();

const kindMatches = (specKind: string, nodeKind: string, format: Format): boolean =>
  specKind === nodeKind ||
  (format === "a2ui" && (A2UI_KIND_ALIASES[specKind] ?? []).includes(nodeKind));

export function matches(entry: Entry, spec: NodeSpec, format: Format): boolean {
  const n = entry.node;
  if (spec.kind !== undefined && !kindMatches(spec.kind, n.kind, format)) return false;
  if (spec.name !== undefined && norm(n.name ?? "\u0000") !== norm(spec.name)) return false;
  if (spec.nameBind !== undefined && n.nameBind !== spec.nameBind) return false;
  if (spec.bind !== undefined && n.bind !== spec.bind) return false;
  if (spec.disabled !== undefined && (n.disabled ?? false) !== spec.disabled) return false;
  if (spec.hidden !== undefined && (n.hidden ?? false) !== spec.hidden) return false;
  if (spec.variant !== undefined && n.variant !== spec.variant) return false;
  if (spec.type !== undefined && (n.type ?? "text") !== spec.type) return false;
  if (spec.required !== undefined && (n.required ?? false) !== spec.required) return false;
  if (spec.level !== undefined && n.level !== spec.level) return false;
  if (spec.value !== undefined && n.value !== spec.value) return false;
  for (const [event, action] of Object.entries(spec.on ?? {}))
    if (n.on[event] !== action) return false;
  if (
    spec.each !== undefined &&
    !entry.ancestors.some((a) => a.kind === "each" && a.each === spec.each)
  )
    return false;
  return true;
}

const asSpec = (r: Ref): NodeSpec => (typeof r === "string" ? { name: r } : r);

export interface AssertionResult {
  assertion: Assertion;
  ok: boolean;
}

export function evaluate(tree: NNode, assertions: Assertion[], format: Format): AssertionResult[] {
  const entries = walk(tree);
  const find = (spec: NodeSpec) => entries.filter((e) => matches(e, spec, format));
  return assertions.map((assertion) => {
    if ("absent" in assertion) return { assertion, ok: find(assertion.absent).length === 0 };
    const candidates = find(assertion.has).filter((e) => {
      const after = assertion.after === undefined ? null : find(asSpec(assertion.after));
      const before = assertion.before === undefined ? null : find(asSpec(assertion.before));
      const inside = assertion.inside === undefined ? null : asSpec(assertion.inside);
      const notInside = assertion.not_inside === undefined ? null : asSpec(assertion.not_inside);
      if (after && !after.some((r) => r.index < e.index)) return false;
      if (before && !before.some((r) => r.index > e.index)) return false;
      const holds = (ref: NodeSpec) =>
        e.ancestors.some((anc) => matches({ node: anc, ancestors: [], index: -1 }, ref, format));
      if (inside && !holds(inside)) return false;
      if (notInside && holds(notInside)) return false;
      return true;
    });
    const ok =
      assertion.count === undefined ? candidates.length > 0 : candidates.length === assertion.count;
    return { assertion, ok };
  });
}

/** Names that carry content; used to detect that an edit deleted something it should not have. */
export function contentNames(tree: NNode, removed: Ref[], format: Format): string[] {
  const entries = walk(tree);
  const gone = removed.map(asSpec);
  const names = new Set<string>();
  for (const e of entries) {
    if (gone.some((spec) => matches(e, spec, format))) continue;
    if (e.node.kind === "screen") continue;
    const label = e.node.name ?? e.node.nameBind;
    if (label) names.add(norm(label));
  }
  return [...names];
}

export function missingContent(
  original: NNode,
  output: NNode,
  removed: Ref[],
  format: Format,
): string[] {
  const have = new Set(
    walk(output).flatMap((e) =>
      [e.node.name, e.node.nameBind].filter((x) => x).map((x) => norm(x as string)),
    ),
  );
  return contentNames(original, removed, format).filter((n) => !have.has(n));
}
