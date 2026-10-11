// The Weft component library as plain data: which components a kind has (one per combination of
// its variant values) and what each one draws. Each tool package turns a `Drawing` into its own
// layers and finds the components it made by plugin data, so both tools draw the same library.
import type { Catalog, ComponentDef } from "@weft/core";
import type { RGB } from "./tokens.ts";
import { CAPTION_KINDS, isLeafKind, type Axis } from "./view.ts";

export type KindEntry<C> = {
  axes: Axis[];
  /** Variant components by variant name (`variant=primary, state=(unset)`); `""` without axes. */
  variants: Map<string, C>;
  fallback: C;
};

export const LIBRARY_PAGE = "Weft library";
export const LIBRARY_BOARD = "Weft components";
export const TOKEN_COLLECTION = "Weft tokens";

/**
 * The catalogs a library is built from (SPEC §10.4): the core first, then each catalog merged over
 * it, and the catalog that defined each kind. Without it the library is the one catalog it is given.
 */
export type LibrarySources = {
  catalogs: readonly { name: string; version: string }[];
  kinds: Readonly<Record<string, { catalog: string }>>;
};

/** What the library was built from (`weft-core@0.2.0 acme-ui@1.0.0`); stored on the library page and board. */
export const libraryTag = (catalog: Catalog, sources?: LibrarySources): string =>
  (sources?.catalogs ?? [catalog]).map(({ name, version }) => `${name}@${version}`).join(" ");

export type CatalogGroup = { name: string; version: string; kinds: string[] };

/**
 * The kinds of each catalog, in catalog order, so each tool can group the library by catalog; one
 * group when the library has one catalog. A kind no listed catalog claims goes with the first.
 */
export function catalogGroups(catalog: Catalog, sources?: LibrarySources): CatalogGroup[] {
  const groups = (sources?.catalogs ?? [catalog]).map(({ name, version }) => ({
    name,
    version,
    kinds: [] as string[],
  }));
  for (const kind of Object.keys(catalog.components)) {
    const owner =
      sources !== undefined && Object.hasOwn(sources.kinds, kind) ? sources.kinds[kind] : undefined;
    (groups.find((g) => g.name === owner?.catalog) ?? groups[0])?.kinds.push(kind);
  }
  return groups.filter((g) => g.kinds.length > 0);
}

export const INK: RGB = { r: 0.1, g: 0.11, b: 0.13 };
export const WHITE: RGB = { r: 1, g: 1, b: 1 };
export const GREY: RGB = { r: 0.9, g: 0.91, b: 0.92 };

/** A color: bound to a token when the library has it, else the fallback; or a plain color. */
export type Paint = { token: string; fallback: RGB } | { color: RGB };

/** A length: bound to a token when the library has it, else `px`. */
export type Length = { token?: string | undefined; px: number };

export type Font = "regular" | "bold";

export type TextDrawing = {
  type: "text";
  name: string;
  characters: string;
  font: Font;
  size: number;
  fill?: Paint | undefined;
};

export type RectDrawing = {
  type: "rect";
  name: string;
  width: number;
  height: number;
  radius?: number | undefined;
  fill: Paint;
  stroke?: Paint | undefined;
};

/** A frame with a row or column layout; the root of every component is one. */
export type BoxDrawing = {
  type: "box";
  name: string;
  direction: "row" | "column";
  gap: number;
  padX?: Length | undefined;
  padY?: Length | undefined;
  radius?: Length | undefined;
  fill?: Paint | undefined;
  stroke?: Paint | undefined;
  /** A fixed size; without it the box hugs its content. */
  size?: { width: number; height: number } | undefined;
  children: Drawing[];
};

export type Drawing = BoxDrawing | RectDrawing | TextDrawing;

export const TEXT_STYLE: Readonly<Record<string, { font: Font; size: number }>> = {
  heading: { font: "bold", size: 24 },
  column: { font: "bold", size: 14 },
};

export const INK_PAINT: Paint = { token: "color.ink", fallback: INK };
export const WHITE_PAINT: Paint = { token: "color.white", fallback: WHITE };

export const textDrawing = (
  name: string,
  characters: string,
  font: Font = "regular",
  size = 14,
  fill?: Paint,
): TextDrawing => ({ type: "text", name, characters, font, size, fill });

const part = (
  name: string,
  width: number,
  height: number,
  fill: Paint,
  radius = 0,
): RectDrawing => ({
  type: "rect",
  name,
  width,
  height,
  radius,
  fill,
});

const GREY_PAINT: Paint = { color: GREY };

/** A row of fixed size that holds the parts of a control. */
const strip = (name: string, width: number, height: number, children: Drawing[]): BoxDrawing => ({
  type: "box",
  name,
  direction: "row",
  gap: 0,
  size: { width, height },
  children,
});

/** The control a number, date or colour kind draws under its label: the same shape for each value. */
function control(kind: string): BoxDrawing {
  const cell = (name: string, width: number, characters: string): BoxDrawing => ({
    type: "box",
    name,
    direction: "row",
    gap: 0,
    padX: { px: 12 },
    padY: { px: 8 },
    stroke: INK_PAINT,
    fill: WHITE_PAINT,
    size: { width, height: 36 },
    children: [textDrawing("symbol", characters)],
  });
  switch (kind) {
    case "slider":
      // Track and thumb: a filled part, the round thumb, the rest.
      return strip("slider", 240, 16, [
        part("filled", 104, 16, INK_PAINT, 8),
        { ...part("thumb", 16, 16, WHITE_PAINT, 8), stroke: INK_PAINT },
        part("rest", 120, 16, GREY_PAINT, 8),
      ]);
    case "stepper":
      return strip("stepper", 160, 36, [
        cell("down", 40, "-"),
        cell("value", 80, "1"),
        cell("up", 40, "+"),
      ]);
    case "date-picker":
      return strip("picker", 240, 36, [
        cell("date", 200, "yyyy-mm-dd"),
        cell("calendar", 40, "31"),
      ]);
    default:
      return strip("swatch", 36, 36, [
        {
          ...part("color", 36, 36, { color: { r: 0.23, g: 0.51, b: 0.96 } }, 4),
          stroke: INK_PAINT,
        },
      ]);
  }
}

/** What the library component of a kind draws for one combination of variant values. */
export function drawing(
  kind: string,
  def: ComponentDef,
  values: Readonly<Record<string, string>>,
): BoxDrawing {
  const row = kind === "button" || kind === "checkbox" || kind === "switch" || kind === "radio";
  const box: BoxDrawing = {
    type: "box",
    name: kind,
    direction: row ? "row" : "column",
    gap: 8,
    children: [],
  };
  const sample = kind.charAt(0).toUpperCase() + kind.slice(1).replace("-", " ");

  if (!isLeafKind(def)) {
    // A container: an empty frame. Its instances cannot take children, so a designer detaches
    // one to fill it; the detached frame keeps the kind as its name.
    const space = { token: "space.sm", px: 8 };
    return {
      ...box,
      padX: space,
      padY: space,
      stroke: INK_PAINT,
      children: [textDrawing("hint", `${kind}: detach to add content`, "regular", 11)],
    };
  }
  if (kind === "button") {
    const variant = values["variant"];
    const filled = variant === "primary" || variant === "danger";
    return {
      ...box,
      padX: { token: "space.md", px: 16 },
      padY: { token: "space.sm", px: 8 },
      radius: { token: "radius.md", px: 8 },
      fill: filled ? { token: `color.action.${variant}`, fallback: INK } : WHITE_PAINT,
      stroke: filled ? undefined : INK_PAINT,
      children: [textDrawing("text", "Button", "regular", 14, filled ? WHITE_PAINT : INK_PAINT)],
    };
  }
  if (kind === "field") {
    const input: BoxDrawing = {
      type: "box",
      name: "box",
      direction: "row",
      gap: 0,
      padX: { px: 8 },
      padY: { px: 8 },
      radius: { token: "radius.sm", px: 4 },
      fill: WHITE_PAINT,
      stroke: INK_PAINT,
      size: { width: 240, height: 36 },
      children: [],
    };
    return { ...box, gap: 4, children: [textDrawing("label", "Label", "regular", 12), input] };
  }
  if (
    kind === "slider" ||
    kind === "stepper" ||
    kind === "date-picker" ||
    kind === "color-picker"
  ) {
    return {
      ...box,
      gap: 4,
      children: [textDrawing("label", "Label", "regular", 12), control(kind)],
    };
  }
  if (kind === "image") {
    const rect: RectDrawing = {
      type: "rect",
      name: "image",
      width: 160,
      height: 100,
      fill: { color: GREY },
    };
    return { ...box, children: [rect] };
  }
  if (kind === "model") {
    // A design tool draws no 3D scene, so the model is the rectangle its still fills, in the
    // still's 4:3 shape (SPEC §9).
    const rect: RectDrawing = {
      type: "rect",
      name: "model",
      width: 160,
      height: 120,
      fill: { color: GREY },
    };
    return { ...box, children: [rect] };
  }
  if (kind === "checkbox" || kind === "switch" || kind === "radio") {
    const mark: RectDrawing = {
      type: "rect",
      name: "mark",
      width: kind === "switch" ? 32 : 16,
      height: 16,
      radius: kind === "checkbox" ? 4 : 999,
      fill: WHITE_PAINT,
      stroke: INK_PAINT,
    };
    const caption = textDrawing(
      kind === "radio" ? "text" : "label",
      CAPTION_KINDS.has(kind) ? "Label" : sample,
    );
    return { ...box, children: [mark, caption] };
  }
  const style = TEXT_STYLE[kind] ?? { font: "regular", size: 14 };
  const tone = values["tone"];
  const fill: Paint =
    kind === "link"
      ? { token: "color.action.primary", fallback: INK }
      : tone === "danger"
        ? { token: "color.action.danger", fallback: INK }
        : INK_PAINT;
  const padded = kind === "menu-item" || kind === "option" || kind === "segment";
  return {
    ...box,
    padX: padded ? { token: "space.sm", px: 8 } : undefined,
    children: [textDrawing("text", sample, style.font, style.size, fill)],
  };
}
