// Assembles a Weft document from the neutral tree (types.ts) so that it validates against the
// catalog in lenient mode: whatever would break a catalog rule is placed in a slot that admits it
// or kept as an `x-aria-<role>` extension element (SPEC §8), and every such step is a loss.
import {
  ARIA_ROLES,
  canonicalize,
  diagnostic,
  EMBEDDED_REFERENCE,
  ID,
  NON_XML_CHAR,
  validate,
  WEFT_VERSION,
  type Catalog,
  type Child,
  type ComponentDef,
  type Diagnostic,
  type Node,
  type PropDef,
  type Value,
} from "@weft/core";
import { component, DISSOLVED_ROLES, kindIndex, resolveKind, type KindIndex } from "./kinds.ts";
import type { ImportResult, Loss, LossKind, Scalar, Sem } from "./types.ts";

// Below the 256 levels a document may nest (SPEC §2), leaving room for wrappers added here.
export const MAX_DEPTH = 200;
export const MAX_NODES = 20_000;

const ROLES: ReadonlySet<string> = new Set(ARIA_ROLES);
const NON_XML = new RegExp(NON_XML_CHAR.source, "gu");
const REFERENCE_START = /\{(?=!?\$|token\.)/g;
const BUSY_STATES = ["busy", "loading", "submitting"];

export const squash = (s: string): string => s.replace(/\s+/g, " ").trim();
export const clean = (s: string): string => s.replace(NON_XML, "");

// A readable id fragment: lower-case ASCII words joined by hyphens.
export function slug(text: string): string {
  return text
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+/, "")
    .slice(0, 32)
    .replace(/-+$/, "");
}

// The text a subtree shows, joined with spaces as an accessible name computed from content is.
export function textOf(list: readonly Sem[], depth = 0): string {
  if (depth > MAX_DEPTH) return "";
  const parts: string[] = [];
  for (const s of list) {
    const inner = s.role === "text" ? s.name : textOf(s.children, depth + 1) || s.name;
    if (inner.trim() !== "") parts.push(inner);
  }
  return squash(parts.join(" "));
}

type Parent = {
  kind: string;
  def: ComponentDef | undefined;
  path: string;
  inForm: boolean;
  depth: number;
};

type Ctx = {
  index: KindIndex;
  formKind: string | undefined;
  used: Set<string>;
  claimed: Set<string>;
  counters: Map<string, number>;
  losses: Loss[];
  diagnostics: Diagnostic[];
  nodes: number;
  truncated: boolean;
};

type Placed = { node: Node; slot?: string | undefined };

const lose = (ctx: Ctx, kind: LossKind, path: string, note: string) =>
  ctx.losses.push({ kind, path, note });

export function limitReached(diagnostics: Diagnostic[], path: string, what: string): void {
  diagnostics.push(
    diagnostic("W602", {
      path,
      message: `The input ${what}; the rest was not imported.`,
      expected: `at most ${MAX_NODES} elements, nested at most ${MAX_DEPTH} levels deep`,
    }),
  );
}

function truncate(ctx: Ctx, path: string): void {
  if (ctx.truncated) return;
  ctx.truncated = true;
  limitReached(ctx.diagnostics, path, "is larger or deeper than the import limit");
}

// A literal must not contain a reference after its first character (SPEC §2.1, W213); such text
// is real content of the UI, so the brace is replaced by a look-alike instead of dropping it.
function literal(ctx: Ctx, path: string, s: string): string {
  const out = clean(s);
  if (out.search(EMBEDDED_REFERENCE) <= 0) return out;
  lose(ctx, "text", path, "text that reads as a binding or token reference had its brace replaced");
  return out[0] + out.slice(1).replace(REFERENCE_START, "｛");
}

function freshId(ctx: Ctx, base: string, name: string): string {
  const s = slug(name);
  if (s !== "" && !ctx.used.has(`${base}-${s}`)) {
    ctx.used.add(`${base}-${s}`);
    return `${base}-${s}`;
  }
  // Counters remember where the last search stopped, so many equal names stay linear.
  const stem = s === "" ? base : `${base}-${s}`;
  let n = ctx.counters.get(stem) ?? (s === "" ? 1 : 2);
  while (ctx.used.has(`${stem}-${n}`)) n++;
  ctx.counters.set(stem, n + 1);
  ctx.used.add(`${stem}-${n}`);
  return `${stem}-${n}`;
}

// A generated id reads as the kind plus the element's name, or its text for kinds that show
// text; containers without a name are numbered.
function takeId(ctx: Ctx, s: Sem, base: string, showsText = false): string {
  if (s.id !== undefined && ID.test(s.id) && !ctx.claimed.has(s.id)) {
    ctx.claimed.add(s.id);
    ctx.used.add(s.id);
    return s.id;
  }
  return freshId(ctx, base, s.name || (showsText ? textOf(s.children) : ""));
}

const textRun = (name: string): Sem => ({
  role: "text",
  name,
  states: {},
  props: {},
  children: [],
});

function flatten(list: readonly Sem[], ctx: Ctx, depth: number, path: string): Sem[] {
  const out: Sem[] = [];
  for (const s of list) {
    if (s.kind !== undefined || !DISSOLVED_ROLES.has(s.role)) {
      out.push(s);
      continue;
    }
    if (depth > MAX_DEPTH) {
      truncate(ctx, path);
      continue;
    }
    out.push(...flatten(s.children, ctx, depth + 1, path));
  }
  return out;
}

function placement(
  kind: string,
  def: ComponentDef,
  parent: Parent,
): { slot?: string | undefined } | { why: string } {
  if (kind === "screen") return { why: "a screen appears only at the root" };
  // Extension parents are opaque: parent/child rules skip them (SPEC §8).
  if (parent.def === undefined) return {};
  if (def.allowedParents && !def.allowedParents.includes(parent.kind))
    return { why: `<${kind}> belongs only in ${def.allowedParents.join(", ")}` };
  const allowed = parent.def.allowedChildren;
  if (!allowed || allowed.includes(kind)) return {};
  for (const [name, slot] of Object.entries(parent.def.slots ?? {}).toSorted()) {
    if (!slot.allowedChildren || slot.allowedChildren.includes(kind)) return { slot: name };
  }
  return { why: `<${parent.kind}> does not hold <${kind}>` };
}

function coerce(def: PropDef, raw: Scalar): Value | undefined {
  switch (def.type) {
    case "string":
      return String(raw);
    case "boolean":
      return raw === true || raw === "true"
        ? true
        : raw === false || raw === "false"
          ? false
          : undefined;
    case "number": {
      const n = typeof raw === "number" ? raw : String(raw).trim() === "" ? NaN : Number(raw);
      if (!Number.isFinite(n) || (def.integer === true && !Number.isInteger(n))) return undefined;
      if ((def.min !== undefined && n < def.min) || (def.max !== undefined && n > def.max))
        return undefined;
      return n;
    }
    case "enum":
      return typeof raw === "string" && (def.values ?? []).includes(raw) ? raw : undefined;
    default:
      // A rendered UI shows resolved values, never token references.
      return undefined;
  }
}

function setProp(
  ctx: Ctx,
  props: Record<string, Value>,
  def: ComponentDef,
  name: string,
  raw: Scalar,
  at: { path: string; inForm: boolean },
): void {
  if (name === "state") {
    if (typeof raw === "string" && (def.states ?? []).includes(raw)) props["state"] = raw;
    else lose(ctx, "props", at.path, `state "${String(raw)}" is not a state of this component`);
    return;
  }
  const pd = def.props && Object.hasOwn(def.props, name) ? def.props[name] : undefined;
  if (!pd || name === "text") return;
  // SPEC §5.1: a submit button outside a form is an error (W313), so the flag is not kept there.
  if (name === "submit" && !at.inForm) {
    lose(ctx, "props", at.path, "a submit button outside a form is imported as a plain button");
    return;
  }
  const value = coerce(pd, raw);
  if (value === undefined) {
    lose(ctx, "props", at.path, `${name}="${String(raw)}" is not a valid value`);
    return;
  }
  props[name] = typeof value === "string" ? literal(ctx, at.path, value) : value;
}

function fillRequired(
  ctx: Ctx,
  props: Record<string, Value>,
  def: ComponentDef,
  parent: Parent,
  s: Sem,
  path: string,
): void {
  for (const [name, pd] of Object.entries(def.props ?? {})) {
    if (pd.required !== true || Object.hasOwn(props, name)) continue;
    // The root's `weft` is `Document.weft`, never a prop (SPEC §3).
    if (name === "weft" && parent.kind === "") continue;
    // A value the parent selects by (radio and option `value`) must tell the children apart.
    const selects = parent.def?.props?.[name]?.writable === true;
    let value: Value;
    if (pd.type === "number") value = pd.default ?? pd.min ?? 0;
    else if (pd.type === "boolean") value = pd.default ?? false;
    else if (pd.type === "enum") value = pd.default ?? pd.values?.[0] ?? "";
    else value = selects ? slug(textOf(s.children) || s.name) || name : "";
    props[name] = value;
    lose(
      ctx,
      "values",
      path,
      `required ${name} is not in the input; ${JSON.stringify(value)} stands in`,
    );
  }
}

function convertList(
  list: readonly Sem[],
  parent: Parent,
  ctx: Ctx,
): { children: Child[]; slots: Record<string, Child[]>; chosen: Node | undefined } {
  const items = flatten(list, ctx, parent.depth, parent.path);
  const children: Child[] = [];
  const slots: Record<string, Child[]> = {};
  let chosen: Node | undefined;
  let pending = "";
  const place = (p: Placed) => {
    if (p.slot === undefined) children.push(p.node);
    else (slots[p.slot] ??= []).push(p.node);
  };
  const flush = () => {
    const text = squash(clean(pending));
    pending = "";
    if (text === "") return;
    const content = parent.def?.content ?? "mixed";
    if (content === "mixed") children.push(text);
    else if (content === "nodes" && ctx.index.text !== undefined) {
      const placed = convertNode(
        { ...textRun(text), role: "paragraph", kind: ctx.index.text, children: [textRun(text)] },
        parent,
        ctx,
      );
      if (placed) place(placed);
    } else lose(ctx, "text", parent.path, `text "${text}" has no place in <${parent.kind}>`);
  };
  for (let i = 0; i < items.length; i++) {
    const s = items[i]!;
    if (s.role === "text") {
      pending += ` ${s.name}`;
      continue;
    }
    flush();
    if (s.kind === undefined && s.role === "tablist") {
      const panels: Sem[] = [];
      while (items[i + 1]?.role === "tabpanel" && items[i + 1]?.kind === undefined)
        panels.push(items[++i]!);
      for (const p of convertTabs(s, panels, parent, ctx)) place(p);
      continue;
    }
    if (isHeaderRow(s, parent, ctx)) {
      items.splice(i + 1, 0, ...flatten(s.children, ctx, parent.depth, parent.path));
      continue;
    }
    const placed = convertNode(s, parent, ctx);
    if (!placed) continue;
    place(placed);
    const node = placed.node;
    for (const state of ["checked", "selected"]) {
      // A state the child kind cannot hold is the parent's choice (radio in radio-group, option
      // in select); `node.props.value` names the choice.
      const own = node.props && Object.hasOwn(node.props, state);
      if (s.states[state] === true && !own && node.props?.["value"] !== undefined) chosen ??= node;
    }
  }
  flush();
  return { children, slots, chosen };
}

// The header row a renderer emits for a table's columns (SPEC §5.1 notes) dissolves back into
// the table's column children.
function isHeaderRow(s: Sem, parent: Parent, ctx: Ctx): boolean {
  if (s.kind !== undefined || s.role !== "row") return false;
  if (parent.kind !== ctx.index.byRole.get("table")) return false;
  const cells = flatten(s.children, ctx, parent.depth, parent.path);
  return (
    cells.some((c) => c.role === "columnheader") &&
    cells.every((c) => c.role === "columnheader" || (c.role === "text" && c.name.trim() === ""))
  );
}

function convertNode(s: Sem, parent: Parent, ctx: Ctx): Placed | undefined {
  if (++ctx.nodes > MAX_NODES || parent.depth >= MAX_DEPTH) {
    truncate(ctx, parent.path);
    return undefined;
  }
  const target = resolveKind(ctx.index, s.role, s.kind);
  if (!target) return { node: extension(s, parent, ctx, undefined) };
  const where = placement(target.kind, target.def, parent);
  if ("why" in where) return { node: extension(s, parent, ctx, where.why) };
  return {
    node: componentNode(s, target.kind, target.def, target.preset, parent, where.slot, ctx),
    slot: where.slot,
  };
}

function extension(s: Sem, parent: Parent, ctx: Ctx, why: string | undefined): Node {
  const role = ROLES.has(s.role) && !DISSOLVED_ROLES.has(s.role) ? s.role : "group";
  const kind = `x-aria-${role}`;
  const id = takeId(ctx, s, role);
  const path = `${parent.path}/${kind}#${id}`;
  report(ctx, s, path);
  lose(
    ctx,
    "kinds",
    path,
    why ??
      (role === s.role
        ? `role ${role} has no kind in the catalog`
        : `"${s.role}" is not a WAI-ARIA role`),
  );
  const props: Record<string, Value> = { role };
  const name = squash(s.name);
  if (name !== "") props["label"] = literal(ctx, path, name);
  const inner = convertList(
    s.children,
    { kind, def: undefined, path, inForm: parent.inForm, depth: parent.depth + 1 },
    ctx,
  );
  return { kind, id, props, children: inner.children };
}

function report(ctx: Ctx, s: Sem, path: string): void {
  for (const n of s.notes ?? []) lose(ctx, n.kind, path, n.note);
}

function componentNode(
  s: Sem,
  kind: string,
  def: ComponentDef,
  preset: Readonly<Record<string, string>>,
  parent: Parent,
  slot: string | undefined,
  ctx: Ctx,
): Node {
  const id = takeId(ctx, s, kind, def.content === "text");
  const path = `${parent.path}${slot === undefined ? "" : `/slot[${slot}]`}/${kind}#${id}`;
  report(ctx, s, path);
  const at = { path, inForm: parent.inForm };
  const props: Record<string, Value> = {};
  for (const [name, raw] of Object.entries({ ...preset, ...s.props }))
    setProp(ctx, props, def, name, raw, at);
  for (const [name, raw] of Object.entries(s.states)) {
    if (name === "level" || def.props?.[name]?.type === "boolean") {
      if (raw !== false && def.props && Object.hasOwn(def.props, name))
        setProp(ctx, props, def, name, raw, at);
    } else if (name === "invalid" && raw === true && props["state"] === undefined) {
      if ((def.states ?? []).includes("invalid")) props["state"] = "invalid";
    } else if (name === "busy" && raw === true && props["state"] === undefined) {
      const busy = BUSY_STATES.find((b) => (def.states ?? []).includes(b));
      if (busy !== undefined) props["state"] = busy;
    }
  }

  const name = squash(s.name);
  let children: Child[] = [];
  let slots: Record<string, Child[]> = {};
  let content = "";
  if (def.content === "none") {
    // An input's value is the text it exposes (a textbox's text run in a snapshot).
    const shown = textOf(s.children);
    if (
      shown !== "" &&
      def.props &&
      Object.hasOwn(def.props, "value") &&
      props["value"] === undefined
    )
      setProp(ctx, props, def, "value", shown, at);
    else if (shown !== "") lose(ctx, "text", path, `content "${shown}" has no place in <${kind}>`);
  } else if (def.content === "text") {
    content = textOf(s.children) || name;
    if (content !== "") children = [clean(content)];
  } else {
    const inner = convertList(
      s.children,
      { kind, def, path, inForm: parent.inForm || kind === ctx.formKind, depth: parent.depth + 1 },
      ctx,
    );
    children = inner.children;
    slots = inner.slots;
    for (const [slotName, list] of Object.entries(slots)) {
      if (list.length > 0)
        lose(
          ctx,
          "slots",
          `${path}/slot[${slotName}]`,
          `content <${kind}> does not hold by default is placed in its ${slotName} slot`,
        );
    }
    content = textOf(s.children);
    const value = inner.chosen?.props?.["value"];
    if (
      value !== undefined &&
      def.props?.["value"]?.writable === true &&
      props["value"] === undefined
    )
      props["value"] = value;
  }
  // A name that only repeats the content is computed from it, not a label of its own.
  if (name !== "" && (name !== content || def.requiresLabel === true))
    props["label"] = literal(ctx, path, name);
  if (def.requiresLabel === true && props["label"] === undefined) {
    props["label"] = "";
    lose(ctx, "names", path, `<${kind}> needs an accessible name and the input gives none`);
  }
  fillRequired(ctx, props, def, parent, s, path);
  const node: Node = { kind, id, props };
  if (Object.keys(slots).length > 0) node.slots = slots;
  if (children.length > 0) node.children = children;
  return node;
}

// SPEC §5.1: a renderer emits one tablist of tab buttons followed by tab panels; each panel goes
// back into the tab it belongs to.
function convertTabs(list: Sem, panels: readonly Sem[], parent: Parent, ctx: Ctx): Placed[] {
  const tabsKind = ctx.index.byRole.get("tablist");
  const tabKind = ctx.index.byRole.get("tab");
  const tabsDef = tabsKind === undefined ? undefined : component(ctx.index, tabsKind);
  const tabDef = tabKind === undefined ? undefined : component(ctx.index, tabKind);
  const where = tabsKind && tabsDef ? placement(tabsKind, tabsDef, parent) : { why: "" };
  if (!tabsKind || !tabsDef || !tabKind || !tabDef || "why" in where) {
    return [list, ...panels].flatMap((s) => convertNode(s, parent, ctx) ?? []);
  }
  if (++ctx.nodes > MAX_NODES || parent.depth >= MAX_DEPTH) {
    truncate(ctx, parent.path);
    return [];
  }
  const id = takeId(ctx, list, tabsKind);
  const path = `${parent.path}${where.slot === undefined ? "" : `/slot[${where.slot}]`}/${tabsKind}#${id}`;
  report(ctx, list, path);
  const items = flatten(list.children, ctx, parent.depth + 1, path);
  const tabs = items.filter((s) => s.kind === undefined && s.role === "tab");
  const owner = new Map<Sem, Sem>();
  const free = (t: Sem) => ![...owner.values()].includes(t);
  const extra: Sem[] = [];
  for (const panel of panels) {
    const tab =
      tabs.find(
        (t) => free(t) && t.ref !== undefined && (panel.labelledBy ?? []).includes(t.ref),
      ) ??
      tabs.find(
        (t) =>
          free(t) &&
          panel.name !== "" &&
          squash(t.name || textOf(t.children)) === squash(panel.name),
      ) ??
      tabs.find((t) => free(t) && t.states["selected"] === true) ??
      tabs.find(free);
    if (tab) owner.set(panel, tab);
    else extra.push(panel);
  }
  const self: Parent = {
    kind: tabsKind,
    def: tabsDef,
    path,
    inForm: parent.inForm,
    depth: parent.depth + 1,
  };
  const children: Child[] = [];
  let selected: string | undefined;
  for (const item of items) {
    if (item.role === "text") continue;
    if (!tabs.includes(item)) {
      const placed = convertNode(item, self, ctx);
      if (placed) children.push(placed.node);
      continue;
    }
    const tabId = takeId(ctx, item, tabKind);
    const tabPath = `${path}/${tabKind}#${tabId}`;
    report(ctx, item, tabPath);
    const panel = [...owner].find(([, t]) => t === item)?.[0];
    const inner = panel
      ? convertList(
          panel.children,
          {
            kind: tabKind,
            def: tabDef,
            path: tabPath,
            inForm: parent.inForm,
            depth: parent.depth + 2,
          },
          ctx,
        )
      : undefined;
    if (!panel)
      lose(
        ctx,
        "hidden",
        tabPath,
        "the panel of this tab is not in the input (only the selected panel is exposed)",
      );
    const node: Node = {
      kind: tabKind,
      id: tabId,
      props: { label: literal(ctx, tabPath, squash(item.name) || textOf(item.children)) },
    };
    if (inner && inner.children.length > 0) node.children = inner.children;
    if (inner && Object.keys(inner.slots).length > 0) node.slots = inner.slots;
    if (item.states["selected"] === true) selected ??= tabId;
    children.push(node);
  }
  const props: Record<string, Value> = {};
  const name = squash(list.name);
  if (name !== "") props["label"] = literal(ctx, path, name);
  if (selected !== undefined && tabsDef.props?.["selected"]?.type === "string")
    props["selected"] = selected;
  const node: Node = { kind: tabsKind, id, props };
  if (children.length > 0) node.children = children;
  return [
    { node, slot: where.slot },
    ...extra.flatMap((p) => {
      const placed = convertNode(p, parent, ctx);
      return placed ? [placed] : [];
    }),
  ];
}

export type BuildOptions = {
  catalog: Catalog;
  // Ids the source carries; generated ids avoid them.
  reserved?: Iterable<string>;
  diagnostics?: Diagnostic[];
};

export type Built = ImportResult & { rootPath: string };

export function buildDocument(top: readonly Sem[], options: BuildOptions): Built {
  const index = kindIndex(options.catalog);
  const ctx: Ctx = {
    index,
    formKind: index.byRole.get("form"),
    used: new Set(options.reserved ?? []),
    claimed: new Set(),
    counters: new Map(),
    losses: [],
    diagnostics: options.diagnostics ?? [],
    nodes: 0,
    truncated: false,
  };
  const items = flatten(top, ctx, 0, "");
  const shown = items.filter((s) => s.role !== "text" || s.name.trim() !== "");
  const only = shown.length === 1 ? shown[0] : undefined;
  const main =
    only && (only.kind === "screen" || (only.kind === undefined && only.role === "main"))
      ? only
      : undefined;
  const root: Sem = main ?? { role: "main", name: "", states: {}, props: {}, children: [...items] };
  if (!main && !ctx.used.has("screen")) root.id = "screen";
  const def = component(index, "screen");
  const none: Parent = { kind: "", def: undefined, path: "", inForm: false, depth: 0 };
  const node = def
    ? componentNode(root, "screen", def, {}, none, undefined, ctx)
    : extensionRoot(root, ctx);
  const rootPath = `/screen#${node.id ?? ""}`;
  if (!main)
    lose(
      ctx,
      "structure",
      rootPath,
      "the input has no single main landmark; a screen was added as the root",
    );
  const document = canonicalize({ weft: WEFT_VERSION, root: node });
  ctx.diagnostics.push(...validate(document, { catalog: options.catalog }));
  return { document, losses: ctx.losses, diagnostics: ctx.diagnostics, rootPath };
}

// A catalog without `screen` still gets the root SPEC §2 requires; validation reports the rest.
function extensionRoot(root: Sem, ctx: Ctx): Node {
  const id = takeId(ctx, root, "screen");
  const inner = convertList(
    root.children,
    { kind: "screen", def: undefined, path: `/screen#${id}`, inForm: false, depth: 1 },
    ctx,
  );
  return { kind: "screen", id, children: inner.children };
}

export function emptyResult(diagnostics: Diagnostic[]): ImportResult {
  return {
    document: { weft: WEFT_VERSION, root: { kind: "screen", id: "screen" } },
    losses: [{ kind: "structure", path: "/screen#screen", note: "nothing could be imported" }],
    diagnostics,
  };
}
