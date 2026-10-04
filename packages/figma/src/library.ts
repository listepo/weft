// The Weft component library in a Figma file: one page with a component set per catalog kind
// (a single component when the kind has no variant properties) and one variable collection with
// the design tokens. Running it again finds what is there by plugin data and adds only what is
// missing, so a designer's changes to existing components survive.
import type { Token } from "@weft/catalog";
import type { Catalog, ComponentDef } from "@weft/core";
import type {
  FBindable,
  FCollection,
  FComponent,
  FFont,
  FFrame,
  FigmaApi,
  FLayout,
  FPage,
  FRGB,
  FRGBA,
  FSolid,
  FText,
  FVariable,
} from "./api.ts";
import { KEY, readMark } from "./keys.ts";
import { tokenColor, tokenPx, variableName } from "./tokens.ts";
import { CAPTION_KINDS, isLeafKind, variantAxes, variantName, type Axis } from "./view.ts";

export type KindEntry = {
  axes: Axis[];
  /** Variant components by variant name (`variant=primary, state=(unset)`); `""` without axes. */
  variants: Map<string, FComponent>;
  fallback: FComponent;
};

export type Library = {
  page: FPage;
  variables: Map<string, FVariable>;
  kinds: Map<string, KindEntry>;
};

export const FONT: { regular: FFont; bold: FFont } = {
  regular: { family: "Inter", style: "Regular" },
  bold: { family: "Inter", style: "Bold" },
};

export async function loadFonts(api: FigmaApi): Promise<void> {
  await Promise.all([api.loadFontAsync(FONT.regular), api.loadFontAsync(FONT.bold)]);
}

const LIBRARY_PAGE = "Weft library";
const COLLECTION = "Weft tokens";
const INK: FRGB = { r: 0.1, g: 0.11, b: 0.13 };
const WHITE: FRGB = { r: 1, g: 1, b: 1 };
const GREY: FRGB = { r: 0.9, g: 0.91, b: 0.92 };

export async function ensureLibrary(
  api: FigmaApi,
  catalog: Catalog,
  tokens: ReadonlyMap<string, Token>,
): Promise<Library> {
  await loadFonts(api);
  const tag = `${catalog.name}@${catalog.version}`;
  let page = api.root.children.find((p) => readMark(p, KEY.library) !== undefined);
  if (page === undefined) {
    page = api.createPage();
    page.name = LIBRARY_PAGE;
  }
  page.setPluginData(KEY.library, tag);
  await page.loadAsync();
  const library: Library = {
    page,
    variables: await ensureVariables(api, tokens),
    kinds: new Map(),
  };

  let board = page.children.find(
    (n): n is FFrame => n.type === "FRAME" && readMark(n, KEY.library) !== undefined,
  );
  if (board === undefined) {
    board = api.createFrame();
    board.name = "Weft components";
    board.setPluginData(KEY.library, tag);
    autoLayout(board, "HORIZONTAL", 32, 32);
    board.layoutWrap = "WRAP";
    board.fills = [];
    page.appendChild(board);
  }
  for (const child of board.children) {
    const kind = readMark(child, KEY.kind);
    const def = kind === undefined ? undefined : catalog.components[kind];
    if (kind === undefined || def === undefined || !Object.hasOwn(catalog.components, kind))
      continue;
    const axes = variantAxes(def);
    if (child.type === "COMPONENT") {
      library.kinds.set(kind, { axes, variants: new Map([["", child]]), fallback: child });
    } else if (child.type === "COMPONENT_SET") {
      const variants = new Map<string, FComponent>();
      for (const c of child.children) {
        if (c.type === "COMPONENT" && c.variantProperties !== null)
          variants.set(variantName(c.variantProperties, axes), c);
      }
      library.kinds.set(kind, { axes, variants, fallback: child.defaultVariant });
    }
  }
  for (const [kind, def] of Object.entries(catalog.components)) {
    if (!library.kinds.has(kind))
      library.kinds.set(kind, createKind(api, library, kind, def, board));
  }
  return library;
}

async function ensureVariables(
  api: FigmaApi,
  tokens: ReadonlyMap<string, Token>,
): Promise<Map<string, FVariable>> {
  const collections = await api.variables.getLocalVariableCollectionsAsync();
  let collection: FCollection | undefined = collections.find(
    (c) => readMark(c, KEY.library) !== undefined,
  );
  if (collection === undefined) {
    collection = api.variables.createVariableCollection(COLLECTION);
    collection.setPluginData(KEY.library, COLLECTION);
  }
  const variables = new Map<string, FVariable>();
  for (const v of await api.variables.getLocalVariablesAsync()) {
    const path = readMark(v, KEY.token);
    if (path !== undefined && v.variableCollectionId === collection.id) variables.set(path, v);
  }
  for (const [path, token] of tokens) {
    const value = variableValue(token);
    if (value === undefined) continue;
    let variable = variables.get(path);
    if (variable !== undefined && variable.resolvedType !== value.type) continue;
    if (variable === undefined) {
      variable = api.variables.createVariable(variableName(path), collection, value.type);
      variable.setPluginData(KEY.token, path);
      variables.set(path, variable);
    }
    variable.setValueForMode(collection.defaultModeId, value.value);
  }
  return variables;
}

function variableValue(
  token: Token,
): { type: "FLOAT"; value: number } | { type: "COLOR"; value: FRGBA } | undefined {
  const px = tokenPx(token);
  if (px !== undefined) return { type: "FLOAT", value: px };
  const color = tokenColor(token);
  return color === undefined ? undefined : { type: "COLOR", value: color };
}

function combinations(axes: readonly Axis[]): Record<string, string>[] {
  let out: Record<string, string>[] = [{}];
  for (const axis of axes)
    out = out.flatMap((c) => axis.values.map((v) => ({ ...c, [axis.name]: v })));
  return out;
}

function createKind(
  api: FigmaApi,
  library: Library,
  kind: string,
  def: ComponentDef,
  board: FFrame,
): KindEntry {
  const axes = variantAxes(def);
  const variants = new Map<string, FComponent>();
  const components = combinations(axes).map((values) => {
    const c = drawComponent(api, library, kind, def, values);
    c.name = axes.length === 0 ? kind : variantName(values, axes);
    c.setPluginData(KEY.kind, kind);
    variants.set(axes.length === 0 ? "" : c.name, c);
    return c;
  });
  const fallback = components[0] as FComponent;
  if (axes.length === 0) {
    board.appendChild(fallback);
    return { axes, variants, fallback };
  }
  const set = api.combineAsVariants(components, board);
  set.name = kind;
  set.setPluginData(KEY.kind, kind);
  autoLayout(set, "HORIZONTAL", 16, 16);
  set.layoutWrap = "WRAP";
  return { axes, variants, fallback: set.defaultVariant };
}

// --- drawing -------------------------------------------------------------------------------

export function autoLayout(
  layer: FLayout,
  mode: "HORIZONTAL" | "VERTICAL",
  spacing: number,
  padding: number,
): void {
  layer.layoutMode = mode;
  layer.primaryAxisSizingMode = "AUTO";
  layer.counterAxisSizingMode = "AUTO";
  layer.itemSpacing = spacing;
  layer.paddingLeft = padding;
  layer.paddingRight = padding;
  layer.paddingTop = padding;
  layer.paddingBottom = padding;
}

/** A solid paint bound to the token's variable when the library has it. */
export function paint(api: FigmaApi, library: Library, path: string, fallback: FRGB): FSolid {
  const solid: FSolid = { type: "SOLID", color: fallback };
  const variable = library.variables.get(path);
  return variable === undefined
    ? solid
    : api.variables.setBoundVariableForPaint(solid, "color", variable);
}

/** Binds numeric fields to a token's variable, or sets the fallback when the library lacks it. */
export function bindNumber(
  layer: FLayout,
  fields: readonly FBindable[],
  library: Library,
  path: string | undefined,
  px: number,
): void {
  for (const field of fields) {
    const variable = path === undefined ? undefined : library.variables.get(path);
    if (variable === undefined) setNumber(layer, field, px);
    else layer.setBoundVariable(field, variable);
  }
}

function setNumber(layer: FLayout, field: FBindable, value: number): void {
  switch (field) {
    case "itemSpacing":
      layer.itemSpacing = value;
      break;
    case "gridRowGap":
      layer.gridRowGap = value;
      break;
    case "gridColumnGap":
      layer.gridColumnGap = value;
      break;
    case "paddingLeft":
      layer.paddingLeft = value;
      break;
    case "paddingRight":
      layer.paddingRight = value;
      break;
    case "paddingTop":
      layer.paddingTop = value;
      break;
    case "paddingBottom":
      layer.paddingBottom = value;
      break;
    default:
      layer.cornerRadius = value;
  }
}

const CORNERS: readonly FBindable[] = [
  "topLeftRadius",
  "topRightRadius",
  "bottomLeftRadius",
  "bottomRightRadius",
];

export function makeText(
  api: FigmaApi,
  name: string,
  characters: string,
  font: FFont = FONT.regular,
  size = 14,
): FText {
  const text = api.createText();
  text.name = name;
  text.fontName = font;
  text.fontSize = size;
  text.characters = characters;
  return text;
}

const TEXT_STYLE: Readonly<Record<string, { font: FFont; size: number }>> = {
  heading: { font: FONT.bold, size: 24 },
  column: { font: FONT.bold, size: 14 },
};

function drawComponent(
  api: FigmaApi,
  library: Library,
  kind: string,
  def: ComponentDef,
  values: Readonly<Record<string, string>>,
): FComponent {
  const c = api.createComponent();
  const row = kind === "button" || kind === "checkbox" || kind === "switch" || kind === "radio";
  autoLayout(c, row ? "HORIZONTAL" : "VERTICAL", 8, 0);
  c.fills = [];
  const ink = paint(api, library, "color.ink", INK);
  const sample = kind.charAt(0).toUpperCase() + kind.slice(1).replace("-", " ");

  if (!isLeafKind(def)) {
    // A container: an empty frame. Its instances cannot take children, so a designer detaches
    // one to fill it; the detached frame keeps the kind as its name.
    bindNumber(
      c,
      ["paddingLeft", "paddingRight", "paddingTop", "paddingBottom"],
      library,
      "space.sm",
      8,
    );
    c.strokes = [ink];
    c.appendChild(makeText(api, "hint", `${kind}: detach to add content`, FONT.regular, 11));
    return c;
  }

  if (kind === "button") {
    const variant = values["variant"];
    const filled = variant === "primary" || variant === "danger";
    bindNumber(c, ["paddingLeft", "paddingRight"], library, "space.md", 16);
    bindNumber(c, ["paddingTop", "paddingBottom"], library, "space.sm", 8);
    bindNumber(c, CORNERS, library, "radius.md", 8);
    c.fills = [
      filled
        ? paint(api, library, `color.action.${variant}`, INK)
        : paint(api, library, "color.white", WHITE),
    ];
    if (!filled) c.strokes = [ink];
    const label = makeText(api, "text", "Button");
    label.fills = [filled ? paint(api, library, "color.white", WHITE) : ink];
    c.appendChild(label);
    return c;
  }
  if (kind === "field") {
    c.itemSpacing = 4;
    c.appendChild(makeText(api, "label", "Label", FONT.regular, 12));
    const box = api.createFrame();
    box.name = "box";
    autoLayout(box, "HORIZONTAL", 0, 8);
    box.fills = [paint(api, library, "color.white", WHITE)];
    box.strokes = [ink];
    bindNumber(box, CORNERS, library, "radius.sm", 4);
    box.resize(240, 36);
    box.primaryAxisSizingMode = "FIXED";
    box.counterAxisSizingMode = "FIXED";
    c.appendChild(box);
    return c;
  }
  if (kind === "image") {
    const rect = api.createRectangle();
    rect.name = "image";
    rect.resize(160, 100);
    rect.fills = [{ type: "SOLID", color: GREY }];
    c.appendChild(rect);
    return c;
  }
  if (kind === "checkbox" || kind === "switch" || kind === "radio") {
    const mark = api.createRectangle();
    mark.name = "mark";
    mark.resize(kind === "switch" ? 32 : 16, 16);
    mark.cornerRadius = kind === "checkbox" ? 4 : 999;
    mark.fills = [paint(api, library, "color.white", WHITE)];
    mark.strokes = [ink];
    c.appendChild(mark);
    c.appendChild(
      makeText(
        api,
        kind === "radio" ? "text" : "label",
        CAPTION_KINDS.has(kind) ? "Label" : sample,
      ),
    );
    return c;
  }
  const style = TEXT_STYLE[kind] ?? { font: FONT.regular, size: 14 };
  const text = makeText(api, "text", sample, style.font, style.size);
  const tone = values["tone"];
  text.fills = [
    kind === "link"
      ? paint(api, library, "color.action.primary", INK)
      : tone === "danger"
        ? paint(api, library, "color.action.danger", INK)
        : ink,
  ];
  if (kind === "menu-item" || kind === "option")
    bindNumber(c, ["paddingLeft", "paddingRight"], library, "space.sm", 8);
  c.appendChild(text);
  return c;
}
