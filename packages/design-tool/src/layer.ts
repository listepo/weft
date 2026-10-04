// A layer of a design file as the read-back sees it, whatever tool it lives in. Each tool package
// wraps its own nodes in this interface (Figma's frames and instances, Penpot's boards and
// component copies), so the mapping back to Weft is written once. Everything a layer reports came
// from a file the plugin does not control, so it is input to check, not a fact to trust.
import type { PluginData } from "./keys.ts";
import type { Align, Mode } from "./view.ts";

/**
 * - `frame`: a container the reader descends into (Figma frames and components, Penpot boards).
 * - `instance`: a copy of a library component.
 * - `text`: a text layer.
 * - `other`: groups, shapes, vectors and anything else; a group still has children.
 */
export type LayerKind = "frame" | "instance" | "text" | "other";

export type LayerLayout = {
  /** `none`: free positions, or a layout Weft has no form for (`label` names it). */
  mode: Mode | "none";
  /** The tool's own name for the layout, for loss notes. */
  label: string;
  /** Undefined when the tool's alignment has no Weft value (`alignLabel` names it). */
  align: Align | undefined;
  alignLabel: string;
  wrap: boolean;
  columns: number;
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
  /**
   * The visual properties Weft has no prop for, as one comparable string (fills, strokes, corners,
   * padding, and spacing when `withSpacing`). The build stores it; a different value on read is a
   * visual edit.
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
