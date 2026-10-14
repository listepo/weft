// The accessibility tree a document declares (SPEC §9), in the shape of a Playwright aria
// snapshot, plus a parser for that snapshot and a comparison that lists the differences. The tree,
// its normal form and its YAML come from the Rust core (`ariaTree` of `@weft/core`), which the MCP
// tools and the cox plugin share; the renderer's tests check that tree against the DOM.
import { ariaTree, normalizeAria, type AriaNode, type Catalog, type Document } from "@weft/core";

export { formatAriaSnapshot, normalizeAria } from "@weft/core";
export type { AriaNode, AriaStates } from "@weft/core";

export type ExpectedTreeOptions = { catalog: Catalog; data?: unknown };

export function expectedTree(document: Document, options: ExpectedTreeOptions): AriaNode {
  return ariaTree(document, options.catalog, options.data);
}

const textNode = (s: string): AriaNode => ({ role: "text", name: s });

// ---- Comparison ----

const describe = (n: AriaNode) =>
  n.role === "text" ? `text "${n.name}"` : n.name ? `${n.role} "${n.name}"` : n.role;

// Lists every difference between two trees, each prefixed with the path to where it occurs.
// An empty list means the trees are equal after normalization.
export function diffAria(expected: AriaNode, actual: AriaNode): string[] {
  const diffs: string[] = [];
  compare(normalizeAria(expected), normalizeAria(actual), "root", diffs);
  return diffs;
}

function compare(e: AriaNode, a: AriaNode, here: string, diffs: string[]) {
  if (e.role !== a.role) diffs.push(`${here}: expected role ${e.role}, got ${a.role}`);
  if (e.name !== a.name) diffs.push(`${here}: expected name "${e.name}", got "${a.name}"`);
  if (e.url !== a.url)
    diffs.push(`${here}: expected url ${e.url ?? "none"}, got ${a.url ?? "none"}`);
  const es = JSON.stringify(sortStates(e.states));
  const as = JSON.stringify(sortStates(a.states));
  if (es !== as) diffs.push(`${here}: expected states ${es}, got ${as}`);
  const ec = e.children ?? [];
  const ac = a.children ?? [];
  if (ec.length !== ac.length) {
    diffs.push(
      `${here}: expected ${ec.length} children [${ec.map(describe).join(", ")}], ` +
        `got ${ac.length} [${ac.map(describe).join(", ")}]`,
    );
  }
  for (let i = 0; i < Math.min(ec.length, ac.length); i++) {
    compare(ec[i]!, ac[i]!, `${here} > [${i}] ${describe(ec[i]!)}`, diffs);
  }
}

// Playwright marks the focused element `active`; focus is not something a document declares.
const IGNORED_STATES = new Set(["active", "ref", "cursor"]);

function sortStates(s: AriaNode["states"]): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(s ?? {}).sort()) {
    if (!IGNORED_STATES.has(k)) out[k] = (s as Record<string, unknown>)[k];
  }
  return out;
}

// ---- Playwright aria snapshot (YAML) parser ----

// Reads the YAML that `locator.ariaSnapshot()` prints into a fragment-rooted `AriaNode`.
// Only the subset Playwright generates is understood; unparseable lines throw, since the input
// comes from Playwright and a silent misread would hide a real difference.
export function parseAriaSnapshot(yaml: string): AriaNode {
  const root: AriaNode = { role: "fragment", name: "" };
  const stack: { indent: number; node: AriaNode }[] = [{ indent: -1, node: root }];
  for (const raw of yaml.split("\n")) {
    if (raw.trim() === "") continue;
    const indent = raw.length - raw.trimStart().length;
    const line = raw.trimStart();
    if (!line.startsWith("- ")) throw new Error(`unexpected aria snapshot line: ${raw}`);
    while (stack.length > 1 && stack.at(-1)!.indent >= indent) stack.pop();
    const parent = stack.at(-1)!.node;
    const { key, value } = splitEntry(line.slice(2));
    if (key.startsWith("/")) {
      if (key === "/url" && value !== undefined) parent.url = value;
      continue;
    }
    const child: AriaNode = key === "text" ? textNode(value ?? "") : parseKey(key, value);
    (parent.children ??= []).push(child);
    if (key !== "text") stack.push({ indent, node: child });
  }
  return root;
}

function splitEntry(body: string): { key: string; value: string | undefined } {
  let key: string;
  let rest: string;
  if (body.startsWith("'")) {
    const [quoted, after] = readSingleQuoted(body);
    key = quoted;
    rest = after;
  } else {
    const end = keyEnd(body);
    key = body.slice(0, end);
    rest = body.slice(end);
  }
  if (rest === "" || rest === ":") return { key, value: undefined };
  if (!rest.startsWith(": ")) throw new Error(`unexpected aria snapshot entry: ${body}`);
  return { key, value: scalar(rest.slice(2)) };
}

// The key ends at the first ":" outside a double-quoted name and outside [attribute] brackets.
function keyEnd(body: string): number {
  let inString = false;
  let depth = 0;
  for (let i = 0; i < body.length; i++) {
    const ch = body[i];
    if (inString) {
      if (ch === "\\") i++;
      else if (ch === '"') inString = false;
    } else if (ch === '"') inString = true;
    else if (ch === "[") depth++;
    else if (ch === "]") depth--;
    else if (ch === ":" && depth === 0) return i;
  }
  return body.length;
}

function readSingleQuoted(s: string): [string, string] {
  let out = "";
  for (let i = 1; i < s.length; i++) {
    if (s[i] === "'") {
      if (s[i + 1] === "'") {
        out += "'";
        i++;
      } else return [out, s.slice(i + 1)];
    } else out += s[i];
  }
  throw new Error(`unterminated quoted key: ${s}`);
}

function scalar(s: string): string {
  if (s.startsWith('"')) return JSON.parse(s) as string;
  if (s.startsWith("'")) return readSingleQuoted(s)[0];
  return s;
}

const KEY = /^([a-z][a-z-]*)(?: ("(?:[^"\\]|\\.)*"))?((?: \[[^\]]*\])*)$/;

function parseKey(key: string, value: string | undefined): AriaNode {
  const m = KEY.exec(key);
  if (!m) throw new Error(`unexpected aria snapshot key: ${key}`);
  const [, role = "", name, attrs = ""] = m;
  const out: AriaNode = { role, name: name ? (JSON.parse(name) as string) : "" };
  const states: Record<string, unknown> = {};
  for (const a of attrs.matchAll(/\[([a-z]+)(?:=([^\]]*))?\]/g)) {
    const [, k = "", v] = a;
    states[k] = v === undefined ? true : /^\d+$/.test(v) ? Number(v) : v;
  }
  if (Object.keys(states).length > 0) out.states = states as NonNullable<AriaNode["states"]>;
  if (value !== undefined) out.children = [textNode(value)];
  return out;
}
