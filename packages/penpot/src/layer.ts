// Penpot shapes as the shared read-back sees them (`Layer` of @weft/design-tool), and the pieces of
// a shape both the build and the read need: plugin data, the style fingerprint, the layout.
import { KEY, readMark, type Layer, type LayerLayout, type PluginData } from "@weft/design-tool";
import type { PBlur, PBoard, PCommonLayout, PFill, PSharedData, PShape, PStroke } from "./api.ts";
import { childrenOf, isBoard } from "./api.ts";

/**
 * The namespace of Weft's plugin data. Penpot keys private plugin data by the plugin's id, which
 * is random per install, so a file built by one install could not be read by another; shared
 * plugin data is keyed by a namespace the plugin chooses.
 */
export const NAMESPACE = "weft";

/**
 * Penpot refuses a text shape without characters, so an empty Weft text is drawn as a zero-width
 * space and read back as empty.
 */
export const EMPTY_TEXT = "​";

/** A shape's plugin data, with `""` for a key Penpot reports as missing. */
export function dataOf(shape: PSharedData): PluginData {
  return {
    // Penpot returns nothing for a missing key, whatever its types say.
    getPluginData: (key) => shape.getSharedPluginData(NAMESPACE, key) ?? "",
    setPluginData: (key, value) => shape.setSharedPluginData(NAMESPACE, key, value),
  };
}

export const layoutOf = (shape: PShape): PCommonLayout | undefined =>
  isBoard(shape) ? (shape.grid ?? shape.flex) : undefined;

/** The gap between children: the row gap of a grid, else the gap along a flex layout's axis. */
export function gapField(board: PBoard, grid: boolean): "rowGap" | "columnGap" {
  if (grid || board.flex === undefined) return "rowGap";
  return board.flex.dir.startsWith("row") ? "columnGap" : "rowGap";
}

const hex = (color: string | undefined): string => (color ?? "").toLowerCase();

function fills(list: readonly PFill[] | "mixed"): unknown {
  if (list === "mixed") return "mixed";
  return list.map((f) =>
    f.fillImage !== undefined && f.fillImage !== null
      ? "image"
      : f.fillColorGradient !== undefined && f.fillColorGradient !== null
        ? "gradient"
        : [hex(f.fillColor), f.fillOpacity ?? 1],
  );
}

const strokes = (list: readonly PStroke[]): unknown =>
  list.map((s) => [hex(s.strokeColor), s.strokeOpacity ?? 1, s.strokeWidth ?? 1]);

const blur = (b: PBlur | undefined): unknown =>
  b === undefined ? null : [b.value, b.hidden ?? false];

/**
 * The visual properties Weft has no prop for, as one comparable string: fills, strokes, the
 * background blur (a material's), corner radius and padding, plus spacing on boards whose spacing
 * is not a prop. Token bindings are left
 * out: Penpot applies a token after the call returns, so the build could not see its own binding,
 * and a binding without a value change shows nothing on the canvas.
 */
export function styleKey(shape: PShape, withSpacing: boolean): string {
  if (shape.type === "text") return "";
  const layout = layoutOf(shape);
  const key: unknown[] = [
    fills(shape.fills),
    strokes(shape.strokes),
    shape.borderRadius,
    layout === undefined
      ? [0, 0, 0, 0]
      : [layout.leftPadding, layout.rightPadding, layout.topPadding, layout.bottomPadding],
    withSpacing && isBoard(shape) && layout !== undefined
      ? layout[gapField(shape, shape.grid !== undefined)]
      : null,
  ];
  // Appended only when there is one, so a board built before the blur was read keeps the
  // fingerprint stored in its plugin data and does not read back as edited.
  if (shape.backgroundBlur !== undefined) key.push(blur(shape.backgroundBlur));
  // Likewise only when turned, so the key of every upright shape stays as it was.
  if (shape.rotation !== 0) key.push(shape.rotation);
  return JSON.stringify(key);
}

const NO_LAYOUT: LayerLayout = {
  mode: "none",
  label: "none",
  align: "start",
  alignLabel: "start",
  wrap: false,
  columns: 1,
};

function layoutView(shape: PShape): LayerLayout {
  if (!isBoard(shape)) return NO_LAYOUT;
  const align = (layout: PCommonLayout) => {
    const label = layout.alignItems ?? "start";
    return { align: label === "stretch" ? undefined : label, alignLabel: label };
  };
  if (shape.grid !== undefined)
    return {
      mode: "grid",
      label: "grid",
      ...align(shape.grid),
      wrap: false,
      columns: shape.grid.columns.length,
    };
  const flex = shape.flex;
  if (flex === undefined) return NO_LAYOUT;
  return {
    // Weft has no reversed stack, so a reversed flex layout is read as having no direction.
    mode: flex.dir === "row" ? "row" : flex.dir === "column" ? "column" : "none",
    label: flex.dir,
    ...align(flex),
    wrap: flex.wrap === "wrap",
    columns: 1,
  };
}

/** A grid board's children in cell order: row by row, left to right. */
function cellOrder(children: readonly PShape[]): PShape[] {
  const at = (s: PShape) => [
    s.layoutCell?.row ?? Number.MAX_SAFE_INTEGER,
    s.layoutCell?.column ?? Number.MAX_SAFE_INTEGER,
  ];
  return [...children].sort((a, b) => {
    const [ra = 0, ca = 0] = at(a);
    const [rb = 0, cb = 0] = at(b);
    return ra - rb || ca - cb;
  });
}

/** Whether a shape is the head of a component copy: what Penpot's UI calls an instance. */
export const isInstance = (shape: PShape): boolean =>
  shape.isComponentCopyInstance() && shape.isComponentHead();

/** Wraps a shape for one read. */
export function penpotLayers(): (shape: PShape) => Layer {
  const wrapped = new WeakMap<PShape, Layer>();
  const wrap = (shape: PShape): Layer => {
    const known = wrapped.get(shape);
    if (known !== undefined) return known;
    const board = isBoard(shape) ? shape : undefined;
    const instance = board !== undefined && isInstance(shape);
    const layer: Layer = {
      id: shape.id,
      kind:
        shape.type === "text"
          ? "text"
          : instance
            ? "instance"
            : board !== undefined
              ? "frame"
              : "other",
      type: shape.type,
      name: shape.name,
      visible: !shape.hidden,
      x: shape.x,
      y: shape.y,
      get children() {
        const children = childrenOf(shape);
        if (children === undefined) return undefined;
        return (board?.grid !== undefined ? cellOrder(children) : children).map(wrap);
      },
      characters:
        shape.type === "text" ? (shape.characters === EMPTY_TEXT ? "" : shape.characters) : "",
      get layout() {
        return layoutView(shape);
      },
      get image() {
        return (
          shape.type === "image" ||
          (shape.fills !== "mixed" &&
            shape.fills.some((f) => f.fillImage !== undefined && f.fillImage !== null))
        );
      },
      get painted() {
        return (
          shape.fills === "mixed" ||
          shape.fills.length > 0 ||
          (board !== undefined && shape.strokes.length > 0)
        );
      },
      get variant() {
        if (!instance) return {};
        const component = shape.component();
        return component?.isVariant() === true ? { ...component.variantProps } : {};
      },
      gap: (grid) => {
        const layout = board === undefined ? undefined : layoutOf(board);
        return board === undefined || layout === undefined ? 0 : layout[gapField(board, grid)];
      },
      gapToken: async (grid) =>
        board === undefined ? undefined : board.tokens[gapField(board, grid)],
      style: (withSpacing) => (board === undefined ? "" : styleKey(shape, withSpacing)),
      main: async () => {
        if (!instance) return undefined;
        const main = shape.component()?.mainInstance();
        if (main === undefined) return undefined;
        return { kind: readMark(dataOf(main), KEY.kind), style: styleKey(main, true) };
      },
      ...dataOf(shape),
    };
    wrapped.set(shape, layer);
    return layer;
  };
  return wrap;
}
