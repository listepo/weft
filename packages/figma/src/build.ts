// Weft to Figma: every element becomes a library instance (kinds without element content) or an
// auto-layout frame (the rest), and remembers its Weft source in plugin data, so that reading the
// layers back returns the document as written.
import type { Token } from "@weft/catalog";
import type { Catalog, Child, Document, Node } from "@weft/core";
import type { FFrame, FigmaApi, FInstance, FLayout, FNode, FText } from "./api.ts";
import { isContainer } from "./api.ts";
import { KEY, sourceOf, writeJson, type Shown } from "./keys.ts";
import {
  autoLayout,
  bindNumber,
  FONT,
  loadFonts,
  makeText,
  paint,
  type Library,
} from "./library.ts";
import {
  CAPTION_KINDS,
  isLeafKind,
  labelDisplay,
  LAYOUT_KINDS,
  layoutView,
  styleKey,
  textDisplay,
  variantName,
  variantOf,
  type Display,
  type Mode,
} from "./view.ts";

export type BuildOptions = {
  catalog: Catalog;
  library: Library;
  tokens?: ReadonlyMap<string, Token> | undefined;
  /** The document's `text` and `label` values in attribute form, from `displayTexts`. */
  display: Display;
};

type Ctx = BuildOptions & { api: FigmaApi };

const SCREEN_WIDTH = 480;
const SCREEN_GAP = 120;

/**
 * Builds the document on the current page, to the right of what is there, and returns its frame.
 * The document must be canonical, as `parse` and `canonicalize` return it: canonicalizing here
 * would need the WebAssembly core, which Figma's main thread cannot run.
 */
export async function buildScreen(
  api: FigmaApi,
  document: Document,
  options: BuildOptions,
): Promise<FFrame | FInstance> {
  await loadFonts(api);
  const ctx: Ctx = { ...options, api };
  const doc = document;
  const right = api.currentPage.children.reduce(
    (x, n) => Math.max(x, n.x + n.width + SCREEN_GAP),
    0,
  );
  const root = await buildNode(ctx, doc.root, "VERTICAL");
  writeJson(root, KEY.document, { weft: doc.weft });
  api.currentPage.appendChild(root);
  root.x = right;
  root.y = 0;
  if (root.type === "FRAME" && doc.root.kind === "screen") {
    root.resize(SCREEN_WIDTH, Math.max(root.height, 1));
    root.counterAxisSizingMode = "FIXED";
  }
  return root;
}

function mark(layer: FNode, node: Node, leaf: boolean, shown: Shown): void {
  layer.name = node.id === undefined ? node.kind : `${node.kind}#${node.id}`;
  writeJson(layer, KEY.source, sourceOf(node, leaf, shown));
  layer.setPluginData(KEY.origin, layer.id);
  if (node.props?.["hidden"] === true) layer.visible = false;
}

async function buildNode(ctx: Ctx, node: Node, parentMode: Mode): Promise<FFrame | FInstance> {
  const def = Object.hasOwn(ctx.catalog.components, node.kind)
    ? ctx.catalog.components[node.kind]
    : undefined;
  const entry = ctx.library.kinds.get(node.kind);
  const textOnly = (node.children ?? []).every((c) => typeof c === "string");
  if (def !== undefined && entry !== undefined && isLeafKind(def) && textOnly && !node.slots) {
    const values = variantOf(node.props, entry.axes);
    const main = entry.variants.get(entry.axes.length === 0 ? "" : variantName(values, entry.axes));
    const instance = (main ?? entry.fallback).createInstance();
    if (main === undefined) instance.setProperties(values);
    const text = textDisplay(node, ctx.display);
    const shown: Shown = { text };
    await setLayerText(ctx.api, instance, "text", text);
    if (CAPTION_KINDS.has(node.kind)) {
      shown.label = labelDisplay(node.props, ctx.display);
      await setLayerText(ctx.api, instance, "label", shown.label);
    }
    mark(instance, node, true, shown);
    return instance;
  }

  const frame = ctx.api.createFrame();
  const view = layoutView(node.kind, node.props, ctx.tokens, parentMode);
  autoLayout(
    frame,
    view.mode === "HORIZONTAL" ? "HORIZONTAL" : "VERTICAL",
    view.gap.px,
    view.padding,
  );
  frame.fills =
    node.kind === "screen"
      ? [paint(ctx.api, ctx.library, "color.white", { r: 1, g: 1, b: 1 })]
      : [];
  if (view.mode === "GRID") {
    frame.layoutMode = "GRID";
    frame.gridColumnCount = view.columns;
    bindNumber(frame, ["gridRowGap", "gridColumnGap"], ctx.library, view.gap.token, view.gap.px);
  } else {
    if (view.wrap) frame.layoutWrap = "WRAP";
    frame.counterAxisAlignItems = view.align;
    bindNumber(frame, ["itemSpacing"], ctx.library, view.gap.token, view.gap.px);
  }
  const shown: Shown = {};
  if (CAPTION_KINDS.has(node.kind)) {
    shown.label = labelDisplay(node.props, ctx.display);
    const label = makeText(ctx.api, "label", shown.label, FONT.bold, 14);
    label.setPluginData(KEY.label, "1");
    frame.appendChild(label);
  }
  await appendChildren(ctx, frame, node.children ?? [], view.mode);
  for (const [name, list] of Object.entries(node.slots ?? {})) {
    const slot = ctx.api.createFrame();
    slot.name = `slot:${name}`;
    slot.setPluginData(KEY.slot, name);
    const slotView = layoutView("slot", undefined, ctx.tokens, view.mode);
    autoLayout(slot, slotView.mode === "HORIZONTAL" ? "HORIZONTAL" : "VERTICAL", 8, 0);
    slot.fills = [];
    await appendChildren(ctx, slot, list, slotView.mode);
    frame.appendChild(slot);
  }
  mark(frame, node, false, shown);
  frame.setPluginData(KEY.style, styleKey(frame, !LAYOUT_KINDS.has(node.kind)));
  return frame;
}

async function appendChildren(
  ctx: Ctx,
  frame: FLayout,
  children: readonly Child[],
  mode: Mode,
): Promise<void> {
  for (const child of children) {
    if (typeof child === "string") {
      const text = makeText(ctx.api, "text", child);
      text.setPluginData(KEY.text, "1");
      frame.appendChild(text);
    } else {
      frame.appendChild(await buildNode(ctx, child, mode));
    }
  }
}

/** The first text layer with this name, searched breadth-first below a layer. */
export function findText(layer: FNode, name: string, depth = 0): FText | undefined {
  if (!isContainer(layer) || depth > 8) return undefined;
  const direct = layer.children.find((c): c is FText => c.type === "TEXT" && c.name === name);
  if (direct !== undefined) return direct;
  for (const child of layer.children) {
    const found = findText(child, name, depth + 1);
    if (found !== undefined) return found;
  }
  return undefined;
}

async function setLayerText(
  api: FigmaApi,
  layer: FNode,
  name: string,
  value: string,
): Promise<void> {
  const text = findText(layer, name);
  if (text === undefined) return;
  if (typeof text.fontName !== "symbol") await api.loadFontAsync(text.fontName);
  text.characters = value;
}
