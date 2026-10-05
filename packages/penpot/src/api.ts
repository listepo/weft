// The subset of Penpot's plugin API the conversions use. Every function here takes this narrow
// interface instead of the global `penpot`, so the same code runs in the plugin and against the
// in-memory fake of the tests. `test/api-types.test.ts` checks at compile time that the official
// types (`@penpot/plugin-types`) satisfy it, so the subset cannot drift from the real API.

/** Plugin data shared between plugins under a namespace. Penpot returns nothing for a missing key. */
export interface PSharedData {
  getSharedPluginData(namespace: string, key: string): string;
  setSharedPluginData(namespace: string, key: string, value: string): void;
}

export type PFill = {
  fillColor?: string;
  fillOpacity?: number;
  fillColorGradient?: unknown;
  fillImage?: unknown;
};

export type PStroke = {
  strokeColor?: string;
  strokeOpacity?: number;
  strokeWidth?: number;
};

export type PTrack = { readonly type: string; readonly value: number | null };

export interface PCommonLayout {
  alignItems?: "start" | "end" | "center" | "stretch";
  rowGap: number;
  columnGap: number;
  topPadding: number;
  rightPadding: number;
  bottomPadding: number;
  leftPadding: number;
  horizontalSizing: "fix" | "fill" | "auto";
  verticalSizing: "fix" | "fill" | "auto";
}

export interface PFlex extends PCommonLayout {
  dir: "row" | "row-reverse" | "column" | "column-reverse";
  wrap?: "wrap" | "nowrap";
}

export interface PGrid extends PCommonLayout {
  readonly columns: readonly PTrack[];
  readonly rows: readonly PTrack[];
  addRow(type: "flex" | "fixed" | "percent" | "auto", value?: number): void;
  addColumn(type: "flex" | "fixed" | "percent" | "auto", value?: number): void;
  removeRow(index: number): void;
  removeColumn(index: number): void;
  /** Cells count from 1 (Penpot's grid layout data); unverified for this call. */
  appendChild(child: PShape, row: number, column: number): void;
}

export interface PToken {
  readonly id: string;
  name: string;
  readonly type: string;
  /** A string for the token types Weft writes (`color`, `spacing`, `number`). */
  value: unknown;
}

export type PTokenType = "color" | "spacing" | "number";

export interface PTokenSet {
  readonly id: string;
  name: string;
  active: boolean;
  readonly tokens: readonly PToken[];
  addToken(token: { type: PTokenType; name: string; value: string }): PToken;
}

/**
 * A preset of active sets. Only one theme of a group is active at a time, so a group is a resolver
 * modifier and its themes the contexts; activating a set directly turns every theme off.
 */
export interface PTokenTheme {
  readonly id: string;
  group: string;
  name: string;
  active: boolean;
  toggleActive(): void;
  readonly activeSets: readonly PTokenSet[];
  /** Takes the set's id: the official type also takes the set, which the subset cannot name. */
  addSet(setId: string): void;
}

export interface PTokenCatalog {
  /** In creation order. */
  readonly themes: readonly PTokenTheme[];
  /** In the user's order: among active sets with the same token name, the later one wins. */
  readonly sets: readonly PTokenSet[];
  addTheme(theme: { group: string; name: string }): PTokenTheme;
  addSet(set: { name: string; active?: boolean }): PTokenSet;
}

/** A blur, as Penpot types it (`Blur`): its intensity, and whether it is hidden. */
export type PBlur = { id?: string | undefined; value?: number | undefined; hidden?: boolean };

export interface PShapeBase extends PSharedData {
  readonly id: string;
  name: string;
  x: number;
  y: number;
  readonly width: number;
  readonly height: number;
  hidden: boolean;
  fills: PFill[] | "mixed";
  strokes: PStroke[];
  borderRadius: number;
  /** The blur of what is behind the shape, which Weft draws for a `material` token. */
  backgroundBlur?: PBlur | undefined;
  readonly layoutCell?: { readonly row?: number; readonly column?: number } | undefined;
  /** Token names applied to the shape, by property (`rowGap`, `fill`). */
  readonly tokens: { readonly [property: string]: string | undefined };
  applyToken(token: PToken, properties?: string[]): void;
  resize(width: number, height: number): void;
  remove(): void;
  isComponentMainInstance(): boolean;
  isComponentCopyInstance(): boolean;
  isComponentHead(): boolean;
  component(): PLibraryComponent | null;
  switchVariant(pos: number, value: string): void;
  /** On a component's main board: combines it with the main boards of `ids` into variants. */
  combineAsVariants(ids: string[]): PVariantContainer;
}

export interface PBoard extends PShapeBase {
  readonly type: "board";
  readonly flex?: PFlex | undefined;
  readonly grid?: PGrid | undefined;
  fills: PFill[];
  readonly children: readonly PShape[];
  appendChild(child: PShape): void;
  insertChild(index: number, child: PShape): void;
  addFlexLayout(): PFlex;
  addGridLayout(): PGrid;
  isVariantContainer(): boolean;
}

export interface PVariantContainer extends PBoard {
  readonly variants: PVariants | null;
}

export interface PText extends PShapeBase {
  readonly type: "text";
  characters: string;
  fontSize: string;
  fontWeight: string;
  growType: "fixed" | "auto-width" | "auto-height";
}

/** Groups and boolean operations: they hold layers but have no layout. */
export interface PGroup extends PShapeBase {
  readonly type: "group" | "boolean";
  readonly children: readonly PShape[];
}

/** Every other shape: read only, for the loss table. */
export interface POther extends PShapeBase {
  readonly type: "rectangle" | "ellipse" | "path" | "svg-raw" | "image";
}

export type PShape = PBoard | PText | PGroup | POther;

export interface PRectangle extends PShapeBase {
  readonly type: "rectangle";
}

export interface PLibraryComponent {
  readonly id: string;
  name: string;
  instance(): PShape;
  mainInstance(): PShape;
  isVariant(): this is PVariantComponent;
}

export interface PVariantComponent extends PLibraryComponent {
  readonly variants: PVariants | null;
  readonly variantProps: { readonly [property: string]: string };
  setVariantProperty(pos: number, value: string): void;
}

export interface PVariants {
  readonly properties: readonly string[];
  addProperty(): void;
  removeProperty(pos: number): void;
  renameProperty(pos: number, name: string): void;
  variantComponents(): PLibraryComponent[];
}

export interface PLibrary extends PSharedData {
  readonly components: readonly PLibraryComponent[];
  readonly tokens: PTokenCatalog;
  createComponent(shapes: PShape[]): PLibraryComponent;
}

export interface PPage extends PSharedData {
  readonly id: string;
  name: string;
  /** The page's root board: its children are the page's top-level shapes. */
  readonly root: PShape;
}

export interface PenpotApi {
  readonly flags: { naturalChildOrdering: boolean };
  readonly currentPage: PPage | null;
  readonly currentFile: { readonly pages: readonly PPage[] } | null;
  readonly library: { readonly local: PLibrary };
  createPage(): PPage;
  /** Takes the page's id: `penpot` maps its methods to properties, so parameters must widen. */
  openPage(page: string, newWindow?: boolean): Promise<void>;
  createBoard(): PBoard;
  createRectangle(): PRectangle;
  createText(text: string): PText | null;
}

export const isBoard = (shape: PShape): shape is PBoard => shape.type === "board";

/** The children of a shape that can hold any. */
export const childrenOf = (shape: PShape): readonly PShape[] | undefined =>
  "children" in shape ? shape.children : undefined;
