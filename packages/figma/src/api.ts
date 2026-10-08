// The subset of the Figma Plugin API the conversions use. Every function here takes this narrow
// interface instead of the global `figma`, so the same code runs in the plugin and against the
// in-memory fake of the tests. `test/api-types.test.ts` checks at compile time that the official
// types (`@figma/plugin-typings`) satisfy it, so the subset cannot drift from the real API.
import type {
  Effect,
  Paint,
  SceneNode,
  VariableResolvedDataType,
} from "@figma/plugin-typings/plugin-api-standalone.js";

/** `figma.mixed`: a property that differs across a text range or across corners. */
export type FMixed = symbol;

export type FRGB = { readonly r: number; readonly g: number; readonly b: number };
export type FRGBA = FRGB & { readonly a: number };
export type FAlias = { readonly type: "VARIABLE_ALIAS"; readonly id: string };
export type FFont = { readonly family: string; readonly style: string };

export type FSolid = {
  readonly type: "SOLID";
  readonly color: FRGB;
  readonly opacity?: number | undefined;
  readonly boundVariables?: { readonly color?: FAlias | undefined } | undefined;
};
/** A background blur, the one effect Weft draws (a `material` token); others are only read. */
export type FEffect =
  | { readonly type: "BACKGROUND_BLUR"; readonly radius: number; readonly visible: boolean }
  | { readonly type: Exclude<Effect["type"], "BACKGROUND_BLUR"> };

export type FPaint = FSolid | { readonly type: Exclude<Paint["type"], "SOLID"> };

/** Node fields a variable can be bound to that this package binds. */
export type FBindable =
  | "itemSpacing"
  | "counterAxisSpacing"
  | "gridRowGap"
  | "gridColumnGap"
  | "paddingLeft"
  | "paddingRight"
  | "paddingTop"
  | "paddingBottom"
  | "topLeftRadius"
  | "topRightRadius"
  | "bottomLeftRadius"
  | "bottomRightRadius";

/** Shared plugin data, reached only through `dataOf` (`data.ts`); private plugin data is not used. */
export interface FPluginData {
  getSharedPluginData(namespace: string, key: string): string;
  setSharedPluginData(namespace: string, key: string, value: string): void;
}

export interface FBase extends FPluginData {
  readonly id: string;
  name: string;
  remove(): void;
}

export interface FScene extends FBase {
  visible: boolean;
  x: number;
  y: number;
  readonly width: number;
  readonly height: number;
}

export interface FParent {
  readonly children: readonly FNode[];
  appendChild(child: FNode): void;
  insertChild(index: number, child: FNode): void;
}

/** Auto layout, fills and corners: frames, components, component sets and instances. */
export interface FLayout extends FScene, FParent {
  /** Degrees counterclockwise (the Plugin API reads and writes -180 to 180). */
  rotation: number;
  layoutMode: "NONE" | "HORIZONTAL" | "VERTICAL" | "GRID";
  layoutWrap: "NO_WRAP" | "WRAP";
  itemSpacing: number;
  counterAxisSpacing: number | null;
  primaryAxisAlignItems:
    | "MIN"
    | "MAX"
    | "CENTER"
    | "SPACE_BETWEEN"
    | "SPACE_EVENLY"
    | "SPACE_AROUND";
  counterAxisAlignItems: "MIN" | "MAX" | "CENTER" | "BASELINE";
  primaryAxisSizingMode: "FIXED" | "AUTO";
  counterAxisSizingMode: "FIXED" | "AUTO";
  paddingLeft: number;
  paddingRight: number;
  paddingTop: number;
  paddingBottom: number;
  gridColumnCount: number;
  gridRowGap: number;
  gridColumnGap: number;
  fills: readonly FPaint[] | FMixed;
  strokes: readonly FPaint[];
  effects: readonly FEffect[];
  strokeWeight: number | FMixed;
  cornerRadius: number | FMixed;
  resize(width: number, height: number): void;
  readonly boundVariables?: { readonly [field in FBindable]?: FAlias | undefined } | undefined;
  setBoundVariable(field: FBindable, variable: FVariable | null): void;
}

export interface FFrame extends FLayout {
  readonly type: "FRAME";
}

export interface FComponent extends FLayout {
  readonly type: "COMPONENT";
  createInstance(): FInstance;
  readonly variantProperties: { readonly [property: string]: string } | null;
}

export interface FComponentSet extends FLayout {
  readonly type: "COMPONENT_SET";
  readonly defaultVariant: FComponent;
}

export type FComponentProperties = {
  readonly [name: string]: { readonly type: string; readonly value: string | boolean };
};

export interface FInstance extends FLayout {
  readonly type: "INSTANCE";
  getMainComponentAsync(): Promise<FComponent | null>;
  readonly componentProperties: FComponentProperties;
  setProperties(properties: { [name: string]: string | boolean }): void;
}

export interface FText extends FScene {
  readonly type: "TEXT";
  rotation: number;
  characters: string;
  fontName: FFont | FMixed;
  fontSize: number | FMixed;
  fills: readonly FPaint[] | FMixed;
}

export interface FRectangle extends FScene {
  readonly type: "RECTANGLE";
  fills: readonly FPaint[] | FMixed;
  strokes: readonly FPaint[];
  cornerRadius: number | FMixed;
  resize(width: number, height: number): void;
}

/** Every other layer: groups, vectors, shapes, slices and the rest. Read only, for the loss table. */
export interface FOther extends FScene {
  readonly type: Exclude<
    SceneNode["type"],
    "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" | "TEXT" | "RECTANGLE"
  >;
  /** Groups and boolean operations hold layers; shapes do not. */
  readonly children?: readonly FNode[] | undefined;
  readonly fills?: readonly FPaint[] | FMixed | undefined;
}

export type FNode = FFrame | FComponent | FComponentSet | FInstance | FText | FRectangle | FOther;

export interface FPage extends FBase, FParent {
  readonly type: "PAGE";
  loadAsync(): Promise<void>;
}

export interface FVariable extends FPluginData {
  readonly id: string;
  name: string;
  readonly resolvedType: VariableResolvedDataType;
  readonly variableCollectionId: string;
  readonly valuesByMode: { readonly [modeId: string]: unknown };
  setValueForMode(modeId: string, value: number | FRGBA): void;
}

export interface FCollection extends FPluginData {
  readonly id: string;
  name: string;
  readonly defaultModeId: string;
  readonly modes: ReadonlyArray<{ readonly modeId: string; readonly name: string }>;
  /** Throws `in addMode: Limited to N modes only` when the file's plan allows no more modes. */
  addMode(name: string): string;
  renameMode(modeId: string, newName: string): void;
}

export interface FVariables {
  createVariableCollection(name: string): FCollection;
  createVariable(name: string, collection: FCollection, type: FVariable["resolvedType"]): FVariable;
  getLocalVariableCollectionsAsync(): Promise<FCollection[]>;
  getLocalVariablesAsync(): Promise<FVariable[]>;
  getVariableByIdAsync(id: string): Promise<FVariable | null>;
  setBoundVariableForPaint(paint: FSolid, field: "color", variable: FVariable | null): FSolid;
}

export interface FigmaApi {
  readonly mixed: FMixed;
  readonly root: { readonly children: readonly FPage[] };
  readonly currentPage: FPage;
  readonly variables: FVariables;
  createPage(): FPage;
  createFrame(): FFrame;
  createComponent(): FComponent;
  createText(): FText;
  createRectangle(): FRectangle;
  combineAsVariants(nodes: readonly FComponent[], parent: FParent, index?: number): FComponentSet;
  loadFontAsync(font: FFont): Promise<void>;
}

/** The layers that hold auto layout and children. */
export type FContainer = FFrame | FComponent | FComponentSet | FInstance;

export function isContainer(node: FNode): node is FContainer {
  return (
    node.type === "FRAME" ||
    node.type === "COMPONENT" ||
    node.type === "COMPONENT_SET" ||
    node.type === "INSTANCE"
  );
}
