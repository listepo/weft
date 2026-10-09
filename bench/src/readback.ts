import type { NNode } from "./neutral.ts";

/**
 * Plain-language diff of the bindings, flags, events and loops that differ between two screens.
 * The sentences match `weft explain` for a negation (`true while $.busy is falsy (NOT $.busy)`),
 * and they are computed from the neutral tree so every format gets the same wording. Elements are
 * paired by kind and visible name, in order; a change of name reads as a removal plus an addition.
 */

const FACT_ORDER = ["disabled", "hidden", "bind", "text", "in"];

function flagSentence(value: string | boolean): string {
  if (value === true) return "always true";
  if (typeof value === "string" && value.startsWith("!")) {
    const path = value.slice(1);
    return `true while ${path} is falsy (NOT ${path})`;
  }
  return `true while ${value} is truthy`;
}

function bindSentence(value: string): string {
  if (value.startsWith("!")) {
    const path = value.slice(1);
    return `true while ${path} is falsy (NOT ${path})`;
  }
  return `reads ${value}`;
}

function facts(node: NNode): Map<string, string> {
  const out = new Map<string, string>();
  for (const name of ["disabled", "hidden"] as const) {
    const value = node[name];
    if (value === undefined || value === false) continue;
    out.set(name, flagSentence(value));
  }
  if (node.bind) out.set("bind", bindSentence(node.bind));
  if (node.nameBind) out.set("text", `reads ${node.nameBind}`);
  if (node.each) out.set("in", `repeats its children once per item of ${node.each}`);
  for (const event of Object.keys(node.on).sort())
    out.set(`on-${event}`, `runs action ${node.on[event]}`);
  return out;
}

function label(node: NNode, path: string): string {
  if (node.name) return `${node.kind} "${node.name}"`;
  if (node.nameBind) return `${node.kind} ${node.nameBind}`;
  return `${node.kind} at ${path}`;
}

function sameElement(a: NNode, b: NNode): boolean {
  return (
    a.kind === b.kind &&
    (a.name ?? "") === (b.name ?? "") &&
    (a.nameBind ?? "") === (b.nameBind ?? "")
  );
}

function factNames(before: Map<string, string>, after: Map<string, string>): string[] {
  const present = new Set([...before.keys(), ...after.keys()]);
  const rest = [...present].filter((name) => !FACT_ORDER.includes(name)).sort();
  return [...FACT_ORDER.filter((name) => present.has(name)), ...rest];
}

function emitFacts(
  before: NNode | undefined,
  after: NNode | undefined,
  path: string,
  lines: string[],
): void {
  const node = after ?? before;
  if (!node) return;
  const target = label(node, path);
  const oldFacts = before ? facts(before) : new Map<string, string>();
  const newFacts = after ? facts(after) : new Map<string, string>();
  for (const name of factNames(oldFacts, newFacts)) {
    const was = oldFacts.get(name);
    const now = newFacts.get(name);
    if (was === now) continue;
    if (was === undefined) lines.push(`${target} ${name} added: ${now}`);
    else if (now === undefined) lines.push(`${target} ${name} removed: was ${was}`);
    else lines.push(`${target} ${name} changed: was ${was}; now ${now}`);
  }
}

function walk(
  before: NNode | undefined,
  after: NNode | undefined,
  path: string,
  lines: string[],
): void {
  emitFacts(before, after, path, lines);
  const left = before?.children ?? [];
  const right = after?.children ?? [];
  const used = new Set<number>();
  right.forEach((child, index) => {
    const match = left.findIndex((other, i) => !used.has(i) && sameElement(child, other));
    const childPath = `${path}/${child.kind}[${index}]`;
    if (match >= 0) {
      used.add(match);
      walk(left[match], child, childPath, lines);
    } else {
      walk(undefined, child, childPath, lines);
    }
  });
  left.forEach((child, index) => {
    if (used.has(index)) return;
    walk(child, undefined, `${path}/${child.kind}[${index}]`, lines);
  });
}

/** One line per binding, flag, event or loop that was added, removed or changed. */
export function bindingChanges(before: NNode, after: NNode): string[] {
  const lines: string[] = [];
  walk(before, after, `/${before.kind}`, lines);
  return lines;
}
