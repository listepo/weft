// What a Weft element looks like in a design tool, as plain data. The build writes this view and
// the read-back computes it again from the stored source: where the layer still shows the same
// view, the source is kept exactly as written, and only a difference counts as a designer's edit.
import type { ComponentDef, Node, Value } from "@weft/core";
import type { Token } from "@weft/catalog";
import { tokenPx } from "./tokens.ts";

/** The variant value of an enum prop or `state` that is not set and has no default. */
export const UNSET = "(unset)";

/** The library holds every combination of variant values; a kind past this many drops axes. */
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
  "slider",
  "stepper",
  "date-picker",
  "color-picker",
  "segmented-control",
  "combobox",
]);

/**
 * Values in attribute form (`{$.a}`, `{token.x}`), keyed by their JSON. The WebAssembly core writes
 * them (`displayTexts`) where it runs, and the build, in a plugin sandbox without WebAssembly, only
 * looks them up.
 */
export type Display = Readonly<Record<string, string>>;

function shownAs(display: Display, value: Value): string {
  const key = JSON.stringify(value);
  const text = Object.hasOwn(display, key) ? display[key] : undefined;
  if (text === undefined) throw new Error(`The build was given no display text for ${key}.`);
  return text;
}

/** The text a leaf shows: its `text` prop in attribute form, else its text content. */
export function textDisplay(node: Pick<Node, "props" | "children">, display: Display): string {
  const text = node.props?.["text"];
  if (text !== undefined) return shownAs(display, text);
  return (node.children ?? []).filter((c) => typeof c === "string").join(" ");
}

export function labelDisplay(
  props: Readonly<Record<string, Value>> | undefined,
  display: Display,
): string {
  const label = props?.["label"];
  return label === undefined ? "" : shownAs(display, label);
}

/** How a container lays out its children: a row, a column or a grid. */
export type Mode = "row" | "column" | "grid";
/** Cross-axis alignment. Weft's `stretch` is drawn as `start`, which both tools show the same way. */
export type Align = "start" | "center" | "end";

export type LayoutView = {
  mode: Mode;
  align: Align;
  wrap: boolean;
  columns: number;
  gap: { token?: string | undefined; px: number };
  padding: number;
};

const ALIGN: Readonly<Record<string, Align>> = {
  start: "start",
  center: "center",
  end: "end",
  stretch: "start",
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

/** The layout a frame gets: `stack` and `grid` from their props, every other kind fixed. */
export function layoutView(
  kind: string,
  props: Readonly<Record<string, Value>> | undefined,
  tokens: ReadonlyMap<string, Token> | undefined,
  parentMode: Mode | "none",
): LayoutView {
  const gapOf = () => {
    const gap = props?.["gap"];
    if (typeof gap === "object" && "token" in gap)
      return { token: gap.token, px: tokenPx(tokens?.get(gap.token)) ?? 0 };
    return { px: 0 };
  };
  const padding = PADDING[kind] ?? 8;
  if (kind === "stack") {
    const mode: Mode = props?.["direction"] === "row" ? "row" : "column";
    const align = props?.["align"];
    return {
      mode,
      // SPEC §5.1: a row without `align` centres its children, a column keeps them at the start.
      align: (typeof align === "string" ? ALIGN[align] : undefined) ?? (mode === "row" ? "center" : "start"),
      wrap: mode === "row" && props?.["wrap"] === true,
      columns: 1,
      gap: gapOf(),
      padding,
    };
  }
  if (kind === "grid") {
    const columns = props?.["columns"];
    return {
      mode: "grid",
      align: "start",
      wrap: false,
      columns:
        typeof columns === "number" && Number.isInteger(columns) && columns >= 1 ? columns : 1,
      gap: gapOf(),
      padding,
    };
  }
  const inherit = parentMode === "row" ? "row" : "column";
  return {
    mode: STRUCTURAL.has(kind) ? inherit : kind === "row" ? "row" : "column",
    align: "start",
    wrap: false,
    columns: 1,
    gap: { px: STRUCTURAL.has(kind) ? 0 : 8 },
    padding: STRUCTURAL.has(kind) ? 0 : padding,
  };
}

/** Kinds whose spacing is a Weft prop; on every other frame spacing is part of the style. */
export const LAYOUT_KINDS: ReadonlySet<string> = new Set(["stack", "grid"]);

/** Every combination of the axes' values, in axis order: the variants a library kind holds. */
export function combinations(axes: readonly Axis[]): Record<string, string>[] {
  let out: Record<string, string>[] = [{}];
  for (const axis of axes)
    out = out.flatMap((c) => axis.values.map((v) => ({ ...c, [axis.name]: v })));
  return out;
}
