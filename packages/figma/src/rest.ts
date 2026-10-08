// Nodes of the Figma REST API (`GET /v1/files/:key/nodes`) as `ReadNode`s, so the read-back that
// serves the plugin reads them too. The response is untrusted: every field is checked and a field
// of the wrong type takes the default the REST docs give, so a crafted file cannot throw here.
// Children are wrapped on first visit, which leaves the size limits to the read-back.
import type { FAlias, FComponentProperties, FEffect, FPaint } from "./api.ts";
import type { ReadInstance, ReadLayout, ReadNode, ReadOther, VariableLookup } from "./layer.ts";

type Json = Record<string, unknown>;

const isObject = (value: unknown): value is Json =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const own = (value: Json, key: string): unknown =>
  Object.hasOwn(value, key) ? value[key] : undefined;
const num = (value: unknown, fallback: number): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback;
const str = (value: unknown, fallback: string): string =>
  typeof value === "string" ? value : fallback;
const list = (value: unknown): unknown[] => (Array.isArray(value) ? value : []);
const oneOf = <T extends string>(value: unknown, values: readonly T[], fallback: T): T =>
  values.includes(value as T) ? (value as T) : fallback;

const MIXED: symbol = Symbol("mixed");
const CONTAINERS = ["FRAME", "COMPONENT", "COMPONENT_SET"] as const;

/**
 * The REST API returns no variables without the Enterprise-only variables endpoint, so a bound
 * gap cannot be named; the read-back then matches the gap by its value, as for a typed number.
 */
export const NO_VARIABLES: VariableLookup = { getVariableByIdAsync: async () => null };

function paint(value: unknown): FPaint | undefined {
  if (!isObject(value) || typeof value["type"] !== "string") return undefined;
  if (value["type"] !== "SOLID") return { type: value["type"] } as FPaint;
  const color = isObject(value["color"]) ? value["color"] : {};
  const bound = isObject(value["boundVariables"])
    ? own(value["boundVariables"], "color")
    : undefined;
  const alias: FAlias | undefined =
    isObject(bound) && typeof bound["id"] === "string"
      ? { type: "VARIABLE_ALIAS", id: bound["id"] }
      : undefined;
  return {
    type: "SOLID",
    color: { r: num(color["r"], 0), g: num(color["g"], 0), b: num(color["b"], 0) },
    opacity: num(value["opacity"], 1),
    ...(alias === undefined ? {} : { boundVariables: { color: alias } }),
  };
}

const paints = (value: unknown): FPaint[] => list(value).flatMap((p) => paint(p) ?? []);

function effect(value: unknown): FEffect | undefined {
  if (!isObject(value) || typeof value["type"] !== "string") return undefined;
  if (value["type"] !== "BACKGROUND_BLUR") return { type: value["type"] } as FEffect;
  return {
    type: "BACKGROUND_BLUR",
    radius: num(value["radius"], 0),
    visible: value["visible"] !== false,
  };
}

function cornerRadius(node: Json): number | symbol {
  const corners = list(node["rectangleCornerRadii"]);
  if (corners.length === 4 && corners.some((c) => c !== corners[0])) return MIXED;
  return num(node["cornerRadius"], 0);
}

// The Plugin API counts degrees; the REST API's value is taken as radians (see README).
const degrees = (radians: number): number => Math.round(((radians * 180) / Math.PI) * 1e9) / 1e9;

function layout(node: Json): ReadLayout {
  return {
    rotation: degrees(num(node["rotation"], 0)),
    layoutMode: oneOf(node["layoutMode"], ["NONE", "HORIZONTAL", "VERTICAL", "GRID"], "NONE"),
    layoutWrap: oneOf(node["layoutWrap"], ["NO_WRAP", "WRAP"], "NO_WRAP"),
    itemSpacing: num(node["itemSpacing"], 0),
    counterAxisAlignItems: oneOf(
      node["counterAxisAlignItems"],
      ["MIN", "CENTER", "MAX", "BASELINE"],
      "MIN",
    ),
    paddingLeft: num(node["paddingLeft"], 0),
    paddingRight: num(node["paddingRight"], 0),
    paddingTop: num(node["paddingTop"], 0),
    paddingBottom: num(node["paddingBottom"], 0),
    gridColumnCount: num(node["gridColumnCount"], 1),
    gridRowGap: num(node["gridRowGap"], 0),
    fills: paints(node["fills"]),
    strokes: paints(node["strokes"]),
    effects: list(node["effects"]).flatMap((e) => effect(e) ?? []),
    cornerRadius: cornerRadius(node),
    boundVariables: undefined,
  };
}

function componentProperties(value: unknown): FComponentProperties {
  const out: Record<string, { type: string; value: string | boolean }> = {};
  if (!isObject(value)) return out;
  for (const [name, property] of Object.entries(value)) {
    if (!isObject(property) || typeof property["type"] !== "string") continue;
    const v = property["value"];
    if (typeof v === "string" || typeof v === "boolean")
      out[name] = { type: property["type"], value: v };
  }
  return out;
}

export type RestOptions = {
  /** The plugin whose plugin data the response carries (`plugin_data=<id>`). */
  pluginId: string;
  /** Main components by node id, from a second request; an instance of another is foreign. */
  components?: ReadonlyMap<string, Json> | undefined;
};

function baseOf(node: Json, pluginId: string) {
  const data = isObject(node["pluginData"]) ? own(node["pluginData"], pluginId) : undefined;
  const box = isObject(node["absoluteBoundingBox"]) ? node["absoluteBoundingBox"] : {};
  return {
    id: str(node["id"], ""),
    name: str(node["name"], ""),
    visible: node["visible"] !== false,
    x: num(box["x"], 0),
    y: num(box["y"], 0),
    getPluginData: (key: string) => (isObject(data) ? str(own(data, key), "") : ""),
    setPluginData: () => {
      throw new Error("a node read through the REST API is read-only");
    },
  };
}

/** The REST JSON of one node as a `ReadNode`; anything that is not a node reads as an empty one. */
export function restNode(value: unknown, options: RestOptions): ReadNode {
  const node = isObject(value) ? value : {};
  const base = baseOf(node, options.pluginId);
  let children: ReadNode[] | undefined;
  const kids = () => (children ??= list(node["children"]).map((c) => restNode(c, options)));
  const type = str(node["type"], "");
  if (type === "TEXT") return { ...base, type, characters: str(node["characters"], "") };
  if (type === "INSTANCE") {
    const instance: ReadInstance = {
      ...base,
      ...layout(node),
      type,
      get children() {
        return kids();
      },
      componentProperties: componentProperties(node["componentProperties"]),
      getMainComponentAsync: async () => {
        const id = node["componentId"];
        const main = typeof id === "string" ? options.components?.get(id) : undefined;
        return main === undefined ? null : { ...baseOf(main, options.pluginId), ...layout(main) };
      },
    };
    return instance;
  }
  const container = CONTAINERS.find((t) => t === type);
  if (container !== undefined)
    return {
      ...base,
      ...layout(node),
      type: container,
      get children() {
        return kids();
      },
    };
  return {
    ...base,
    type: type as ReadOther["type"],
    get children() {
      return Array.isArray(node["children"]) ? kids() : undefined;
    },
    fills: paints(node["fills"]),
  };
}
