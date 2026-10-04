// The Weft component library in a Penpot file: one page with a component per catalog kind (its
// variants combined when the kind has variant properties) and one design token set. Running it
// again finds what is there by plugin data and token name and adds only what is missing, so a
// designer's changes to existing components survive. What each component draws is shared with the
// other tools (`drawing` of @weft/design-tool); this file draws it with Penpot shapes.
import type { Token } from "@weft/catalog";
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
  tokenColor,
  tokenPx,
  variantAxes,
  variantName,
  type BoxDrawing,
  type Drawing,
  type KindEntry as SharedKindEntry,
  type Length,
  type Paint,
  type RGB,
  type RGBA,
  type TextDrawing,
} from "@weft/design-tool";
import type {
  PBoard,
  PCommonLayout,
  PenpotApi,
  PFill,
  PLibraryComponent,
  PPage,
  PShape,
  PShapeBase,
  PStroke,
  PText,
  PToken,
  PTokenType,
  PVariants,
} from "./api.ts";
import { childrenOf, isBoard } from "./api.ts";
import { dataOf, EMPTY_TEXT } from "./layer.ts";

export type KindEntry = SharedKindEntry<PLibraryComponent>;

/** A design token of the library's set, with the value Weft gave it. */
export type LibraryToken = { token: PToken; px?: number | undefined; color?: RGBA | undefined };

export type Library = {
  page: PPage;
  /** By Weft token path, which is also the Penpot token name. */
  tokens: Map<string, LibraryToken>;
  kinds: Map<string, KindEntry>;
};

/** Polls for a change Penpot applies after the call that asked for it returns. */
async function settle<T>(read: () => T | null | undefined, what: string): Promise<T> {
  for (let i = 0; i < 50; i++) {
    const value = read();
    if (value !== null && value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(`Penpot did not finish: ${what}`);
}

export async function ensureLibrary(
  api: PenpotApi,
  catalog: Catalog,
  tokens: ReadonlyMap<string, Token>,
): Promise<Library> {
  // With natural ordering, a flex board lists and appends children in the order the layout shows
  // them, which is the order Weft children have.
  api.flags.naturalChildOrdering = true;
  const original = api.currentPage;
  const tag = libraryTag(catalog);
  let page = api.currentFile?.pages.find((p) => readMark(dataOf(p), KEY.library) !== undefined);
  if (page === undefined) {
    page = api.createPage();
    page.name = LIBRARY_PAGE;
  }
  // Penpot refuses changes to a page that is not the active one.
  await api.openPage(page.id);
  try {
    dataOf(page).setPluginData(KEY.library, tag);
    const library: Library = { page, tokens: ensureTokens(api, tokens), kinds: new Map() };
    const board = libraryBoard(api, page, tag);
    for (const child of board.children) {
      const kind = readMark(dataOf(child), KEY.kind);
      if (kind === undefined || !Object.hasOwn(catalog.components, kind)) continue;
      const entry = existingKind(child, variantAxes(catalog.components[kind] as ComponentDef));
      if (entry !== undefined) library.kinds.set(kind, entry);
    }
    for (const [kind, def] of Object.entries(catalog.components)) {
      if (!library.kinds.has(kind))
        library.kinds.set(kind, await createKind(api, library, kind, def, board));
    }
    return library;
  } finally {
    if (original !== null && original.id !== page.id) await api.openPage(original.id);
  }
}

function libraryBoard(api: PenpotApi, page: PPage, tag: string): PBoard {
  const found = (childrenOf(page.root) ?? []).find(
    (s): s is PBoard => isBoard(s) && readMark(dataOf(s), KEY.library) !== undefined,
  );
  if (found !== undefined) return found;
  const board = api.createBoard();
  board.name = LIBRARY_BOARD;
  dataOf(board).setPluginData(KEY.library, tag);
  const flex = board.addFlexLayout();
  flex.dir = "row";
  flex.wrap = "wrap";
  flex.horizontalSizing = "auto";
  flex.verticalSizing = "auto";
  setGap(board, flex, 32);
  setPadding(flex, ["leftPadding", "rightPadding", "topPadding", "bottomPadding"], 32);
  board.fills = [];
  return board;
}

function existingKind(shape: PShape, axes: KindEntry["axes"]): KindEntry | undefined {
  if (isBoard(shape) && shape.isVariantContainer()) {
    const variants = new Map<string, PLibraryComponent>();
    let fallback: PLibraryComponent | undefined;
    for (const child of shape.children) {
      const component = child.component();
      if (component?.isVariant() !== true) continue;
      variants.set(variantName(component.variantProps, axes), component);
      fallback ??= component;
    }
    return fallback === undefined ? undefined : { axes, variants, fallback };
  }
  const component = shape.isComponentMainInstance() ? shape.component() : null;
  return component === null
    ? undefined
    : { axes, variants: new Map([["", component]]), fallback: component };
}

// --- tokens --------------------------------------------------------------------------------

function tokenValue(
  token: Token,
): { type: PTokenType; text: string; px?: number; color?: RGBA } | undefined {
  const px = tokenPx(token);
  if (px !== undefined)
    return { type: token.type === "number" ? "number" : "spacing", text: String(px), px };
  const color = tokenColor(token);
  return color === undefined ? undefined : { type: "color", text: cssColor(color), color };
}

const channel = (c: number) => Math.round(c * 255);

export const hexColor = (c: RGB): string =>
  `#${[c.r, c.g, c.b].map((v) => channel(v).toString(16).padStart(2, "0")).join("")}`;

const cssColor = (c: RGBA): string =>
  c.a >= 1 ? hexColor(c) : `rgba(${channel(c.r)}, ${channel(c.g)}, ${channel(c.b)}, ${c.a})`;

function ensureTokens(
  api: PenpotApi,
  tokens: ReadonlyMap<string, Token>,
): Map<string, LibraryToken> {
  const catalog = api.library.local.tokens;
  const set =
    catalog.sets.find((s) => s.name === TOKEN_COLLECTION) ??
    catalog.addSet({ name: TOKEN_COLLECTION, active: true });
  // Penpot resolves an applied token through the active sets only.
  if (!set.active) set.active = true;
  const byName = new Map(set.tokens.map((t) => [t.name, t]));
  const out = new Map<string, LibraryToken>();
  for (const [path, token] of tokens) {
    const value = tokenValue(token);
    if (value === undefined) continue;
    let made = byName.get(path);
    if (made !== undefined && made.type !== value.type) continue;
    if (made === undefined)
      made = set.addToken({ type: value.type, name: path, value: value.text });
    else if (made.value !== value.text) made.value = value.text;
    out.set(path, { token: made, px: value.px, color: value.color });
  }
  return out;
}

// --- drawing -------------------------------------------------------------------------------

type PaddingField = "leftPadding" | "rightPadding" | "topPadding" | "bottomPadding";

const TOKEN_FIELD: Readonly<Record<PaddingField, string>> = {
  leftPadding: "paddingLeft",
  rightPadding: "paddingRight",
  topPadding: "paddingTop",
  bottomPadding: "paddingBottom",
};

/** Sets padding fields, then applies the token when the library has it. */
export function setPadding(
  layout: PCommonLayout,
  fields: readonly PaddingField[],
  px: number,
  shape?: PShapeBase,
  token?: LibraryToken,
): void {
  for (const field of fields) layout[field] = token?.px ?? px;
  if (shape !== undefined && token !== undefined)
    shape.applyToken(
      token.token,
      fields.map((f) => TOKEN_FIELD[f]),
    );
}

/**
 * Sets the gap between children, then applies the token when the library has it. Both gaps are
 * set, as Penpot's UI does for a single gap value, so turning a column into a row keeps the
 * spacing the designer sees.
 */
export function setGap(
  board: PBoard,
  layout: PCommonLayout,
  px: number,
  token?: LibraryToken,
): void {
  layout.rowGap = token?.px ?? px;
  layout.columnGap = token?.px ?? px;
  if (token !== undefined) board.applyToken(token.token, ["rowGap", "columnGap"]);
}

const tokenOf = (library: Library, path: string | undefined) =>
  path === undefined ? undefined : library.tokens.get(path);

/** A solid fill for a paint, and the token to apply after it is set. */
function fillOf(
  library: Library,
  of: Paint,
): { color: string; opacity: number; token?: PToken | undefined } {
  if ("color" in of) return { color: hexColor(of.color), opacity: 1 };
  const token = library.tokens.get(of.token);
  const color = token?.color ?? { ...of.fallback, a: 1 };
  return { color: hexColor(color), opacity: color.a, token: token?.token };
}

export function paintFill(shape: PShapeBase, library: Library, of: Paint | undefined): void {
  if (of === undefined) {
    shape.fills = [];
    return;
  }
  const fill = fillOf(library, of);
  shape.fills = [{ fillColor: fill.color, fillOpacity: fill.opacity } satisfies PFill];
  if (fill.token !== undefined) shape.applyToken(fill.token, ["fill"]);
}

function paintStroke(shape: PShapeBase, library: Library, of: Paint): void {
  const stroke = fillOf(library, of);
  shape.strokes = [
    { strokeColor: stroke.color, strokeOpacity: stroke.opacity, strokeWidth: 1 } satisfies PStroke,
  ];
  if (stroke.token !== undefined) shape.applyToken(stroke.token, ["strokeColor"]);
}

export function drawText(api: PenpotApi, library: Library, d: TextDrawing): PText {
  const text = api.createText(d.characters === "" ? EMPTY_TEXT : d.characters);
  if (text === null) throw new Error("Penpot did not create a text layer");
  text.name = d.name;
  text.growType = "auto-width";
  text.fontSize = String(d.size);
  text.fontWeight = d.font === "bold" ? "700" : "400";
  if (d.fill !== undefined) paintFill(text, library, d.fill);
  return text;
}

const radiusOf = (library: Library, length: Length | undefined): number | undefined =>
  length === undefined ? undefined : (tokenOf(library, length.token)?.px ?? length.px);

function drawBox(api: PenpotApi, library: Library, d: BoxDrawing): PBoard {
  const box = api.createBoard();
  box.name = d.name;
  const flex = box.addFlexLayout();
  flex.dir = d.direction;
  flex.horizontalSizing = d.size === undefined ? "auto" : "fix";
  flex.verticalSizing = d.size === undefined ? "auto" : "fix";
  setGap(box, flex, d.gap);
  setPadding(flex, ["leftPadding", "rightPadding", "topPadding", "bottomPadding"], 0);
  if (d.padX !== undefined)
    setPadding(
      flex,
      ["leftPadding", "rightPadding"],
      d.padX.px,
      box,
      tokenOf(library, d.padX.token),
    );
  if (d.padY !== undefined)
    setPadding(
      flex,
      ["topPadding", "bottomPadding"],
      d.padY.px,
      box,
      tokenOf(library, d.padY.token),
    );
  // Weft's radius tokens are dimensions, which Penpot keeps as spacing tokens; a spacing token
  // cannot be applied to a radius, so the radius is the token's value.
  const radius = radiusOf(library, d.radius);
  if (radius !== undefined) box.borderRadius = radius;
  paintFill(box, library, d.fill);
  if (d.stroke !== undefined) paintStroke(box, library, d.stroke);
  if (d.size !== undefined) box.resize(d.size.width, d.size.height);
  for (const child of d.children) box.appendChild(draw(api, library, child));
  return box;
}

function draw(api: PenpotApi, library: Library, d: Drawing): PShape {
  if (d.type === "text") return drawText(api, library, d);
  if (d.type === "box") return drawBox(api, library, d);
  const rect = api.createRectangle();
  rect.name = d.name;
  rect.resize(d.width, d.height);
  if (d.radius !== undefined) rect.borderRadius = d.radius;
  paintFill(rect, library, d.fill);
  if (d.stroke !== undefined) paintStroke(rect, library, d.stroke);
  return rect;
}

// --- components ----------------------------------------------------------------------------

/** Gives the combined variants one property per axis, named after it. */
function matchProperties(variants: PVariants, axes: KindEntry["axes"]): void {
  while (variants.properties.length < axes.length) variants.addProperty();
  while (variants.properties.length > axes.length)
    variants.removeProperty(variants.properties.length - 1);
  axes.forEach((axis, i) => {
    if (variants.properties[i] !== axis.name) variants.renameProperty(i, axis.name);
  });
}

async function createKind(
  api: PenpotApi,
  library: Library,
  kind: string,
  def: ComponentDef,
  board: PBoard,
): Promise<KindEntry> {
  const axes = variantAxes(def);
  const made = combinations(axes).map((values) => {
    const root = drawBox(api, library, drawing(kind, def, values));
    root.name = axes.length === 0 ? kind : variantName(values, axes);
    board.appendChild(root);
    const component = api.library.local.createComponent([root]);
    dataOf(component.mainInstance()).setPluginData(KEY.kind, kind);
    return { values, component };
  });
  const first = made[0];
  if (first === undefined) throw new Error(`no variants for ${kind}`);
  if (axes.length === 0)
    return { axes, variants: new Map([["", first.component]]), fallback: first.component };

  const others = made.slice(1).map((m) => m.component.mainInstance().id);
  const container = first.component.mainInstance().combineAsVariants(others);
  const variants = await settle(() => container.variants, `the variants of ${kind}`);
  container.name = kind;
  dataOf(container).setPluginData(KEY.kind, kind);
  // The library is found again by the board's children, wherever Penpot put the container.
  board.appendChild(container);
  matchProperties(variants, axes);
  const entries = new Map<string, PLibraryComponent>();
  for (const { values, component } of made) {
    if (!component.isVariant()) throw new Error(`Penpot did not make ${kind} a variant`);
    axes.forEach((axis, i) => component.setVariantProperty(i, values[axis.name] ?? axis.fallback));
    entries.set(variantName(values, axes), component);
  }
  return { axes, variants: entries, fallback: first.component };
}
