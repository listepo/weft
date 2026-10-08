// Figma layers as the shared read-back sees them (`Layer` of @weft/design-tool). The wrapper reads
// the node lazily, so a large file costs only what the read visits. It reads `ReadNode`s, the
// read-only part of a node: Plugin API nodes are ones, and so are the REST API's once `rest.ts`
// has checked them, so both sources share this one wrapper.
import { KEY, readMark, type Layer, type LayerLayout } from "@weft/design-tool";
import type {
  FComponentProperties,
  FEffect,
  FLayout,
  FMixed,
  FOther,
  FPaint,
  FPluginData,
  FRectangle,
  FScene,
} from "./api.ts";
import { tokenPathOf } from "./tokens.ts";

/** The layout and style fields the read-back looks at. */
export type ReadLayout = Pick<
  FLayout,
  | "rotation"
  | "layoutMode"
  | "layoutWrap"
  | "itemSpacing"
  | "counterAxisAlignItems"
  | "paddingLeft"
  | "paddingRight"
  | "paddingTop"
  | "paddingBottom"
  | "gridColumnCount"
  | "gridRowGap"
  | "fills"
  | "strokes"
  | "effects"
  | "cornerRadius"
  | "boundVariables"
>;

type ReadBase = FPluginData & Pick<FScene, "id" | "name" | "visible" | "x" | "y">;

export type ReadContainer = ReadBase &
  ReadLayout & {
    readonly type: "FRAME" | "COMPONENT" | "COMPONENT_SET";
    readonly children: readonly ReadNode[];
  };

export type ReadInstance = ReadBase &
  ReadLayout & {
    readonly type: "INSTANCE";
    readonly children: readonly ReadNode[];
    readonly componentProperties: FComponentProperties;
    getMainComponentAsync(): Promise<(ReadBase & ReadLayout) | null>;
  };

export type ReadText = ReadBase & { readonly type: "TEXT"; readonly characters: string };

export type ReadOther = ReadBase & {
  readonly type: FRectangle["type"] | FOther["type"];
  readonly children?: readonly ReadNode[] | undefined;
  readonly fills?: readonly FPaint[] | FMixed | undefined;
};

/** A node as the read-back sees it; every Plugin API node (`FNode`) is one. */
export type ReadNode = ReadContainer | ReadInstance | ReadText | ReadOther;

/** Finds the design token a variable stands for; `figma.variables` is one. */
export type VariableLookup = {
  getVariableByIdAsync(id: string): Promise<(FPluginData & { name: string }) | null>;
};

const isReadContainer = (node: ReadNode): node is ReadContainer | ReadInstance =>
  node.type === "FRAME" ||
  node.type === "COMPONENT" ||
  node.type === "COMPONENT_SET" ||
  node.type === "INSTANCE";

function paints(list: readonly FPaint[] | symbol): unknown {
  if (typeof list === "symbol") return "mixed";
  return list.map((p) =>
    p.type === "SOLID"
      ? [p.color.r, p.color.g, p.color.b, p.opacity ?? 1, p.boundVariables?.color?.id ?? ""]
      : p.type,
  );
}

const effects = (list: readonly FEffect[]): unknown =>
  list.map((e) => (e.type === "BACKGROUND_BLUR" ? [e.type, e.radius, e.visible] : e.type));

/**
 * The visual properties Weft has no prop for, as one comparable string: fills, strokes, corner
 * radius and padding, plus spacing on frames whose spacing is not a prop, and the effects (a
 * material's background blur) when there are any.
 */
export function styleKey(layer: ReadLayout, withSpacing: boolean): string {
  const key = [
    paints(layer.fills),
    paints(layer.strokes),
    typeof layer.cornerRadius === "symbol" ? "mixed" : layer.cornerRadius,
    [layer.paddingLeft, layer.paddingRight, layer.paddingTop, layer.paddingBottom],
    withSpacing ? layer.itemSpacing : null,
  ];
  // Appended only when there is one, so a frame built before effects were read keeps the
  // fingerprint stored in its plugin data and does not read back as edited.
  if (layer.effects.length > 0) key.push(effects(layer.effects));
  // Likewise only when turned, so the key of every upright layer stays as it was.
  if (layer.rotation !== 0) key.push(layer.rotation);
  return JSON.stringify(key);
}

const MODE = { HORIZONTAL: "row", VERTICAL: "column", GRID: "grid", NONE: "none" } as const;
const ALIGN = { MIN: "start", CENTER: "center", MAX: "end", BASELINE: undefined } as const;

const NO_LAYOUT: LayerLayout = {
  mode: "none",
  label: "NONE",
  align: "start",
  alignLabel: "MIN",
  wrap: false,
  columns: 1,
};

/** Wraps a node for one read; variable lookups are cached across the read. */
export function figmaLayers(variables: VariableLookup): (node: ReadNode) => Layer {
  const paths = new Map<string, string | undefined>();
  const wrapped = new WeakMap<ReadNode, Layer>();

  const tokenOf = async (id: string | undefined): Promise<string | undefined> => {
    if (id === undefined) return undefined;
    if (paths.has(id)) return paths.get(id);
    const variable = await variables.getVariableByIdAsync(id);
    const path =
      variable === null ? undefined : (readMark(variable, KEY.token) ?? tokenPathOf(variable.name));
    paths.set(id, path);
    return path;
  };

  const wrap = (node: ReadNode): Layer => {
    const known = wrapped.get(node);
    if (known !== undefined) return known;
    const container = isReadContainer(node) ? node : undefined;
    const layer: Layer = {
      id: node.id,
      kind:
        node.type === "TEXT"
          ? "text"
          : node.type === "INSTANCE"
            ? "instance"
            : container !== undefined
              ? "frame"
              : "other",
      type: node.type.toLowerCase(),
      name: node.name,
      visible: node.visible,
      x: node.x,
      y: node.y,
      get children() {
        return "children" in node && node.children !== undefined
          ? node.children.map(wrap)
          : undefined;
      },
      characters: node.type === "TEXT" ? node.characters : "",
      get layout(): LayerLayout {
        if (container === undefined) return NO_LAYOUT;
        return {
          mode: MODE[container.layoutMode],
          label: container.layoutMode,
          align: ALIGN[container.counterAxisAlignItems],
          alignLabel: container.counterAxisAlignItems,
          wrap: container.layoutWrap === "WRAP",
          columns: container.gridColumnCount,
        };
      },
      get image() {
        const fills = "fills" in node ? node.fills : undefined;
        return (
          fills !== undefined && typeof fills !== "symbol" && fills.some((p) => p.type === "IMAGE")
        );
      },
      get painted() {
        const fills = "fills" in node ? node.fills : undefined;
        return (
          (fills !== undefined && (typeof fills === "symbol" || fills.length > 0)) ||
          (container?.strokes.length ?? 0) > 0
        );
      },
      get variant() {
        if (node.type !== "INSTANCE") return {};
        const values: Record<string, string> = {};
        for (const [name, property] of Object.entries(node.componentProperties))
          if (typeof property.value === "string") values[name] = property.value;
        return values;
      },
      gap: (grid) =>
        container === undefined ? 0 : grid ? container.gridRowGap : container.itemSpacing,
      gapToken: (grid) =>
        tokenOf(container?.boundVariables?.[grid ? "gridRowGap" : "itemSpacing"]?.id),
      style: (withSpacing) => (container === undefined ? "" : styleKey(container, withSpacing)),
      main: async () => {
        if (node.type !== "INSTANCE") return undefined;
        const main = await node.getMainComponentAsync();
        if (main === null) return undefined;
        return { kind: readMark(main, KEY.kind), style: styleKey(main, true) };
      },
      getPluginData: (key) => node.getPluginData(key),
      setPluginData: (key, value) => node.setPluginData(key, value),
    };
    wrapped.set(node, layer);
    return layer;
  };
  return wrap;
}
