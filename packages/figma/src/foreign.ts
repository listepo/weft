// Layers that did not come from Weft carry no semantics, so they convert with losses, reported in
// the loss table of `@weft/from-aria`: text becomes text, a frame named after a kind becomes that
// kind, an image fill becomes an image, any other frame or group a stack, and shapes are dropped.
import type { Child, Node, Value } from "@weft/core";
import { ID } from "@weft/core";
import { fillRequired, freshId, literal } from "@weft/from-aria";
import type { FNode, FPaint } from "./api.ts";
import { isContainer } from "./api.ts";
import { findText } from "./build.ts";
import { component, convertChildren, lose, type Parent, type ReadCtx } from "./read.ts";
import { matchToken, tokenPathOf } from "./tokens.ts";
import { CAPTION_KINDS, isLeafKind } from "./view.ts";
import { KEY, readMark } from "./keys.ts";

const NAMED = /^([a-z][a-z0-9]*(?:-[a-z0-9]+)*)(?:#(\S+))?$/;

const hasImage = (fills: readonly FPaint[] | symbol | undefined): boolean =>
  fills !== undefined && typeof fills !== "symbol" && fills.some((p) => p.type === "IMAGE");

const painted = (fills: readonly FPaint[] | symbol | undefined): boolean =>
  fills !== undefined && (typeof fills === "symbol" || fills.length > 0);

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
  layer: FNode,
  parent: Parent,
  depth: number,
): Promise<Child[]> {
  if (!layer.visible) {
    lose(ctx, "hidden", parent.path, `the hidden layer "${layer.name}" was dropped`);
    return [];
  }
  if (layer.type === "TEXT") {
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
  if (hasImage("fills" in layer ? layer.fills : undefined)) {
    const { id, path } = generated(ctx, "image", layer.name, parent);
    const props: Record<string, Value> = { label: literal(ctx, path, layer.name) };
    const def = component(ctx.catalog, "image");
    if (def !== undefined) fillRequired(ctx, props, def, parent.def, false, "", path);
    lose(ctx, "values", path, "the image fill has no URL; src is empty");
    return [{ kind: "image", id, props }];
  }
  const children = "children" in layer ? layer.children : undefined;
  if (children === undefined) {
    lose(
      ctx,
      "kinds",
      parent.path,
      `the ${layer.type.toLowerCase()} layer "${layer.name}" has no Weft kind and was dropped`,
    );
    return [];
  }
  return [await stackOf(ctx, layer, children, parent, depth)];
}

/** A frame or group without semantics: a stack (or a grid) with its layout carried over. */
async function stackOf(
  ctx: ReadCtx,
  layer: FNode,
  children: readonly FNode[],
  parent: Parent,
  depth: number,
): Promise<Node> {
  const container = isContainer(layer) ? layer : undefined;
  const mode = container?.layoutMode ?? "NONE";
  const kind = mode === "GRID" ? "grid" : "stack";
  const { id, path } = generated(ctx, kind, layer.name, parent);
  const props: Record<string, Value> = {};
  if (mode === "HORIZONTAL") props["direction"] = "row";
  if (mode === "GRID") props["columns"] = Math.max(1, container?.gridColumnCount ?? 1);
  let ordered = children;
  if (mode === "NONE") {
    // Free positions have no Weft form; reading order is top to bottom, then left to right.
    ordered = [...children].sort((a, b) => a.y - b.y || a.x - b.x);
    lose(
      ctx,
      "layout",
      path,
      "free positions are not carried; the layers are stacked in reading order",
    );
  }
  if (container !== undefined && mode !== "NONE") {
    const field = mode === "GRID" ? "gridRowGap" : "itemSpacing";
    const px = mode === "GRID" ? container.gridRowGap : container.itemSpacing;
    const alias = container.boundVariables?.[field];
    const variable =
      alias === undefined ? null : await ctx.api.variables.getVariableByIdAsync(alias.id);
    const token =
      variable !== null
        ? (readMark(variable, KEY.token) ?? tokenPathOf(variable.name))
        : ctx.tokens === undefined
          ? undefined
          : matchToken(px, ctx.tokens, "dimension", undefined, ctx.gapGroups);
    if (token !== undefined) props["gap"] = { token };
    else if (px !== 0) lose(ctx, "tokens", path, `gap ${px}px matches no token and was dropped`);
  }
  if (painted("fills" in layer ? layer.fills : undefined) || (container?.strokes.length ?? 0) > 0)
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
    mode: mode === "HORIZONTAL" ? "HORIZONTAL" : "VERTICAL",
  };
  const inner = await convertChildren(ctx, { children: ordered }, owner, depth);
  return { kind, id, props, children: inner.children, slots: Object.fromEntries(inner.slots) };
}

/**
 * A frame whose name is a catalog kind (`button`, `list#todos`): a detached library component,
 * or a frame the designer named on purpose. Its kind, text and children are read; other props
 * are not, so required ones get stand-ins.
 */
async function namedKind(
  ctx: ReadCtx,
  layer: FNode,
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
    const text = findText(layer, "text") ?? layer.children.find((c) => c.type === "TEXT");
    shown = text?.type === "TEXT" ? text.characters.trim() : "";
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
    const owner: Parent = { kind, def, path, mode: layer.layoutMode };
    const inner = await convertChildren(
      ctx,
      // The library draws a hint into empty containers and a caption into labelled ones.
      {
        children: layer.children.filter(
          (c) => !(c.type === "TEXT" && (c.name === "hint" || c.name === "label")),
        ),
      },
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
