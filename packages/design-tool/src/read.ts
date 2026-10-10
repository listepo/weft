// Design tool to Weft. A layer built from Weft is rebuilt from its stored source, and its state in
// the tool is compared with the state that source builds (`view.ts`): what is unchanged keeps the
// source as written, so an unedited screen comes back byte-identical; what differs is a designer's
// edit and is written into the element. Layers that did not come from Weft convert lossily
// (`foreign.ts`).
import type { Token } from "@weft/catalog";
import {
  WEFT_VERSION,
  type Catalog,
  type Child,
  type ComponentDef,
  type Diagnostic,
  type Document,
  type Entry,
  type Node,
  type Value,
} from "@weft/core";
import {
  fillRequired,
  freshId,
  limitReached,
  literal,
  MAX_DEPTH,
  MAX_NODES,
  type Loss,
  type LossKind,
} from "@weft/from-aria";
import { convertForeign } from "./foreign.ts";
import { KEY, readContext, readMark, readSource, readVersion, type Source } from "./keys.ts";
import { findText, isContainer, type Layer } from "./layer.ts";
import { matchToken, tokenGroup, tokenPx } from "./tokens.ts";
import {
  CAPTION_KINDS,
  isLeafKind,
  LAYOUT_KINDS,
  layoutView,
  UNSET,
  variantAxes,
  variantOf,
  type LayoutView,
  type Mode,
} from "./view.ts";

export type ReadOptions = {
  catalog: Catalog;
  tokens?: ReadonlyMap<string, Token> | undefined;
  /** `drop` leaves the root's context unread, as `import.figma.context` asks (SPEC §10.6). */
  context?: "keep" | "drop" | undefined;
};

export type ReadResult = { document: Document; losses: Loss[]; diagnostics: Diagnostic[] };

/**
 * Text a designer typed into a `text` or `label`, not yet read as a value: it may be a reference
 * such as `{$.name}`. Reading it takes the core's value rules, so `readLayers` leaves this in the
 * prop and `finishRead` replaces it. With `content`, a plain string becomes the element's content.
 */
export type RawText = { raw: string; path: string; content?: true };

export function isRawText(value: unknown): value is RawText {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as Partial<RawText>).raw === "string" &&
    typeof (value as Partial<RawText>).path === "string"
  );
}

// Stands where a value goes until `finishRead`; the document type has no place for it.
const rawText = (raw: string, path: string, content: boolean): Value =>
  (content ? { raw, path, content: true } : { raw, path }) as unknown as Value;

export type ReadCtx = ReadOptions & {
  used: Set<string>;
  counters: Map<string, number>;
  losses: Loss[];
  diagnostics: Diagnostic[];
  /** Weft id → the tool's id of the layer that keeps it; copies of that layer get new ids. */
  keepers: Map<string, string>;
  /** Token groups the document's `gap` values use, most used first, to break ties. */
  gapGroups: string[];
  nodes: number;
  truncated: boolean;
};

export type Parent = {
  kind: string;
  def: ComponentDef | undefined;
  path: string;
  mode: Mode | "none";
};

export const lose = (ctx: ReadCtx, kind: LossKind, path: string, note: string): void => {
  ctx.losses.push({ kind, path, note });
};

export const STYLE_NOTE =
  "Weft has no prop for this visual edit; it is not carried (see docs/figma-style-overrides-design.md)";

export function component(catalog: Catalog, kind: string): ComponentDef | undefined {
  return Object.hasOwn(catalog.components, kind) ? catalog.components[kind] : undefined;
}

/**
 * Reads a layer built by `buildScreen` (or any layer) back into a Weft document that is not yet
 * finished: typed text is still `RawText`, and nothing is canonical or validated. `finishRead`
 * does that with the WebAssembly core, which a plugin's sandbox cannot run, so the plugin finishes
 * in its UI. `readScreen` does both where the core runs.
 */
export async function readLayers(layer: Layer, options: ReadOptions): Promise<ReadResult> {
  const ctx: ReadCtx = {
    ...options,
    used: new Set(),
    counters: new Map(),
    losses: [],
    diagnostics: [],
    keepers: new Map(),
    gapGroups: [],
    nodes: 0,
    truncated: false,
  };
  const context = options.context === "drop" ? [] : (readContext(layer) ?? []);
  // Reserved first, so an element the designer added cannot take an entry's id.
  for (const entry of context) if (entry.id !== undefined) ctx.used.add(entry.id);
  survey(ctx, layer);
  const top: Parent = { kind: "", def: undefined, path: "", mode: "column" };
  const converted = await convertLayer(ctx, layer, top, 0);
  let root: Node | undefined = converted.find((c): c is Node => typeof c !== "string");
  if (root === undefined || root.kind !== "screen" || converted.length !== 1) {
    const id = freshId(ctx, "screen", "");
    const children = converted;
    root = { kind: "screen", id, children };
    lose(
      ctx,
      "structure",
      `/screen#${id}`,
      "the selection is not a Weft screen; a screen was added as the root",
    );
  }
  const document: Document = { weft: readVersion(layer) ?? WEFT_VERSION, root };
  const kept = keptContext(ctx, context, root);
  if (kept.length > 0) document.context = kept;
  return { document, losses: ctx.losses, diagnostics: ctx.diagnostics };
}

/** Visits every element of the tree: children and slot content, `<each>` included. */
export function eachNode(root: Node, visit: (node: Node) => void): void {
  const stack = [root];
  for (let node = stack.pop(); node !== undefined; node = stack.pop()) {
    visit(node);
    for (const child of node.children ?? []) if (typeof child !== "string") stack.push(child);
    for (const list of Object.values(node.slots ?? {}))
      for (const child of list) if (typeof child !== "string") stack.push(child);
  }
}

/**
 * The entries still about something in the file. The designer deleted the layer an entry names,
 * so the entry goes with it, as a `context` loss, rather than failing the read with `W309`.
 */
function keptContext(ctx: ReadCtx, context: readonly Entry[], root: Node): Entry[] {
  if (context.length === 0) return [];
  const ids = new Set<string>();
  eachNode(root, (node) => {
    if (node.id !== undefined) ids.add(node.id);
  });
  const at = `/${root.kind}${root.id === undefined ? "" : `#${root.id}`}/context`;
  return context.filter((entry, index) => {
    if (entry.for === undefined || ids.has(entry.for)) return true;
    const name = entry.id === undefined ? `entry[${index}]` : `entry#${entry.id}`;
    lose(ctx, "context", `${at}/${name}`, "the layer this entry is about was removed");
    return false;
  });
}

/** First pass: reserve every stored id, choose which layer keeps a copied id, count gap groups. */
function survey(ctx: ReadCtx, layer: Layer): void {
  const groups = new Map<string, number>();
  const firstHolder = new Map<string, string>();
  const stack: { layer: Layer; depth: number }[] = [{ layer, depth: 0 }];
  let seen = 0;
  while (stack.length > 0 && seen < MAX_NODES) {
    const { layer: l, depth } = stack.pop() as { layer: Layer; depth: number };
    seen++;
    const source = readSource(l);
    if (source?.id !== undefined) {
      ctx.used.add(source.id);
      if (!firstHolder.has(source.id)) firstHolder.set(source.id, l.id);
      if (l.getPluginData(KEY.origin) === l.id) ctx.keepers.set(source.id, l.id);
      const gap = source.props?.["gap"];
      if (typeof gap === "object" && "token" in gap) {
        const group = tokenGroup(gap.token);
        groups.set(group, (groups.get(group) ?? 0) + 1);
      }
    }
    // Instances of leaf kinds hold only their drawing; their children are not elements.
    if (isContainer(l) && depth < MAX_DEPTH && !(l.kind === "instance" && source !== undefined))
      for (const child of [...(l.children ?? [])].reverse())
        stack.push({ layer: child, depth: depth + 1 });
  }
  // Without the layer the id was built on (a copied file), the first holder in order keeps it.
  for (const [id, holder] of firstHolder) if (!ctx.keepers.has(id)) ctx.keepers.set(id, holder);
  ctx.gapGroups = [...groups].sort((a, b) => b[1] - a[1]).map(([g]) => g);
}

function overLimit(ctx: ReadCtx, path: string, depth: number): boolean {
  ctx.nodes++;
  if (ctx.nodes <= MAX_NODES && depth <= MAX_DEPTH) return false;
  if (!ctx.truncated) {
    ctx.truncated = true;
    limitReached(ctx.diagnostics, path, "is larger or deeper than the import limit");
  }
  return true;
}

/** Converts one layer; a layer gives no element (dropped), one element, or text. */
export async function convertLayer(
  ctx: ReadCtx,
  layer: Layer,
  parent: Parent,
  depth: number,
): Promise<Child[]> {
  if (overLimit(ctx, parent.path, depth)) return [];
  const source = readSource(layer);
  if (source !== undefined) return [await fromSource(ctx, layer, source, parent, depth)];
  if (layer.kind === "instance") {
    const kind = (await layer.main())?.kind;
    const def = kind === undefined ? undefined : component(ctx.catalog, kind);
    if (kind !== undefined && def !== undefined)
      return [await fromNewInstance(ctx, layer, kind, def, parent)];
  }
  return convertForeign(ctx, layer, parent, depth);
}

/** The children of a container layer: elements, text and named slots. */
export async function convertChildren(
  ctx: ReadCtx,
  layers: readonly Layer[],
  owner: Parent,
  depth: number,
): Promise<{ children: Child[]; slots: [string, Child[]][] }> {
  const children: Child[] = [];
  const slots: [string, Child[]][] = [];
  for (const child of layers) {
    if (readMark(child, KEY.label) !== undefined) continue;
    const slot = readMark(child, KEY.slot);
    if (slot !== undefined && isContainer(child)) {
      const path = `${owner.path}/slot[${slot}]`;
      const inner = await convertChildren(
        ctx,
        child.children ?? [],
        { ...owner, path, mode: child.layout.mode },
        depth + 1,
      );
      const existing = slots.find(([name]) => name === slot);
      if (existing === undefined) slots.push([slot, inner.children]);
      else {
        existing[1].push(...inner.children);
        lose(ctx, "slots", path, "two layers held this slot; their content was joined");
      }
      continue;
    }
    if (child.kind === "text" && readMark(child, KEY.text) !== undefined) {
      if (child.visible && child.characters.trim() !== "")
        children.push(literal(ctx, owner.path, child.characters));
      else if (!child.visible) lose(ctx, "hidden", owner.path, "a hidden text layer was dropped");
      continue;
    }
    children.push(...(await convertLayer(ctx, child, owner, depth + 1)));
  }
  return { children, slots };
}

function idFor(ctx: ReadCtx, layer: Layer, source: Source, path: string): string | undefined {
  const { id } = source;
  if (id === undefined) return undefined;
  if (ctx.keepers.get(id) === layer.id) return id;
  const fresh = freshId(ctx, source.kind, id);
  lose(
    ctx,
    "ids",
    `${path}/${source.kind}#${fresh}`,
    `a copy of ${source.kind}#${id} got a new id`,
  );
  return fresh;
}

async function fromSource(
  ctx: ReadCtx,
  layer: Layer,
  source: Source,
  parent: Parent,
  depth: number,
): Promise<Node> {
  const id = idFor(ctx, layer, source, parent.path);
  const path = `${parent.path}/${source.kind}${id === undefined ? "" : `#${id}`}`;
  const props: Record<string, Value> = { ...source.props };
  const def = component(ctx.catalog, source.kind);
  const node: Node = { kind: source.kind, id, props, on: source.on };

  // Visibility: Weft's `hidden` is a hidden layer.
  const shown = props["hidden"] !== true;
  if (layer.visible !== shown) {
    if (layer.visible) delete props["hidden"];
    else props["hidden"] = true;
  }

  if (
    isLeafKind(def) &&
    !(isContainer(layer) && (layer.children ?? []).some((c) => readSource(c) !== undefined))
  ) {
    readLeafText(layer, source, node, def as ComponentDef, path);
    if (layer.kind === "instance")
      await readVariants(ctx, layer, source, def as ComponentDef, props, path);
    if (CAPTION_KINDS.has(source.kind))
      readLabel(findText(layer, "label")?.characters, source, props, def, path);
    return node;
  }

  if (!isContainer(layer)) {
    lose(
      ctx,
      "structure",
      path,
      "the layer of this element is no longer a frame; its content was dropped",
    );
    return node;
  }
  const view = layoutView(source.kind, source.props, ctx.tokens, parent.mode);
  if (LAYOUT_KINDS.has(source.kind)) await readLayout(ctx, layer, view, source, props, path);
  else if (layer.getPluginData(KEY.style) !== layer.style(true))
    lose(ctx, "tokens", path, STYLE_NOTE);
  const own = layer.children ?? [];
  if (CAPTION_KINDS.has(source.kind)) {
    const label = own.find((c) => readMark(c, KEY.label) !== undefined);
    readLabel(label?.kind === "text" ? label.characters : undefined, source, props, def, path);
  }
  const owner: Parent = { kind: source.kind, def, path, mode: layer.layout.mode };
  const { children, slots } = await convertChildren(ctx, own, owner, depth);
  node.children = children;
  node.slots = Object.fromEntries(slots);
  return node;
}

/** A text edit: the leaf's text layer against what it showed when built. */
function readLeafText(
  layer: Layer,
  source: Source,
  node: Node,
  def: ComponentDef,
  path: string,
): void {
  node.children = source.children;
  if (def.content !== "text") return;
  const shown = findText(layer, "text")?.characters;
  if (shown === undefined || shown === (source.shown?.text ?? "")) return;
  const props = node.props as Record<string, Value>;
  // A `text` prop stays a prop; otherwise plain text is content and a reference becomes `text`.
  const content = props["text"] === undefined;
  if (shown === "") delete props["text"];
  else props["text"] = rawText(shown, path, content);
  node.children = [];
}

function readLabel(
  shown: string | undefined,
  source: Source,
  props: Record<string, Value>,
  def: ComponentDef | undefined,
  path: string,
): void {
  if (shown === undefined || shown === (source.shown?.label ?? "")) return;
  if (shown !== "") props["label"] = rawText(shown, path, false);
  else if (def?.requiresLabel === true) props["label"] = "";
  else delete props["label"];
}

async function readVariants(
  ctx: ReadCtx,
  layer: Layer,
  source: Source,
  def: ComponentDef,
  props: Record<string, Value>,
  path: string,
): Promise<void> {
  const axes = variantAxes(def);
  const expected = variantOf(source.props, axes);
  for (const axis of axes) {
    const now = Object.hasOwn(layer.variant, axis.name) ? layer.variant[axis.name] : undefined;
    if (now === undefined || now === expected[axis.name]) continue;
    if (now === UNSET) delete props[axis.name];
    else if (axis.values.includes(now)) props[axis.name] = now;
    else lose(ctx, "props", path, `variant ${axis.name}=${now} is not a value Weft knows`);
  }
  const main = await layer.main();
  if (main !== undefined && main.style !== layer.style(true)) lose(ctx, "tokens", path, STYLE_NOTE);
}

async function readLayout(
  ctx: ReadCtx,
  layer: Layer,
  view: LayoutView,
  source: Source,
  props: Record<string, Value>,
  path: string,
): Promise<void> {
  const grid = source.kind === "grid";
  const { layout } = layer;
  if (grid) {
    if (layout.mode !== "grid")
      lose(ctx, "layout", path, "a grid is no longer a grid layout; its layout is kept");
    else if (layout.columns !== view.columns) props["columns"] = layout.columns;
  } else if (layout.mode !== view.mode) {
    if (layout.mode === "row" || layout.mode === "column") props["direction"] = layout.mode;
    else lose(ctx, "layout", path, `layout ${layout.label} has no Weft stack direction`);
  }
  if (!grid && layout.align !== view.align) {
    if (layout.align === undefined)
      lose(ctx, "layout", path, `alignment ${layout.alignLabel} has no Weft value`);
    else props["align"] = layout.align;
  }
  if (!grid && layout.wrap !== view.wrap) {
    if (layout.wrap) props["wrap"] = true;
    else delete props["wrap"];
  }
  await readGap(ctx, layer, view, source, props, path, grid);
  if (layer.getPluginData(KEY.style) !== layer.style(false)) lose(ctx, "tokens", path, STYLE_NOTE);
}

async function readGap(
  ctx: ReadCtx,
  layer: Layer,
  view: LayoutView,
  source: Source,
  props: Record<string, Value>,
  path: string,
  grid: boolean,
): Promise<void> {
  const px = layer.gap(grid);
  let bound = await layer.gapToken(grid);
  // A binding whose value no longer matches its token was overridden by hand.
  if (
    bound !== undefined &&
    ctx.tokens?.has(bound) === true &&
    tokenPx(ctx.tokens.get(bound)) !== px
  )
    bound = undefined;
  if (bound !== undefined) {
    if (bound !== view.gap.token) props["gap"] = { token: bound };
    return;
  }
  if (px === view.gap.px) return;
  const previous = view.gap.token;
  const def = component(ctx.catalog, source.kind);
  const tokenType = def?.props?.["gap"]?.tokenType;
  const match =
    ctx.tokens === undefined
      ? undefined
      : matchToken(px, ctx.tokens, tokenType, previous, ctx.gapGroups);
  if (match !== undefined) props["gap"] = { token: match };
  else if (px === 0) delete props["gap"];
  else
    lose(ctx, "tokens", path, `gap ${px}px matches no ${tokenType ?? ""} token; the gap is kept`);
}

/** A library instance the designer added: a new element with a generated id. */
async function fromNewInstance(
  ctx: ReadCtx,
  layer: Layer,
  kind: string,
  def: ComponentDef,
  parent: Parent,
): Promise<Node> {
  const props: Record<string, Value> = {};
  for (const axis of variantAxes(def)) {
    const v = Object.hasOwn(layer.variant, axis.name) ? layer.variant[axis.name] : undefined;
    const declared = axis.name === "state" ? undefined : def.props?.[axis.name]?.default;
    if (v !== undefined && v !== UNSET && axis.values.includes(v) && v !== declared)
      props[axis.name] = v;
  }
  if (!layer.visible) props["hidden"] = true;
  const shown = def.content === "text" ? (findText(layer, "text")?.characters ?? "") : "";
  const label = CAPTION_KINDS.has(kind) ? (findText(layer, "label")?.characters ?? "") : "";
  const id = freshId(ctx, kind, shown || label);
  const path = `${parent.path}/${kind}#${id}`;
  const node: Node = { kind, id, props };
  if (label !== "") props["label"] = rawText(label, path, false);
  if (shown !== "") props["text"] = rawText(shown, path, true);
  if (def.requiresLabel === true && props["label"] === undefined) {
    props["label"] = "";
    lose(ctx, "names", path, 'the layer has no label; "" stands in');
  }
  fillRequired(ctx, props, def, parent.def, false, shown || label, path);
  return node;
}
