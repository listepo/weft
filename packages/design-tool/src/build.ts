// Weft to a design tool: every element becomes a library instance (kinds without element content)
// or a frame with a layout (the rest), and remembers its Weft source in plugin data, so that
// reading the layers back returns the document as written. What to draw is decided here; how is
// up to the tool's `BuildHost`.
import type { Token } from "@weft/catalog";
import type { Catalog, Child, Document, Node } from "@weft/core";
import { KEY, sourceOf, writeJson, type PluginData, type Shown } from "./keys.ts";
import {
  textDrawing,
  WHITE_PAINT,
  type KindEntry,
  type Paint,
  type TextDrawing,
} from "./library.ts";
import {
  CAPTION_KINDS,
  isLeafKind,
  labelDisplay,
  LAYOUT_KINDS,
  layoutView,
  textDisplay,
  variantName,
  variantOf,
  type Display,
  type LayoutView,
  type Mode,
} from "./view.ts";

/** What a layer the build made can be marked with. */
export type Marks = PluginData & { readonly id: string; name: string; visible: boolean };

/** The drawing operations a tool provides for the build; `L` is its layer, `C` its component. */
export interface BuildHost<L, C> {
  /** Runs before anything is drawn (fonts, the position of the new screen). */
  prepare(): Promise<void>;
  /** A copy of a library component; switched to `values` when the exact variant is missing. */
  instance(main: C, values: Readonly<Record<string, string>> | undefined): L;
  /** A frame with this layout and fill. */
  frame(view: LayoutView, fill: Paint | undefined): L;
  text(drawing: TextDrawing): L;
  /** Sets the text layer named `name` inside an instance, if it has one. */
  setText(layer: L, name: string, value: string): Promise<void>;
  append(parent: L, child: L): void;
  marks(layer: L): Marks;
  /** The same fingerprint `Layer.style` reads back. */
  style(layer: L, withSpacing: boolean): string;
  /** Places the finished root on the page; `screen` when the root is a `screen`. */
  place(root: L, screen: boolean): void;
}

export type BuildOptions<C> = {
  catalog: Catalog;
  kinds: ReadonlyMap<string, KindEntry<C>>;
  tokens?: ReadonlyMap<string, Token> | undefined;
  /** The document's `text` and `label` values in attribute form, from `displayTexts`. */
  display: Display;
};

type Ctx<L, C> = BuildOptions<C> & { host: BuildHost<L, C> };

/**
 * Builds the document on the current page and returns its root layer. The document must be
 * canonical, as `parse` and `canonicalize` return it: canonicalizing here would need the
 * WebAssembly core, which a plugin sandbox cannot run.
 */
export async function buildScreen<L, C>(
  host: BuildHost<L, C>,
  document: Document,
  options: BuildOptions<C>,
): Promise<L> {
  await host.prepare();
  const ctx: Ctx<L, C> = { ...options, host };
  const root = await buildNode(ctx, document.root, "column");
  writeJson(host.marks(root), KEY.document, { weft: document.weft });
  host.place(root, document.root.kind === "screen");
  return root;
}

function mark(marks: Marks, node: Node, leaf: boolean, shown: Shown): void {
  marks.name = node.id === undefined ? node.kind : `${node.kind}#${node.id}`;
  writeJson(marks, KEY.source, sourceOf(node, leaf, shown));
  marks.setPluginData(KEY.origin, marks.id);
  if (node.props?.["hidden"] === true) marks.visible = false;
}

async function buildNode<L, C>(ctx: Ctx<L, C>, node: Node, parentMode: Mode): Promise<L> {
  const { host } = ctx;
  const def = Object.hasOwn(ctx.catalog.components, node.kind)
    ? ctx.catalog.components[node.kind]
    : undefined;
  const entry = ctx.kinds.get(node.kind);
  const textOnly = (node.children ?? []).every((c) => typeof c === "string");
  if (def !== undefined && entry !== undefined && isLeafKind(def) && textOnly && !node.slots) {
    const values = variantOf(node.props, entry.axes);
    const main = entry.variants.get(entry.axes.length === 0 ? "" : variantName(values, entry.axes));
    const instance = host.instance(main ?? entry.fallback, main === undefined ? values : undefined);
    const text = textDisplay(node, ctx.display);
    const shown: Shown = { text };
    await host.setText(instance, "text", text);
    if (CAPTION_KINDS.has(node.kind)) {
      shown.label = labelDisplay(node.props, ctx.display);
      await host.setText(instance, "label", shown.label);
    }
    mark(host.marks(instance), node, true, shown);
    return instance;
  }

  const view = layoutView(node.kind, node.props, ctx.tokens, parentMode);
  const frame = host.frame(view, node.kind === "screen" ? WHITE_PAINT : undefined);
  const shown: Shown = {};
  if (CAPTION_KINDS.has(node.kind)) {
    shown.label = labelDisplay(node.props, ctx.display);
    const label = host.text(textDrawing("label", shown.label, "bold", 14));
    host.marks(label).setPluginData(KEY.label, "1");
    host.append(frame, label);
  }
  await appendChildren(ctx, frame, node.children ?? [], view.mode);
  for (const [name, list] of Object.entries(node.slots ?? {})) {
    const slotView = layoutView("slot", undefined, ctx.tokens, view.mode);
    // Slot frames are not elements: their spacing only separates what they hold.
    const slot = host.frame({ ...slotView, gap: { px: 8 } }, undefined);
    const marks = host.marks(slot);
    marks.name = `slot:${name}`;
    marks.setPluginData(KEY.slot, name);
    await appendChildren(ctx, slot, list, slotView.mode);
    host.append(frame, slot);
  }
  const marks = host.marks(frame);
  mark(marks, node, false, shown);
  marks.setPluginData(KEY.style, host.style(frame, !LAYOUT_KINDS.has(node.kind)));
  return frame;
}

async function appendChildren<L, C>(
  ctx: Ctx<L, C>,
  frame: L,
  children: readonly Child[],
  mode: Mode,
): Promise<void> {
  for (const child of children) {
    if (typeof child === "string") {
      const text = ctx.host.text(textDrawing("text", child));
      ctx.host.marks(text).setPluginData(KEY.text, "1");
      ctx.host.append(frame, text);
    } else {
      ctx.host.append(frame, await buildNode(ctx, child, mode));
    }
  }
}
