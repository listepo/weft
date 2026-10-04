// Figma layers as the shared read-back sees them (`Layer` of @weft/design-tool). The wrapper reads
// the node lazily, so a large file costs only what the read visits.
import { KEY, readMark, type Layer, type LayerLayout } from "@weft/design-tool";
import type { FigmaApi, FLayout, FNode, FPaint } from "./api.ts";
import { isContainer } from "./api.ts";
import { tokenPathOf } from "./tokens.ts";

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
export function figmaLayers(api: FigmaApi): (node: FNode) => Layer {
  const paths = new Map<string, string | undefined>();
  const wrapped = new WeakMap<FNode, Layer>();

  const tokenOf = async (id: string | undefined): Promise<string | undefined> => {
    if (id === undefined) return undefined;
    if (paths.has(id)) return paths.get(id);
    const variable = await api.variables.getVariableByIdAsync(id);
    const path =
      variable === null ? undefined : (readMark(variable, KEY.token) ?? tokenPathOf(variable.name));
    paths.set(id, path);
    return path;
  };

  const wrap = (node: FNode): Layer => {
    const known = wrapped.get(node);
    if (known !== undefined) return known;
    const container = isContainer(node) ? node : undefined;
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
