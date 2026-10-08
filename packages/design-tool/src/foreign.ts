// Layers that did not come from Weft carry no semantics, so they convert with losses, reported in
// the loss table of `@weft/from-aria`: text becomes text, a frame named after a kind becomes that
// kind, an image fill becomes an image, any other frame or group a stack, and shapes are dropped.
import type { Child, Node, Value } from "@weft/core";
import { ID } from "@weft/core";
import { fillRequired, freshId, literal } from "@weft/from-aria";
import { findText, isContainer, type Layer, type LayerLayout } from "./layer.ts";
import { component, convertChildren, lose, type Parent, type ReadCtx } from "./read.ts";
import { matchToken } from "./tokens.ts";
import { CAPTION_KINDS, isLeafKind } from "./view.ts";

const NAMED = /^([a-z][a-z0-9]*(?:-[a-z0-9]+)*)(?:#(\S+))?$/;

// A row's `center` is the SwiftUI default and a column's `start` is the host default (SPEC §5.1),
// so an importer leaves those unset. Anything else a stack can say is written.
const ROW_ALIGN: ReadonlySet<string> = new Set(["start", "end", "stretch"]);
const COLUMN_ALIGN: ReadonlySet<string> = new Set(["center", "end", "stretch"]);

function stackAlign(mode: "row" | "column", align: LayerLayout["align"]): string | undefined {
  if (align === undefined) return undefined;
  return (mode === "row" ? ROW_ALIGN : COLUMN_ALIGN).has(align) ? align : undefined;
}

/**
 * In-flow children of a grid, in cell order (row, then column, then the original index).
 * Absolutely positioned children stay where they are. Undefined when no child carries an
 * anchor, or when that order is already the z-order: the caller then keeps the array it has.
 */
function gridReadingOrder(children: readonly Layer[]): readonly Layer[] | undefined {
  if (children.length < 2) return undefined;
  const absolute = (layer: Layer): boolean => layer.layoutPositioning === "ABSOLUTE";
  if (
    !children.some(
      (layer) =>
        !absolute(layer) &&
        (layer.gridRowAnchorIndex !== undefined || layer.gridColumnAnchorIndex !== undefined),
    )
  )
    return undefined;
  const inFlow = children
    .map((layer, index) => ({ layer, index }))
    .filter(({ layer }) => !absolute(layer))
    .sort((a, b) => {
      const rows = (a.layer.gridRowAnchorIndex ?? 0) - (b.layer.gridRowAnchorIndex ?? 0);
      if (rows !== 0) return rows;
      const columns = (a.layer.gridColumnAnchorIndex ?? 0) - (b.layer.gridColumnAnchorIndex ?? 0);
      return columns !== 0 ? columns : a.index - b.index;
    });
  let cursor = 0;
  const next = children.map((layer) => {
    if (absolute(layer)) return layer;
    const placed = inFlow[cursor];
    cursor += 1;
    return placed === undefined ? layer : placed.layer;
  });
  return next.every((layer, index) => layer === children[index]) ? undefined : next;
}

function generated(
  ctx: ReadCtx,
  kind: string,
  name: string,
  parent: Parent,
): { id: string; path: string } {
  const id = freshId(ctx, kind, name);
  const path = `${parent.path}/${kind}#${id}`;
  lose(ctx, "ids", path, "the layer did not come from Weft; its id was generated");
  return { id, path };
}

export async function convertForeign(
  ctx: ReadCtx,
  layer: Layer,
  parent: Parent,
  depth: number,
): Promise<Child[]> {
  if (!layer.visible) {
    lose(ctx, "hidden", parent.path, `the hidden layer "${layer.name}" was dropped`);
    return [];
  }
  if (layer.kind === "text") {
    const text = layer.characters.trim();
    if (text === "") return [];
    const content = parent.def?.content;
    // Text goes straight into a parent that takes text (or an extension, which takes anything).
    if (
      content === "text" ||
      content === "mixed" ||
      (parent.def === undefined && parent.kind !== "")
    )
      return [literal(ctx, parent.path, text)];
    const { id, path } = generated(ctx, "text", text, parent);
    return [{ kind: "text", id, children: [literal(ctx, path, text)] }];
  }
  const named = await namedKind(ctx, layer, parent, depth);
  if (named !== undefined) return [named];
  if (layer.image) {
    const { id, path } = generated(ctx, "image", layer.name, parent);
    const props: Record<string, Value> = { label: literal(ctx, path, layer.name) };
    const def = component(ctx.catalog, "image");
    if (def !== undefined) fillRequired(ctx, props, def, parent.def, false, "", path);
    lose(ctx, "values", path, "the image fill has no URL; src is empty");
    return [{ kind: "image", id, props }];
  }
  const { children } = layer;
  if (children === undefined) {
    lose(
      ctx,
      "kinds",
      parent.path,
      `the ${layer.type} layer "${layer.name}" has no Weft kind and was dropped`,
    );
    return [];
  }
  return [await stackOf(ctx, layer, children, parent, depth)];
}

/** A frame or group without semantics: a stack (or a grid) with its layout carried over. */
async function stackOf(
  ctx: ReadCtx,
  layer: Layer,
  children: readonly Layer[],
  parent: Parent,
  depth: number,
): Promise<Node> {
  const container = isContainer(layer);
  // A layout Weft has no form for is read as free positions.
  const mode = container ? layer.layout.mode : "none";
  const kind = mode === "grid" ? "grid" : "stack";
  const { id, path } = generated(ctx, kind, layer.name, parent);
  const props: Record<string, Value> = {};
  if (mode === "row") props["direction"] = "row";
  if (mode === "grid") props["columns"] = Math.max(1, layer.layout.columns);
  // `align` and `wrap` only, and only on a stack: a grid has neither prop. Padding, fills and
  // fonts stay losses; the painted-loss below is unchanged.
  if (kind === "stack" && (mode === "row" || mode === "column")) {
    if (layer.layout.wrap) props["wrap"] = true;
    const align = stackAlign(mode, layer.layout.align);
    if (align !== undefined) props["align"] = align;
  }
  let ordered = children;
  if (mode === "none") {
    // Free positions have no Weft form; reading order is top to bottom, then left to right.
    ordered = [...children].sort((a, b) => a.y - b.y || a.x - b.x);
    lose(
      ctx,
      "layout",
      path,
      "free positions are not carried; the layers are stacked in reading order",
    );
  } else if (mode === "grid") {
    const placed = gridReadingOrder(children);
    if (placed !== undefined) {
      ordered = placed;
      lose(
        ctx,
        "layout",
        path,
        "grid children are ordered by their row and column anchors; their z-order was not kept",
      );
    }
  }
  if (mode !== "none") {
    const px = layer.gap(mode === "grid");
    const token =
      (await layer.gapToken(mode === "grid")) ??
      (ctx.tokens === undefined
        ? undefined
        : matchToken(px, ctx.tokens, "dimension", undefined, ctx.gapGroups));
    if (token !== undefined) props["gap"] = { token };
    else if (px !== 0) lose(ctx, "tokens", path, `gap ${px}px matches no token and was dropped`);
  }
  if (layer.painted)
    lose(
      ctx,
      "tokens",
      path,
      "fills and strokes of a layer that did not come from Weft are not carried",
    );
  const def = component(ctx.catalog, kind);
  const owner: Parent = {
    kind,
    def,
    path,
    mode: mode === "row" ? "row" : "column",
  };
  const inner = await convertChildren(ctx, ordered, owner, depth);
  return { kind, id, props, children: inner.children, slots: Object.fromEntries(inner.slots) };
}

/**
 * A frame whose name is a catalog kind (`button`, `list#todos`): a detached library component,
 * or a frame the designer named on purpose. Its kind, text and children are read; other props
 * are not, so required ones get stand-ins.
 */
async function namedKind(
  ctx: ReadCtx,
  layer: Layer,
  parent: Parent,
  depth: number,
): Promise<Node | undefined> {
  const match = NAMED.exec(layer.name.trim());
  const kind = match?.[1];
  const def = kind === undefined ? undefined : component(ctx.catalog, kind);
  if (kind === undefined || def === undefined || kind === "screen" || !isContainer(layer))
    return undefined;
  // The shown text names a generated id, so it is read before the id is chosen.
  const label = CAPTION_KINDS.has(kind) ? (findText(layer, "label")?.characters ?? "") : "";
  let shown = "";
  if (isLeafKind(def) && def.content === "text") {
    const text = findText(layer, "text") ?? layer.children?.find((c) => c.kind === "text");
    shown = text?.kind === "text" ? text.characters.trim() : "";
  }
  const name = label || shown;
  const wanted = match?.[2];
  let id: string;
  let path: string;
  if (wanted !== undefined && ID.test(wanted) && !ctx.used.has(wanted)) {
    ctx.used.add(wanted);
    id = wanted;
    path = `${parent.path}/${kind}#${id}`;
  } else {
    ({ id, path } = generated(ctx, kind, wanted ?? name, parent));
  }
  const props: Record<string, Value> = {};
  const node: Node = { kind, id, props };
  lose(
    ctx,
    "props",
    path,
    `the layer is named after ${kind} but has no Weft source; only its text and content were read`,
  );
  if (label !== "") props["label"] = literal(ctx, path, label);
  if (isLeafKind(def)) {
    if (shown !== "") node.children = [literal(ctx, path, shown)];
  } else {
    const owner: Parent = { kind, def, path, mode: layer.layout.mode };
    const inner = await convertChildren(
      ctx,
      // The library draws a hint into empty containers and a caption into labelled ones.
      (layer.children ?? []).filter(
        (c) => !(c.kind === "text" && (c.name === "hint" || c.name === "label")),
      ),
      owner,
      depth,
    );
    node.children = inner.children;
    node.slots = Object.fromEntries(inner.slots);
  }
  if (def.requiresLabel === true && props["label"] === undefined) {
    props["label"] = "";
    lose(ctx, "names", path, 'the layer has no label; "" stands in');
  }
  fillRequired(ctx, props, def, parent.def, false, name, path);
  return node;
}
