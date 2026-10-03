// The accessibility tree a document declares (SPEC §9), in the shape of a Playwright aria
// snapshot, plus a parser for that snapshot and a comparison that lists the differences.
import type { Catalog, Document } from "@weft/core";
import {
  contentText,
  dialogOpen,
  expandRoot,
  fieldRole,
  fieldInvalid,
  fieldValue,
  flag,
  headingLevel,
  label,
  nodes,
  options,
  ordered,
  ownText,
  radioChecked,
  selectedOption,
  selectedTab,
  text,
  type Inst,
  type InstChild,
} from "./expand.ts";
import { exposedRole, fallbackRole, NAME_FROM_CONTENT, TRANSPARENT } from "./roles.ts";
import { safeUrl } from "./values.ts";

// Only the states a Playwright aria snapshot reports; the rest are covered by markup tests.
export type AriaStates = {
  checked?: boolean | "mixed";
  disabled?: boolean;
  expanded?: boolean;
  invalid?: boolean;
  level?: number;
  pressed?: boolean | "mixed";
  selected?: boolean;
};

// `role` is an ARIA role, "text" for a run of text (its `name` is the text), or "fragment" for
// the root, which holds the top-level nodes like a snapshot of `<body>` does.
export type AriaNode = {
  role: string;
  name: string;
  states?: AriaStates;
  url?: string;
  children?: AriaNode[];
};

export type ExpectedTreeOptions = { catalog: Catalog; data?: unknown };

type Ctx = { group: Inst | undefined };

const textNode = (s: string): AriaNode => ({ role: "text", name: s });

function node(
  role: string,
  n: Inst,
  children: AriaNode[],
  extra: { name?: string; states?: AriaStates; url?: string } = {},
): AriaNode[] {
  if (TRANSPARENT.has(role)) return children;
  const name =
    extra.name ?? (label(n) || (NAME_FROM_CONTENT.has(role) ? contentText(ordered(n)) : ""));
  const out: AriaNode = { role, name };
  const states = Object.fromEntries(
    Object.entries(extra.states ?? {}).filter(([, v]) => v !== false && v !== undefined),
  );
  if (Object.keys(states).length > 0) out.states = states;
  if (extra.url !== undefined) out.url = extra.url;
  if (children.length > 0) out.children = children;
  return [out];
}

const many = (list: InstChild[], ctx: Ctx): AriaNode[] => list.flatMap((c) => build(c, ctx));

function build(c: InstChild, ctx: Ctx): AriaNode[] {
  if (typeof c === "string") return [textNode(c)];
  const n = c;
  const kids = () => many(ordered(n), ctx);
  const disabled = flag(n, "disabled");
  if (!n.def) return node(exposedRole(fallbackRole(n), n), n, kids());
  switch (n.kind) {
    case "screen":
      return node("main", n, kids());
    case "stack":
    case "grid":
      return kids();
    case "text": {
      const t = ownText(n);
      return t.trim() === "" ? [] : [textNode(t)];
    }
    case "section":
      return node(exposedRole("region", n), n, kids());
    case "heading": {
      const t = ownText(n);
      return node("heading", n, t ? [textNode(t)] : [], {
        name: label(n) || t,
        states: { level: headingLevel(n) },
      });
    }
    case "image":
      // An empty alt marks an image decorative, which removes it from the tree.
      return label(n) === "" ? [] : node("img", n, []);
    case "link": {
      const href = safeUrl(text(n, "href"));
      return node(
        "link",
        n,
        [textNode(contentText(n.children))],
        href === undefined ? {} : { url: href },
      );
    }
    case "button":
    case "menu-item":
      return node(
        n.kind === "button" ? "button" : "menuitem",
        n,
        [textNode(contentText(n.children))],
        {
          states: { disabled },
        },
      );
    case "form":
      return node(exposedRole("form", n), n, kids());
    case "field": {
      const value = fieldValue(n);
      const error = text(n, "error");
      return [
        ...node(fieldRole(n), n, value ? [textNode(value)] : [], {
          states: { disabled, invalid: fieldInvalid(n) },
        }),
        ...(error ? [textNode(error)] : []),
      ];
    }
    case "checkbox":
    case "switch":
      return node(n.kind, n, [], { states: { checked: flag(n, "checked"), disabled } });
    case "radio-group":
      return node("radiogroup", n, many(ordered(n), { group: n }));
    case "radio":
      return node("radio", n, [], {
        name: label(n) || contentText(n.children),
        states: { checked: radioChecked(n, ctx.group), disabled },
      });
    case "select": {
      const selected = selectedOption(n)?.node;
      const opts = options(n).flatMap((o) =>
        node("option", o, [textNode(contentText(o.children))], {
          states: { selected: o === selected },
        }),
      );
      return node("combobox", n, opts, { states: { disabled } });
    }
    case "option":
      // Outside a select an option has no list to belong to; it renders as its text.
      return [textNode(contentText(n.children))];
    case "list":
      return node("list", n, kids());
    case "item":
      return node("listitem", n, kids());
    case "table":
      return node("table", n, tableBody(n, ctx));
    // `row`, `column` and `cell` inside a table are built by `tableBody`; anywhere else they
    // fall through to a plain container with their catalog role, as the renderer emits them.
    case "tabs":
      return tabsTree(n, ctx);
    case "tab":
      // A tab outside `tabs` has no tab list; it renders as a named group of its content.
      return node("group", n, kids());
    case "dialog":
      return dialogOpen(n) ? node("dialog", n, kids()) : [];
    case "alert":
      return node("alert", n, kids());
    case "menu":
      return node("menu", n, kids());
    default:
      return node(exposedRole(fallbackRole(n), n), n, kids());
  }
}

// Mirrors the renderer: columns form the header row in one row group, everything else goes to
// the body row group, and content that is not a row is wrapped in a row and cell (SPEC §5.1).
function tableBody(table: Inst, ctx: Ctx): AriaNode[] {
  const columns: InstChild[] = [];
  const body: AriaNode[] = [];
  for (const c of table.children) {
    if (typeof c !== "string" && c.kind === "column") columns.push(c);
    else if (typeof c !== "string" && c.kind === "row") {
      body.push(
        ...node("row", c, rowCells(c.children, ctx), { states: { selected: flag(c, "selected") } }),
      );
    } else body.push(...wrapRow([c], ctx));
  }
  const groups: AriaNode[] = [];
  if (columns.length > 0) {
    const header = many(columns, ctx);
    groups.push({ role: "rowgroup", name: "", children: [rowOf(header, contentText(columns))] });
  }
  if (body.length > 0) groups.push({ role: "rowgroup", name: "", children: body });
  return groups;
}

function rowOf(children: AriaNode[], name: string): AriaNode {
  return children.length > 0 ? { role: "row", name, children } : { role: "row", name };
}

function wrapRow(list: InstChild[], ctx: Ctx): AriaNode[] {
  return [rowOf(rowCells(list, ctx), contentText(list))];
}

function rowCells(list: InstChild[], ctx: Ctx): AriaNode[] {
  return list.flatMap((c) => {
    if (typeof c !== "string" && c.kind === "cell") return build(c, ctx);
    const inner = build(c, ctx);
    const cell: AriaNode = { role: "cell", name: contentText([c]) };
    if (inner.length > 0) cell.children = inner;
    return [cell];
  });
}

function tabsTree(n: Inst, ctx: Ctx): AriaNode[] {
  const tabs = nodes(n.children).filter((c) => c.kind === "tab");
  const other = n.children.filter((c) => typeof c === "string" || c.kind !== "tab");
  const sel = selectedTab(n, tabs);
  const list = [
    ...tabs.flatMap((t, i) =>
      node("tab", t, label(t) ? [textNode(label(t))] : [], { states: { selected: i === sel } }),
    ),
    ...many(other, ctx),
  ];
  const panel = tabs[sel];
  return [
    ...node("tablist", n, list),
    ...(panel ? node("tabpanel", panel, many(ordered(panel), ctx), { name: label(panel) }) : []),
  ];
}

export function expectedTree(document: Document, options: ExpectedTreeOptions): AriaNode {
  const root = expandRoot(document?.root, options.catalog, options.data);
  const children = root ? build(root, { group: undefined }) : [];
  return normalizeAria({ role: "fragment", name: "", children });
}

// ---- Normalization and comparison ----

const squash = (s: string) => s.replace(/\s+/g, " ").trim();

// Brings a tree to the form a Playwright snapshot prints: transparent roles dissolved,
// adjacent text merged, whitespace collapsed, and text that only repeats the name dropped.
export function normalizeAria(tree: AriaNode): AriaNode {
  const out: AriaNode = { role: tree.role, name: squash(tree.name) };
  if (tree.states && Object.keys(tree.states).length > 0) out.states = { ...tree.states };
  if (tree.url !== undefined) out.url = tree.url;
  const children = normalizeChildren(tree.children ?? []);
  const onlyText = children.length === 1 && children[0]?.role === "text";
  if (children.length > 0 && !(onlyText && children[0]?.name === out.name)) out.children = children;
  return out;
}

function normalizeChildren(list: AriaNode[]): AriaNode[] {
  const flat: AriaNode[] = [];
  const visit = (n: AriaNode) => {
    if (TRANSPARENT.has(n.role) && squash(n.name) === "") {
      for (const c of n.children ?? []) visit(c);
      return;
    }
    flat.push(n);
  };
  for (const n of list) visit(n);
  const out: AriaNode[] = [];
  for (const n of flat) {
    if (n.role === "text") {
      const t = squash(n.name);
      if (t === "") continue;
      const last = out.at(-1);
      if (last?.role === "text") last.name = `${last.name} ${t}`;
      else out.push({ role: "text", name: t });
    } else {
      out.push(normalizeAria(n));
    }
  }
  return out;
}

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

function sortStates(s: AriaStates | undefined): Record<string, unknown> {
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
  if (Object.keys(states).length > 0) out.states = states as AriaStates;
  if (value !== undefined) out.children = [textNode(value)];
  return out;
}
