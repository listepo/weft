// Canonical JSON (SPEC §3): one byte sequence per document, so that tools can diff and hash.
import type { Child, Document, Node, Value } from "./model.ts";

export type Entries<T> = readonly (readonly [string, T])[];

/** Whitespace handling of SPEC §2 for text content; only XML whitespace counts. */
export function normalizeText(text: string): string {
  return text.replace(/[ \t\n\r]+/g, " ").replace(/^ | $/g, "");
}

/** Appends text to a child list; adjacent text runs join with one space, as markup would read them. */
export function appendText(children: Child[], text: string): boolean {
  if (text === "") return false;
  const last = children.length - 1;
  const previous = children[last];
  if (typeof previous === "string") {
    children[last] = `${previous} ${text}`;
    return false;
  }
  children.push(text);
  return true;
}

function compareKeys(a: readonly [string, unknown], b: readonly [string, unknown]): number {
  return a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0;
}

/**
 * `Object.fromEntries` defines own properties, so a hostile key such as `__proto__` stays data
 * instead of replacing the prototype.
 */
function sortedRecord<T>(entries: Entries<T>): Record<string, T> | undefined {
  return entries.length === 0 ? undefined : Object.fromEntries(entries.toSorted(compareKeys));
}

/** Builds a node with canonical key order and without empty members. */
export function assembleNode(parts: {
  kind: string;
  id?: string | undefined;
  props?: Entries<Value>;
  on?: Entries<string>;
  slots?: Entries<Child[]>;
  children?: Child[];
}): Node {
  const node: Node = { kind: parts.kind };
  if (parts.id !== undefined) node.id = parts.id;
  const props = sortedRecord(parts.props ?? []);
  if (props !== undefined) node.props = props;
  const on = sortedRecord(parts.on ?? []);
  if (on !== undefined) node.on = on;
  const slots = sortedRecord((parts.slots ?? []).filter(([, list]) => list.length > 0));
  if (slots !== undefined) node.slots = slots;
  if (parts.children !== undefined && parts.children.length > 0) node.children = parts.children;
  return node;
}

export function canonicalValue(value: Value): Value {
  if (typeof value === "number") return value === 0 ? 0 : value;
  if (typeof value !== "object") return value;
  if ("bind" in value)
    return value.not === true ? { bind: value.bind, not: true } : { bind: value.bind };
  return { token: value.token };
}

function canonicalChildren(children: readonly Child[]): Child[] {
  const out: Child[] = [];
  for (const child of children) {
    if (typeof child === "string") appendText(out, normalizeText(child));
    else out.push(canonicalNode(child));
  }
  return out;
}

function canonicalNode(node: Node): Node {
  return assembleNode({
    kind: node.kind,
    id: node.id,
    props: Object.entries(node.props ?? {}).map(([k, v]) => [k, canonicalValue(v)] as const),
    on: Object.entries(node.on ?? {}),
    slots: Object.entries(node.slots ?? {}).map(([k, v]) => [k, canonicalChildren(v)] as const),
    children: canonicalChildren(node.children ?? []),
  });
}

export function canonicalize(document: Document): Document {
  return { weft: document.weft, root: canonicalNode(document.root) };
}

/** Canonical JSON text: two-space indentation and a final newline. */
export function stringify(document: Document): string {
  return `${JSON.stringify(canonicalize(document), null, 2)}\n`;
}
