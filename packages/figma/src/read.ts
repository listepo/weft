// Figma to Weft. A layer built from Weft is rebuilt from its stored source, and its Figma state is
// compared with the state that source builds (`view.ts`): what is unchanged keeps the source as
// written, so an unedited screen comes back byte-identical; what differs is a designer's edit and
// is written into the element. Layers that did not come from Weft convert lossily (`foreign.ts`).
import type { Token } from "@weft/catalog";
import {
  WEFT_VERSION,
  type Catalog,
  type Child,
  type ComponentDef,
  type Diagnostic,
  type Document,
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
import type { FAlias, FigmaApi, FLayout, FNode } from "./api.ts";
import { isContainer } from "./api.ts";
import { findText } from "./build.ts";
import { convertForeign } from "./foreign.ts";
import { KEY, readMark, readSource, readVersion, type Source } from "./keys.ts";
import { matchToken, tokenPathOf, tokenPx } from "./tokens.ts";
import { readText } from "./values.ts";
import {
  CAPTION_KINDS,
  isLeafKind,
  labelDisplay,
  LAYOUT_KINDS,
  layoutView,
  styleKey,
  textDisplay,
  UNSET,
  variantAxes,
  variantOf,
  type Mode,
} from "./view.ts";

export type ReadOptions = {
  catalog: Catalog;
  tokens?: ReadonlyMap<string, Token> | undefined;
};

export type ReadResult = { document: Document; losses: Loss[]; diagnostics: Diagnostic[] };

export type ReadCtx = ReadOptions & {
  api: FigmaApi;
  used: Set<string>;
  counters: Map<string, number>;
  losses: Loss[];
  diagnostics: Diagnostic[];
  /** Weft id → the Figma id of the layer that keeps it; copies of that layer get new ids. */
  keepers: Map<string, string>;
  /** Token groups the document's `gap` values use, most used first, to break ties. */
  gapGroups: string[];
  variablePaths: Map<string, string | undefined>;
  nodes: number;
  truncated: boolean;
};

export type Parent = { kind: string; def: ComponentDef | undefined; path: string; mode: Mode };

export const lose = (ctx: ReadCtx, kind: LossKind, path: string, note: string): void => {
  ctx.losses.push({ kind, path, note });
};

export const STYLE_NOTE =
  "Weft has no prop for this visual edit; it is not carried (see docs/figma-style-overrides-design.md)";

export function component(catalog: Catalog, kind: string): ComponentDef | undefined {
  return Object.hasOwn(catalog.components, kind) ? catalog.components[kind] : undefined;
}

/**
 * Reads a frame built by `buildScreen` (or any layer) back into a Weft document that is not yet
 * canonical or validated: `finishRead` does that with the WebAssembly core, which Figma's main
 * thread cannot run, so the plugin finishes in its UI. `readScreen` does both where the core runs.
 */
export async function readLayers(
  api: FigmaApi,
  layer: FNode,
  options: ReadOptions,
): Promise<ReadResult> {
  const ctx: ReadCtx = {
    ...options,
    api,
    used: new Set(),
    counters: new Map(),
    losses: [],
    diagnostics: [],
    keepers: new Map(),
    gapGroups: [],
    variablePaths: new Map(),
    nodes: 0,
    truncated: false,
  };
  survey(ctx, layer);
  const top: Parent = { kind: "", def: undefined, path: "", mode: "VERTICAL" };
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
  const document = { weft: readVersion(layer) ?? WEFT_VERSION, root };
  return { document, losses: ctx.losses, diagnostics: ctx.diagnostics };
}

/** First pass: reserve every stored id, choose which layer keeps a copied id, count gap groups. */
function survey(ctx: ReadCtx, layer: FNode): void {
  const groups = new Map<string, number>();
  const firstHolder = new Map<string, string>();
  const stack: { layer: FNode; depth: number }[] = [{ layer, depth: 0 }];
  let seen = 0;
  while (stack.length > 0 && seen < MAX_NODES) {
    const { layer: l, depth } = stack.pop() as { layer: FNode; depth: number };
    seen++;
    const source = readSource(l);
    if (source?.id !== undefined) {
      ctx.used.add(source.id);
      if (!firstHolder.has(source.id)) firstHolder.set(source.id, l.id);
      if (l.getPluginData(KEY.origin) === l.id) ctx.keepers.set(source.id, l.id);
      const gap = source.props?.["gap"];
      if (typeof gap === "object" && "token" in gap) {
        const group = gap.token.slice(0, Math.max(0, gap.token.lastIndexOf(".")));
        groups.set(group, (groups.get(group) ?? 0) + 1);
      }
    }
    // Instances of leaf kinds hold only their drawing; their children are not elements.
    if (isContainer(l) && depth < MAX_DEPTH && !(l.type === "INSTANCE" && source !== undefined))
      for (const child of [...l.children].reverse()) stack.push({ layer: child, depth: depth + 1 });
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
  layer: FNode,
  parent: Parent,
  depth: number,
): Promise<Child[]> {
  if (overLimit(ctx, parent.path, depth)) return [];
  const source = readSource(layer);
  if (source !== undefined) return [await fromSource(ctx, layer, source, parent, depth)];
  if (layer.type === "INSTANCE") {
    const main = await layer.getMainComponentAsync();
    const kind = main === null ? undefined : readMark(main, KEY.kind);
    const def = kind === undefined ? undefined : component(ctx.catalog, kind);
    if (kind !== undefined && def !== undefined)
      return [await fromNewInstance(ctx, layer, kind, def, parent)];
  }
  return convertForeign(ctx, layer, parent, depth);
}

/** The children of a container layer: elements, text and named slots. */
export async function convertChildren(
  ctx: ReadCtx,
  layer: { readonly children: readonly FNode[] },
  owner: Parent,
  depth: number,
): Promise<{ children: Child[]; slots: [string, Child[]][] }> {
  const children: Child[] = [];
  const slots: [string, Child[]][] = [];
  for (const child of layer.children) {
    if (readMark(child, KEY.label) !== undefined) continue;
    const slot = readMark(child, KEY.slot);
    if (slot !== undefined && isContainer(child)) {
      const path = `${owner.path}/slot[${slot}]`;
      const inner = await convertChildren(
        ctx,
        child,
        { ...owner, path, mode: child.layoutMode },
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
    if (child.type === "TEXT" && readMark(child, KEY.text) !== undefined) {
      if (child.visible && child.characters.trim() !== "")
        children.push(literal(ctx, owner.path, child.characters));
      else if (!child.visible) lose(ctx, "hidden", owner.path, "a hidden text layer was dropped");
      continue;
    }
    children.push(...(await convertLayer(ctx, child, owner, depth + 1)));
  }
  return { children, slots };
}

function idFor(ctx: ReadCtx, layer: FNode, source: Source, path: string): string | undefined {
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
  layer: FNode,
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
  if ("visible" in layer && layer.visible !== shown) {
    if (layer.visible) delete props["hidden"];
    else props["hidden"] = true;
  }

  if (
    isLeafKind(def) &&
    !(isContainer(layer) && layer.children.some((c) => readSource(c) !== undefined))
  ) {
    readLeafText(ctx, layer, source, node, def as ComponentDef, path);
    if (layer.type === "INSTANCE")
      await readVariants(ctx, layer, source, def as ComponentDef, props, path);
    if (CAPTION_KINDS.has(source.kind))
      readLabel(ctx, findText(layer, "label")?.characters, props, def, path);
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
  else if (layer.getPluginData(KEY.style) !== styleKey(layer, true))
    lose(ctx, "tokens", path, STYLE_NOTE);
  if (CAPTION_KINDS.has(source.kind)) {
    const label = layer.children.find((c) => readMark(c, KEY.label) !== undefined);
    readLabel(ctx, label?.type === "TEXT" ? label.characters : undefined, props, def, path);
  }
  const owner: Parent = { kind: source.kind, def, path, mode: layer.layoutMode };
  const { children, slots } = await convertChildren(ctx, layer, owner, depth);
  node.children = children;
  node.slots = Object.fromEntries(slots);
  return node;
}

/** A text edit: the leaf's text layer against what its source shows. */
function readLeafText(
  ctx: ReadCtx,
  layer: FNode,
  source: Source,
  node: Node,
  def: ComponentDef,
  path: string,
): void {
  node.children = source.children;
  if (def.content !== "text") return;
  const shown = findText(layer, "text")?.characters;
  if (shown === undefined || shown === textDisplay(source)) return;
  const props = node.props as Record<string, Value>;
  const value = typed(ctx, shown, path);
  if (props["text"] !== undefined || typeof value !== "string") {
    if (value === "") delete props["text"];
    else props["text"] = value;
    node.children = [];
  } else {
    node.children = value === "" ? [] : [value];
  }
}

/** Text a designer typed, as a value: a whole `{$…}` or `{token.…}` is a reference, else text. */
export function typed(ctx: ReadCtx, shown: string, path: string): Value {
  const value = readText(shown) ?? shown;
  return typeof value === "string" ? literal(ctx, path, value) : value;
}

function readLabel(
  ctx: ReadCtx,
  shown: string | undefined,
  props: Record<string, Value>,
  def: ComponentDef | undefined,
  path: string,
): void {
  if (shown === undefined || shown === labelDisplay(props)) return;
  const value = typed(ctx, shown, path);
  if (value === "" && def?.requiresLabel !== true) delete props["label"];
  else props["label"] = value;
}

async function readVariants(
  ctx: ReadCtx,
  layer: FLayout & {
    readonly componentProperties: Readonly<Record<string, { value: string | boolean }>>;
    getMainComponentAsync(): Promise<FLayout | null>;
  },
  source: Source,
  def: ComponentDef,
  props: Record<string, Value>,
  path: string,
): Promise<void> {
  const axes = variantAxes(def);
  const expected = variantOf(source.props, axes);
  for (const axis of axes) {
    const now = layer.componentProperties[axis.name]?.value;
    if (typeof now !== "string" || now === expected[axis.name]) continue;
    if (now === UNSET) delete props[axis.name];
    else if (axis.values.includes(now)) props[axis.name] = now;
    else lose(ctx, "props", path, `variant ${axis.name}=${now} is not a value Weft knows`);
  }
  const main = await layer.getMainComponentAsync();
  if (main !== null && styleKey(main, true) !== styleKey(layer, true))
    lose(ctx, "tokens", path, STYLE_NOTE);
}

async function tokenOf(ctx: ReadCtx, alias: FAlias | undefined): Promise<string | undefined> {
  if (alias === undefined) return undefined;
  if (ctx.variablePaths.has(alias.id)) return ctx.variablePaths.get(alias.id);
  const variable = await ctx.api.variables.getVariableByIdAsync(alias.id);
  const path =
    variable === null ? undefined : (readMark(variable, KEY.token) ?? tokenPathOf(variable.name));
  ctx.variablePaths.set(alias.id, path);
  return path;
}

async function readLayout(
  ctx: ReadCtx,
  layer: FLayout,
  view: ReturnType<typeof layoutView>,
  source: Source,
  props: Record<string, Value>,
  path: string,
): Promise<void> {
  const grid = source.kind === "grid";
  if (grid) {
    if (layer.layoutMode !== "GRID")
      lose(ctx, "layout", path, "a grid is no longer a grid layout; its layout is kept");
    else if (layer.gridColumnCount !== view.columns) props["columns"] = layer.gridColumnCount;
  } else if (layer.layoutMode !== view.mode) {
    if (layer.layoutMode === "HORIZONTAL") props["direction"] = "row";
    else if (layer.layoutMode === "VERTICAL") props["direction"] = "column";
    else lose(ctx, "layout", path, `layout ${layer.layoutMode} has no Weft stack direction`);
  }
  if (!grid && layer.counterAxisAlignItems !== view.align) {
    const align = { MIN: "start", CENTER: "center", MAX: "end" }[
      layer.counterAxisAlignItems as string
    ];
    if (align === undefined)
      lose(ctx, "layout", path, `alignment ${layer.counterAxisAlignItems} has no Weft value`);
    else props["align"] = align;
  }
  if (!grid && (layer.layoutWrap === "WRAP") !== view.wrap) {
    if (layer.layoutWrap === "WRAP") props["wrap"] = true;
    else delete props["wrap"];
  }
  await readGap(ctx, layer, view, source, props, path, grid);
  if (layer.getPluginData(KEY.style) !== styleKey(layer, false))
    lose(ctx, "tokens", path, STYLE_NOTE);
}

async function readGap(
  ctx: ReadCtx,
  layer: FLayout,
  view: ReturnType<typeof layoutView>,
  source: Source,
  props: Record<string, Value>,
  path: string,
  grid: boolean,
): Promise<void> {
  const field = grid ? "gridRowGap" : "itemSpacing";
  const px = grid ? layer.gridRowGap : layer.itemSpacing;
  let bound = await tokenOf(ctx, layer.boundVariables?.[field]);
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
  layer: FNode & {
    readonly componentProperties: Readonly<Record<string, { value: string | boolean }>>;
  },
  kind: string,
  def: ComponentDef,
  parent: Parent,
): Promise<Node> {
  const props: Record<string, Value> = {};
  for (const axis of variantAxes(def)) {
    const v = layer.componentProperties[axis.name]?.value;
    const declared = axis.name === "state" ? undefined : def.props?.[axis.name]?.default;
    if (typeof v === "string" && v !== UNSET && axis.values.includes(v) && v !== declared)
      props[axis.name] = v;
  }
  if (!layer.visible) props["hidden"] = true;
  const shown = def.content === "text" ? (findText(layer, "text")?.characters ?? "") : "";
  const label = CAPTION_KINDS.has(kind) ? (findText(layer, "label")?.characters ?? "") : "";
  const id = freshId(ctx, kind, shown || label);
  const path = `${parent.path}/${kind}#${id}`;
  const node: Node = { kind, id, props };
  if (label !== "") props["label"] = typed(ctx, label, path);
  if (shown !== "") {
    const value = typed(ctx, shown, path);
    if (typeof value === "string") node.children = [value];
    else props["text"] = value;
  }
  if (def.requiresLabel === true && props["label"] === undefined) {
    props["label"] = "";
    lose(ctx, "names", path, 'the layer has no label; "" stands in');
  }
  fillRequired(ctx, props, def, parent.def, false, shown || label, path);
  return node;
}
