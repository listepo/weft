// What a Weft element looks like on the Figma side, as plain data. The build writes this view and
// the read-back computes it again from the stored source: where the layer still shows the same
// view, the source is kept exactly as written, and only a difference counts as a designer's edit.
import type { ComponentDef, Node, Value } from "@weft/core";
import { formatValue } from "@weft/core";
import type { Token } from "@weft/catalog";
import type { FLayout, FPaint } from "./api.ts";
import { tokenPx } from "./tokens.ts";

/** The variant value of an enum prop or `state` that is not set and has no default. */
export const UNSET = "(unset)";

/** Figma builds every combination of variant values; a kind past this many drops axes. */
export const MAX_VARIANTS = 64;

export type Axis = { name: string; values: readonly string[]; fallback: string };

/** Variant properties of a kind: its enum props by name, then `state`. */
export function variantAxes(def: ComponentDef): Axis[] {
  const axes: Axis[] = [];
  for (const [name, prop] of Object.entries(def.props ?? {}).sort(([a], [b]) => (a < b ? -1 : 1))) {
    if (prop.type !== "enum" || prop.values === undefined || prop.values.length === 0) continue;
    const fallback = typeof prop.default === "string" ? prop.default : UNSET;
    const values = fallback === UNSET ? [UNSET, ...prop.values] : [...prop.values];
    axes.push({ name, values, fallback });
  }
  if (def.states !== undefined && def.states.length > 0)
    axes.push({ name: "state", values: [UNSET, ...def.states], fallback: UNSET });
  const size = (list: Axis[]) => list.reduce((n, a) => n * a.values.length, 1);
  while (axes.length > 0 && size(axes) > MAX_VARIANTS) axes.pop();
  return axes;
}

/** The variant an element shows: its literal value, else what an absent value renders as. */
export function variantOf(
  props: Readonly<Record<string, Value>> | undefined,
  axes: readonly Axis[],
): Record<string, string> {
  const out: Record<string, string> = {};
  for (const axis of axes) {
    const v = props?.[axis.name];
    out[axis.name] =
      typeof v === "string" && v !== UNSET && axis.values.includes(v) ? v : axis.fallback;
  }
  return out;
}

export function variantName(values: Readonly<Record<string, string>>, axes: readonly Axis[]) {
  return axes.map((a) => `${a.name}=${values[a.name] ?? a.fallback}`).join(", ");
}

/** Kinds without element content become library instances; the rest become frames. */
export const isLeafKind = (def: ComponentDef | undefined): boolean =>
  def !== undefined && (def.content === "none" || def.content === "text");

/** Kinds that show `label` as a visible caption or title (SPEC §2.2). */
export const CAPTION_KINDS: ReadonlySet<string> = new Set([
  "field",
  "checkbox",
  "switch",
  "radio-group",
  "select",
  "tab",
]);

/** The text a leaf shows: a bound or literal `text` prop in attribute form, else its content. */
export function textDisplay(node: Pick<Node, "props" | "children">): string {
  const text = node.props?.["text"];
  if (text !== undefined) return formatValue(text);
  return (node.children ?? []).filter((c) => typeof c === "string").join(" ");
}

export function labelDisplay(props: Readonly<Record<string, Value>> | undefined): string {
  const label = props?.["label"];
  return label === undefined ? "" : formatValue(label);
}

export type Mode = FLayout["layoutMode"];
export type Align = FLayout["counterAxisAlignItems"];

export type LayoutView = {
  mode: Mode;
  align: Align;
  wrap: boolean;
  columns: number;
  gap: { token?: string | undefined; px: number };
  padding: number;
};

const ALIGN: Readonly<Record<string, Align>> = {
  start: "MIN",
  center: "CENTER",
  end: "MAX",
  stretch: "MIN",
};

/** Padding of the frame a kind becomes; containers that group without a box have none. */
const PADDING: Readonly<Record<string, number>> = {
  screen: 24,
  stack: 0,
  grid: 0,
  each: 0,
  dialog: 16,
  alert: 12,
  section: 12,
  form: 0,
};

const STRUCTURAL = new Set(["each", "slot"]);

/** The auto layout a frame gets: `stack` and `grid` from their props, every other kind fixed. */
export function layoutView(
  kind: string,
  props: Readonly<Record<string, Value>> | undefined,
  tokens: ReadonlyMap<string, Token> | undefined,
  parentMode: Mode,
): LayoutView {
  const gapOf = () => {
    const gap = props?.["gap"];
    if (typeof gap === "object" && "token" in gap)
      return { token: gap.token, px: tokenPx(tokens?.get(gap.token)) ?? 0 };
    return { px: 0 };
  };
  const padding = PADDING[kind] ?? 8;
  if (kind === "stack") {
    const mode: Mode = props?.["direction"] === "row" ? "HORIZONTAL" : "VERTICAL";
    const align = props?.["align"];
    return {
      mode,
      align: (typeof align === "string" ? ALIGN[align] : undefined) ?? "MIN",
      wrap: mode === "HORIZONTAL" && props?.["wrap"] === true,
      columns: 1,
      gap: gapOf(),
      padding,
    };
  }
  if (kind === "grid") {
    const columns = props?.["columns"];
    return {
      mode: "GRID",
      align: "MIN",
      wrap: false,
      columns:
        typeof columns === "number" && Number.isInteger(columns) && columns >= 1 ? columns : 1,
      gap: gapOf(),
      padding,
    };
  }
  const inherit = parentMode === "HORIZONTAL" ? "HORIZONTAL" : "VERTICAL";
  return {
    mode: STRUCTURAL.has(kind) ? inherit : kind === "row" ? "HORIZONTAL" : "VERTICAL",
    align: "MIN",
    wrap: false,
    columns: 1,
    gap: { px: STRUCTURAL.has(kind) ? 0 : 8 },
    padding: STRUCTURAL.has(kind) ? 0 : padding,
  };
}

/** Kinds whose spacing is a Weft prop; on every other frame spacing is part of the style. */
export const LAYOUT_KINDS: ReadonlySet<string> = new Set(["stack", "grid"]);

function paints(list: readonly FPaint[] | symbol): unknown {
  if (typeof list === "symbol") return "mixed";
  return list.map((p) =>
    p.type === "SOLID"
      ? [p.color.r, p.color.g, p.color.b, p.opacity ?? 1, p.boundVariables?.color?.id ?? ""]
      : p.type,
  );
}

/**
 * The visual properties Weft has no prop for, as one comparable string: fills, strokes, corner
 * radius and padding, plus spacing on frames whose spacing is not a prop.
 */
export function styleKey(layer: FLayout, withSpacing: boolean): string {
  return JSON.stringify([
    paints(layer.fills),
    paints(layer.strokes),
    typeof layer.cornerRadius === "symbol" ? "mixed" : layer.cornerRadius,
    [layer.paddingLeft, layer.paddingRight, layer.paddingTop, layer.paddingBottom],
    withSpacing ? layer.itemSpacing : null,
  ]);
}
