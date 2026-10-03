// Canonical markup (SPEC §3): the serializer emits exactly one text per canonical document.
import { canonicalize } from "./canonical.ts";
import type { Child, Document, Node } from "./model.ts";
import { SLOT } from "./rules.ts";
import { formatValue } from "./values.ts";

const INDENT = "  ";

export function serialize(document: Document): string {
  const doc = canonicalize(document);
  const lines: string[] = [];
  writeNode(doc.root, "", lines, doc.weft);
  return `${lines.join("\n")}\n`;
}

function escapeAttribute(value: string): string {
  // Tabs and newlines become references because XML would otherwise read them as spaces.
  return value.replace(/[&<"\t\n\r]/g, (c) => ATTRIBUTE_ESCAPES[c] ?? c);
}
const ATTRIBUTE_ESCAPES: Readonly<Record<string, string>> = {
  "&": "&amp;",
  "<": "&lt;",
  '"': "&quot;",
  "\t": "&#9;",
  "\n": "&#10;",
  "\r": "&#13;",
};

function escapeText(value: string): string {
  // `>` is escaped too, because `]]>` is not allowed in XML text.
  return value.replace(/[&<>]/g, (c) => (c === "&" ? "&amp;" : c === "<" ? "&lt;" : "&gt;"));
}

function writeNode(node: Node, indent: string, lines: string[], weft?: string): void {
  const attributes: [string, string][] = [];
  if (node.id !== undefined) attributes.push(["id", node.id]);
  const props = Object.entries(node.props ?? {})
    .filter(([name]) => weft === undefined || name !== "weft")
    .map(([name, value]): [string, string] => [name, formatValue(value)]);
  // The root carries the document version as an ordinary attribute, sorted with the props.
  if (weft !== undefined && weft !== "") props.push(["weft", weft]);
  props.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  attributes.push(...props);
  for (const [event, action] of Object.entries(node.on ?? {}))
    attributes.push([`on-${event}`, action]);
  const head = `<${node.kind}${attributes.map(([n, v]) => ` ${n}="${escapeAttribute(v)}"`).join("")}`;
  writeElement(
    head,
    node.kind,
    node.children ?? [],
    Object.entries(node.slots ?? {}),
    indent,
    lines,
  );
}

function writeElement(
  head: string,
  kind: string,
  children: readonly Child[],
  slots: readonly [string, Child[]][],
  indent: string,
  lines: string[],
): void {
  const only = children[0];
  if (children.length === 0 && slots.length === 0) {
    lines.push(`${indent}${head}/>`);
  } else if (children.length === 1 && typeof only === "string" && slots.length === 0) {
    lines.push(`${indent}${head}>${escapeText(only)}</${kind}>`);
  } else {
    lines.push(`${indent}${head}>`);
    const inner = indent + INDENT;
    for (const child of children) {
      if (typeof child === "string") lines.push(`${inner}${escapeText(child)}`);
      else writeNode(child, inner, lines);
    }
    for (const [name, list] of slots) {
      writeElement(`<${SLOT} name="${escapeAttribute(name)}"`, SLOT, list, [], inner, lines);
    }
    lines.push(`${indent}</${kind}>`);
  }
}
