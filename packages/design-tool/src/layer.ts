// A layer of a design file as the read-back sees it, whatever tool it lives in. Each tool package
// wraps its own nodes in this interface (Figma's frames and instances, Penpot's boards and
// component copies), so the mapping back to Weft is written once. Everything a layer reports came
// from a file the plugin does not control, so it is input to check, not a fact to trust.
import type { PluginData } from "./keys.ts";
import type { Align, Justify, Mode } from "./view.ts";

/**
 * - `frame`: a container the reader descends into (Figma frames and components, Penpot boards).
 * - `instance`: a copy of a library component.
 * - `text`: a text layer.
 * - `other`: groups, shapes, vectors and anything else; a group still has children.
 */
export type LayerKind = "frame" | "instance" | "text" | "other";

/** How a layer sizes itself on one axis as its parent's layout sees it. */
export type Sizing = "fixed" | "hug" | "fill";

export type LayerLayout = {
  /** `none`: free positions, or a layout Weft has no form for (`label` names it). */
  mode: Mode | "none";
  /** The tool's own name for the layout, for loss notes. */
  label: string;
  /**
   * Undefined when the tool's alignment has no Weft value (`alignLabel` names it).
   * `stretch` is a Weft value; a tool that draws it as `start` reports `start` instead.
   */
  align: Align | "stretch" | undefined;
  alignLabel: string;
  wrap: boolean;
  columns: number;
  /**
   * Main-axis alignment of a stack; undefined when the tool's value has no Weft form
   * (`justifyLabel` names it).
   */
  justify: Justify | undefined;
  justifyLabel: string;
  /** The common padding when every side matches; undefined when the sides differ. */
  padding: number | undefined;
  /** The max width the tool reports, if any. */
  maxWidth: number | undefined;
  /** The min width the tool reports, if any (Weft has no form for it). */
  minWidth: number | undefined;
  /** Whether every grid track is an equal `flex` track. */
  flexTracks: boolean;
};

export interface Layer extends PluginData {
  readonly id: string;
  readonly kind: LayerKind;
  /** The tool's layer type in lower case (`vector`, `ellipse`), for loss notes. */
  readonly type: string;
  readonly name: string;
  readonly visible: boolean;
  readonly x: number;
  readonly y: number;
  /** In the order the layout shows them; undefined for layers that cannot hold any. */
  readonly children: readonly Layer[] | undefined;
  /** A text layer's text; `""` for every other layer. */
  readonly characters: string;
  readonly layout: LayerLayout;
  /**
   * The 0-based cell this layer occupies when it is a child of a grid. Absent when the tool
   * does not report anchors; the reader then keeps the children in z-order.
   */
  readonly gridRowAnchorIndex?: number | undefined;
  readonly gridColumnAnchorIndex?: number | undefined;
  /** `ABSOLUTE` is out of flow and keeps its place when grid children are ordered. */
  readonly layoutPositioning?: "AUTO" | "ABSOLUTE" | undefined;
  /** Whether a fill is an image. */
  readonly image: boolean;
  /** Whether the layer has fills or strokes. */
  readonly painted: boolean;
  /** An instance's variant property values; empty for every other layer. */
  readonly variant: Readonly<Record<string, string>>;
  /** The spacing between children: the row gap of a grid, else the gap along the main axis. */
  gap(grid: boolean): number;
  /** The token path bound to that spacing, if the tool says one is. */
  gapToken(grid: boolean): Promise<string | undefined>;
  /** How the layer sizes itself on each axis, as its parent's layout sees it. */
  readonly sizing: { readonly horizontal: Sizing; readonly vertical: Sizing };
  /** The token path bound to the common padding, if the tool says one is. */
  paddingToken(): Promise<string | undefined>;
  /** The token path bound to the max width, if the tool says one is. */
  maxWidthToken(): Promise<string | undefined>;
  /**
   * The visual properties Weft has no prop for, as one comparable string (fills, strokes, corners,
   * and, when `withSpacing`, padding and spacing: on a `stack` or `grid` those are props). The
   * build stores it; a different value on read is a visual edit.
   */
  style(withSpacing: boolean): string;
  /** An instance's library component: its catalog kind mark and its style; undefined if gone. */
  main(): Promise<{ kind: string | undefined; style: string } | undefined>;
}

export const isContainer = (layer: Layer): boolean =>
  layer.kind === "frame" || layer.kind === "instance";

/**
 * The first layer that matches, searched one level at a time below a layer, through containers
 * only. Generic so the build can search the tool's own nodes before they are read as layers.
 */
export function findBelow<T>(
  layer: T,
  match: (child: T) => boolean,
  childrenOf: (node: T) => readonly T[] | undefined,
  depth = 0,
): T | undefined {
  const children = childrenOf(layer);
  if (children === undefined || depth > 8) return undefined;
  const direct = children.find(match);
  if (direct !== undefined) return direct;
  for (const child of children) {
    const found = findBelow(child, match, childrenOf, depth + 1);
    if (found !== undefined) return found;
  }
  return undefined;
}

/** The first text layer with this name below a layer. */
export const findText = (layer: Layer, name: string): Layer | undefined =>
  findBelow(
    layer,
    (c) => c.kind === "text" && c.name === name,
    (l) => (isContainer(l) ? l.children : undefined),
  );
