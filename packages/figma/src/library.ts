// The Weft component library in a Figma file: one page with a component set per catalog kind
// (a single component when the kind has no variant properties) and one variable collection with
// the design tokens. Running it again finds what is there by plugin data and adds only what is
// missing, so a designer's changes to existing components survive. What each component draws is
// shared with the other tools (`drawing` of @weft/design-tool); this file draws it with Figma nodes.
import type { Token, TokenModifier } from "@weft/catalog";
import type { Catalog, ComponentDef } from "@weft/core";
import {
  combinations,
  drawing,
  KEY,
  LIBRARY_BOARD,
  LIBRARY_PAGE,
  libraryTag,
  readMark,
  TOKEN_COLLECTION,
  variantAxes,
  variantName,
  type BoxDrawing,
  type Drawing,
  type KindEntry as SharedKindEntry,
  type Length,
  type Paint,
  type TextDrawing,
} from "@weft/design-tool";
import type {
  FBindable,
  FCollection,
  FComponent,
  FFont,
  FFrame,
  FigmaApi,
  FLayout,
  FNode,
  FPage,
  FSolid,
  FText,
  FVariable,
} from "./api.ts";
import { variableValue, writeModes } from "./modes.ts";
import { variableName } from "./tokens.ts";

export type KindEntry = SharedKindEntry<FComponent>;

export type Library = {
  page: FPage;
  variables: Map<string, FVariable>;
  kinds: Map<string, KindEntry>;
  /** What the build skipped without failing, such as modes the file's plan does not allow. */
  notes: string[];
};

export const FONT: { regular: FFont; bold: FFont } = {
  regular: { family: "Inter", style: "Regular" },
  bold: { family: "Inter", style: "Bold" },
};

export async function loadFonts(api: FigmaApi): Promise<void> {
  await Promise.all([api.loadFontAsync(FONT.regular), api.loadFontAsync(FONT.bold)]);
}

export async function ensureLibrary(
  api: FigmaApi,
  catalog: Catalog,
  tokens: ReadonlyMap<string, Token>,
  modifier?: TokenModifier,
): Promise<Library> {
  await loadFonts(api);
  const tag = libraryTag(catalog);
  let page = api.root.children.find((p) => readMark(p, KEY.library) !== undefined);
  if (page === undefined) {
    page = api.createPage();
    page.name = LIBRARY_PAGE;
  }
  page.setPluginData(KEY.library, tag);
  await page.loadAsync();
  const notes: string[] = [];
  const library: Library = {
    page,
    variables: await ensureVariables(api, tokens, modifier, notes),
    kinds: new Map(),
    notes,
  };

  let board = page.children.find(
    (n): n is FFrame => n.type === "FRAME" && readMark(n, KEY.library) !== undefined,
  );
  if (board === undefined) {
    board = api.createFrame();
    board.name = LIBRARY_BOARD;
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
  modifier: TokenModifier | undefined,
  notes: string[],
): Promise<Map<string, FVariable>> {
  const collections = await api.variables.getLocalVariableCollectionsAsync();
  let collection: FCollection | undefined = collections.find(
    (c) => readMark(c, KEY.library) !== undefined,
  );
  if (collection === undefined) {
    collection = api.variables.createVariableCollection(TOKEN_COLLECTION);
    collection.setPluginData(KEY.library, TOKEN_COLLECTION);
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
  if (modifier !== undefined) writeModes(collection, variables, modifier, notes);
  return variables;
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
    const c = api.createComponent();
    drawBox(api, library, c, drawing(kind, def, values));
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

/** A solid paint, bound to the token's variable when the library has it. */
export function paint(api: FigmaApi, library: Library, of: Paint): FSolid {
  if ("color" in of) return { type: "SOLID", color: of.color };
  const solid: FSolid = { type: "SOLID", color: of.fallback };
  const variable = library.variables.get(of.token);
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

export function drawText(api: FigmaApi, library: Library, d: TextDrawing): FText {
  const text = makeText(api, d.name, d.characters, FONT[d.font], d.size);
  if (d.fill !== undefined) text.fills = [paint(api, library, d.fill)];
  return text;
}

const bindLength = (
  layer: FLayout,
  fields: readonly FBindable[],
  library: Library,
  length: Length | undefined,
) => {
  if (length !== undefined) bindNumber(layer, fields, library, length.token, length.px);
};

function drawBox(api: FigmaApi, library: Library, box: FLayout, d: BoxDrawing): void {
  box.name = d.name;
  autoLayout(box, d.direction === "row" ? "HORIZONTAL" : "VERTICAL", d.gap, 0);
  box.fills = d.fill === undefined ? [] : [paint(api, library, d.fill)];
  bindLength(box, ["paddingLeft", "paddingRight"], library, d.padX);
  bindLength(box, ["paddingTop", "paddingBottom"], library, d.padY);
  bindLength(box, CORNERS, library, d.radius);
  if (d.stroke !== undefined) box.strokes = [paint(api, library, d.stroke)];
  if (d.size !== undefined) {
    box.resize(d.size.width, d.size.height);
    box.primaryAxisSizingMode = "FIXED";
    box.counterAxisSizingMode = "FIXED";
  }
  for (const child of d.children) box.appendChild(draw(api, library, child));
}

function draw(api: FigmaApi, library: Library, d: Drawing): FNode {
  if (d.type === "text") return drawText(api, library, d);
  if (d.type === "box") {
    const frame = api.createFrame();
    drawBox(api, library, frame, d);
    return frame;
  }
  const rect = api.createRectangle();
  rect.name = d.name;
  rect.resize(d.width, d.height);
  if (d.radius !== undefined) rect.cornerRadius = d.radius;
  rect.fills = [paint(api, library, d.fill)];
  if (d.stroke !== undefined) rect.strokes = [paint(api, library, d.stroke)];
  return rect;
}
