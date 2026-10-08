// An in-memory fake of the Plugin API subset in `src/api.ts`, with the behaviour the conversions
// rely on: plugin data per layer, child order, auto layout fields, variables and bindings,
// component sets and instances with variant properties, and fonts that must be loaded before
// text changes (the real API throws there too). Test-only helpers end in `For` or start with `find`.
import type {
  FAlias,
  FBindable,
  FCollection,
  FComponent,
  FComponentProperties,
  FComponentSet,
  FEffect,
  FFont,
  FFrame,
  FigmaApi,
  FInstance,
  FMixed,
  FNode,
  FPage,
  FPaint,
  FRectangle,
  FRGBA,
  FSolid,
  FText,
  FVariable,
  FVariables,
} from "../src/api.ts";

let nextId = 1;
const newId = () => `1:${nextId++}`;

type Owner = { children: FakeNode[] };

export abstract class FakeBase {
  id = newId();
  name = "";
  visible = true;
  x = 0;
  y = 0;
  rotation = 0;
  width = 100;
  height = 100;
  /** Set only when a test places the layer in a parent grid; absent leaves z-order. */
  gridRowAnchorIndex?: number;
  gridColumnAnchorIndex?: number;
  layoutPositioning?: "AUTO" | "ABSOLUTE";
  parent: Owner | undefined;
  /** Shared plugin data, by namespace; the package uses no private plugin data. */
  readonly shared = new Map<string, Map<string, string>>();

  getSharedPluginData(namespace: string, key: string): string {
    return this.shared.get(namespace)?.get(key) ?? "";
  }
  setSharedPluginData(namespace: string, key: string, value: string): void {
    // The real API refuses a namespace shorter than 3 alphanumeric characters.
    if (!/^[A-Za-z0-9]{3,}$/.test(namespace)) throw new Error(`bad namespace ${namespace}`);
    const entries = this.shared.get(namespace) ?? new Map<string, string>();
    if (value === "") entries.delete(key);
    else entries.set(key, value);
    if (entries.size === 0) this.shared.delete(namespace);
    else this.shared.set(namespace, entries);
  }
  remove(): void {
    if (this.parent === undefined) return;
    const list = this.parent.children;
    list.splice(list.indexOf(this as unknown as FakeNode), 1);
    this.parent = undefined;
  }
  protected copyBase(to: FakeBase): void {
    to.name = this.name;
    to.visible = this.visible;
    to.x = this.x;
    to.y = this.y;
    to.rotation = this.rotation;
    to.width = this.width;
    to.height = this.height;
    if (this.gridRowAnchorIndex !== undefined) to.gridRowAnchorIndex = this.gridRowAnchorIndex;
    if (this.gridColumnAnchorIndex !== undefined)
      to.gridColumnAnchorIndex = this.gridColumnAnchorIndex;
    if (this.layoutPositioning !== undefined) to.layoutPositioning = this.layoutPositioning;
    for (const [ns, entries] of this.shared) to.shared.set(ns, new Map(entries));
  }
}

function attach(owner: Owner & object, child: FNode, index?: number): void {
  const node = child as FakeNode;
  node.remove();
  const at = index === undefined ? owner.children.length : Math.min(index, owner.children.length);
  owner.children.splice(at, 0, node);
  node.parent = owner;
}

export class FakeText extends FakeBase implements FText {
  readonly type = "TEXT";
  private chars = "";
  private font: FFont = { family: "Inter", style: "Regular" };
  private size = 12;
  fills: readonly FPaint[] | FMixed = [];
  private readonly figma: FakeFigma;
  constructor(figma: FakeFigma) {
    super();
    this.figma = figma;
  }
  get characters(): string {
    return this.chars;
  }
  set characters(value: string) {
    this.figma.requireFont(this.font);
    this.chars = value;
  }
  get fontName(): FFont {
    return this.font;
  }
  set fontName(value: FFont | FMixed) {
    if (typeof value === "symbol") throw new Error("cannot set mixed");
    this.figma.requireFont(value);
    this.font = value;
  }
  get fontSize(): number {
    return this.size;
  }
  set fontSize(value: number | FMixed) {
    if (typeof value === "symbol") throw new Error("cannot set mixed");
    this.figma.requireFont(this.font);
    this.size = value;
  }
  clone(): FakeText {
    const c = new FakeText(this.figma);
    this.copyBase(c);
    c.font = this.font;
    c.size = this.size;
    c.chars = this.chars;
    c.fills = this.fills;
    return c;
  }
}

export class FakeRectangle extends FakeBase implements FRectangle {
  readonly type = "RECTANGLE";
  fills: readonly FPaint[] | FMixed = [];
  strokes: readonly FPaint[] = [];
  cornerRadius: number | FMixed = 0;
  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }
  clone(): FakeRectangle {
    const c = new FakeRectangle();
    this.copyBase(c);
    c.fills = this.fills;
    c.strokes = this.strokes;
    c.cornerRadius = this.cornerRadius;
    return c;
  }
}

/** Any other layer (vector, ellipse, group …), for foreign-layer tests. */
export class FakeOther extends FakeBase {
  children: FakeNode[] | undefined;
  fills: readonly FPaint[] = [];
  readonly type: "GROUP" | "VECTOR" | "ELLIPSE";
  constructor(type: "GROUP" | "VECTOR" | "ELLIPSE", children?: FNode[]) {
    super();
    this.type = type;
    if (children !== undefined) {
      this.children = [];
      for (const c of children) attach(this as Owner, c);
    }
  }
  clone(): FakeOther {
    const c = new FakeOther(
      this.type,
      this.children?.map((n) => n.clone()),
    );
    this.copyBase(c);
    return c;
  }
}

abstract class FakeLayout extends FakeBase {
  children: FakeNode[] = [];
  layoutMode: "NONE" | "HORIZONTAL" | "VERTICAL" | "GRID" = "NONE";
  layoutWrap: "NO_WRAP" | "WRAP" = "NO_WRAP";
  itemSpacing = 0;
  counterAxisSpacing: number | null = null;
  primaryAxisAlignItems: FFrame["primaryAxisAlignItems"] = "MIN";
  counterAxisAlignItems: FFrame["counterAxisAlignItems"] = "MIN";
  primaryAxisSizingMode: "FIXED" | "AUTO" = "FIXED";
  counterAxisSizingMode: "FIXED" | "AUTO" = "FIXED";
  paddingLeft = 0;
  paddingRight = 0;
  paddingTop = 0;
  paddingBottom = 0;
  gridColumnCount = 1;
  gridRowGap = 0;
  gridColumnGap = 0;
  fills: readonly FPaint[] | FMixed = [{ type: "SOLID", color: { r: 1, g: 1, b: 1 } }];
  strokes: readonly FPaint[] = [];
  effects: readonly FEffect[] = [];
  strokeWeight: number | FMixed = 1;
  cornerRadius: number | FMixed = 0;
  bound: { [field in FBindable]?: FAlias } = {};
  protected readonly figma: FakeFigma;
  constructor(figma: FakeFigma) {
    super();
    this.figma = figma;
  }
  get boundVariables(): { readonly [field in FBindable]?: FAlias } {
    return { ...this.bound };
  }
  appendChild(child: FNode): void {
    attach(this, child);
  }
  insertChild(index: number, child: FNode): void {
    attach(this, child, index);
  }
  clearChildren(): void {
    for (const child of this.children.splice(0)) child.parent = undefined;
  }
  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }
  setBoundVariable(field: FBindable, variable: FVariable | null): void {
    if (variable === null) {
      delete this.bound[field];
      return;
    }
    this.bound[field] = { type: "VARIABLE_ALIAS", id: variable.id };
    const value = Object.values(variable.valuesByMode)[0];
    if (typeof value !== "number") throw new Error(`${field} needs a FLOAT variable`);
    if (field.endsWith("Radius")) this.cornerRadius = value;
    else (this as unknown as Record<string, number>)[field] = value;
  }
  copyLayout(to: FakeLayout): void {
    this.copyBase(to);
    for (const key of [
      "layoutMode",
      "layoutWrap",
      "itemSpacing",
      "counterAxisSpacing",
      "primaryAxisAlignItems",
      "counterAxisAlignItems",
      "primaryAxisSizingMode",
      "counterAxisSizingMode",
      "paddingLeft",
      "paddingRight",
      "paddingTop",
      "paddingBottom",
      "gridColumnCount",
      "gridRowGap",
      "gridColumnGap",
      "fills",
      "strokes",
      "effects",
      "strokeWeight",
      "cornerRadius",
    ] as const)
      (to as unknown as Record<string, unknown>)[key] = this[key];
    to.bound = { ...this.bound };
    for (const child of this.children) to.appendChild(child.clone());
  }
  abstract clone(): FakeNode;
}

export class FakeFrame extends FakeLayout implements FFrame {
  readonly type = "FRAME";
  clone(): FakeFrame {
    const c = new FakeFrame(this.figma);
    this.copyLayout(c);
    return c;
  }
}

export class FakeComponent extends FakeLayout implements FComponent {
  readonly type = "COMPONENT";
  get variantProperties(): Record<string, string> | null {
    if (!(this.parent instanceof FakeComponentSet)) return null;
    return Object.fromEntries(
      this.name.split(", ").map((pair) => pair.split("=") as [string, string]),
    );
  }
  createInstance(): FakeInstance {
    return new FakeInstance(this.figma, this);
  }
  clone(): FakeComponent {
    const c = new FakeComponent(this.figma);
    this.copyLayout(c);
    return c;
  }
}

export class FakeComponentSet extends FakeLayout implements FComponentSet {
  readonly type = "COMPONENT_SET";
  get defaultVariant(): FakeComponent {
    return this.children[0] as FakeComponent;
  }
  clone(): FakeComponentSet {
    const c = new FakeComponentSet(this.figma);
    this.copyLayout(c);
    return c;
  }
}

export class FakeInstance extends FakeLayout implements FInstance {
  readonly type = "INSTANCE";
  main: FakeComponent;
  constructor(figma: FakeFigma, main: FakeComponent) {
    super(figma);
    this.main = main;
    this.adopt(main);
  }
  /** Takes the main component's layout and layers; text a designer changed is kept by name. */
  private adopt(main: FakeComponent): void {
    const texts = new Map(
      this.children.flatMap((c) =>
        c instanceof FakeText ? [[c.name, c.characters] as const] : [],
      ),
    );
    const shared = new Map(this.shared);
    this.clearChildren();
    main.copyLayout(this);
    this.shared.clear();
    for (const [ns, entries] of shared) this.shared.set(ns, entries);
    for (const child of this.children)
      if (child instanceof FakeText && texts.has(child.name)) {
        this.figma.requireFont(child.fontName);
        child.characters = texts.get(child.name) as string;
      }
    this.main = main;
  }
  async getMainComponentAsync(): Promise<FakeComponent> {
    return this.main;
  }
  get componentProperties(): FComponentProperties {
    const values = this.main.variantProperties ?? {};
    return Object.fromEntries(
      Object.entries(values).map(([k, v]) => [k, { type: "VARIANT", value: v }]),
    );
  }
  setProperties(properties: { [name: string]: string | boolean }): void {
    const set = this.main.parent;
    if (!(set instanceof FakeComponentSet)) throw new Error("not a variant");
    const wanted = { ...this.main.variantProperties, ...properties };
    const target = set.children.find(
      (c): c is FakeComponent =>
        c instanceof FakeComponent &&
        Object.entries(wanted).every(([k, v]) => c.variantProperties?.[k] === v),
    );
    if (target === undefined) throw new Error(`no variant ${JSON.stringify(wanted)}`);
    this.adopt(target);
  }
  clone(): FakeInstance {
    const c = new FakeInstance(this.figma, this.main);
    c.clearChildren();
    this.copyLayout(c);
    return c;
  }
}

export type FakeNode =
  | FakeFrame
  | FakeComponent
  | FakeComponentSet
  | FakeInstance
  | FakeText
  | FakeRectangle
  | FakeOther;

export class FakePage extends FakeBase implements FPage {
  readonly type = "PAGE";
  children: FakeNode[] = [];
  appendChild(child: FNode): void {
    attach(this, child);
  }
  insertChild(index: number, child: FNode): void {
    attach(this, child, index);
  }
  async loadAsync(): Promise<void> {}
}

class FakeVariable extends FakeBase implements FVariable {
  valuesByMode: Record<string, unknown> = {};
  readonly variableCollectionId: string;
  readonly resolvedType: FVariable["resolvedType"];
  constructor(name: string, collectionId: string, type: FVariable["resolvedType"]) {
    super();
    this.name = name;
    this.variableCollectionId = collectionId;
    this.resolvedType = type;
  }
  setValueForMode(modeId: string, value: number | FRGBA): void {
    this.valuesByMode[modeId] = value;
  }
}

class FakeCollection extends FakeBase implements FCollection {
  readonly defaultModeId = "mode:1";
  readonly modes: { modeId: string; name: string }[] = [{ modeId: "mode:1", name: "Mode 1" }];
  /** Figma's Starter plan allows one mode; paid plans allow more (`Infinity` here). */
  private readonly modeLimit: number;
  constructor(modeLimit: number) {
    super();
    this.modeLimit = modeLimit;
  }
  addMode(name: string): string {
    if (this.modes.length >= this.modeLimit)
      throw new Error(`in addMode: Limited to ${this.modeLimit} modes only`);
    const modeId = `mode:${this.modes.length + 1}`;
    this.modes.push({ modeId, name });
    return modeId;
  }
  renameMode(modeId: string, newName: string): void {
    const mode = this.modes.find((m) => m.modeId === modeId);
    if (mode === undefined) throw new Error(`no mode ${modeId}`);
    mode.name = newName;
  }
}

class FakeVariables implements FVariables {
  readonly collections: FakeCollection[] = [];
  readonly all: FakeVariable[] = [];
  modeLimit = Number.POSITIVE_INFINITY;
  createVariableCollection(name: string): FakeCollection {
    const c = new FakeCollection(this.modeLimit);
    c.name = name;
    this.collections.push(c);
    return c;
  }
  createVariable(
    name: string,
    collection: FCollection,
    type: FVariable["resolvedType"],
  ): FVariable {
    if (this.all.some((v) => v.variableCollectionId === collection.id && v.name === name))
      throw new Error(`duplicate variable ${name}`);
    const v = new FakeVariable(name, collection.id, type);
    this.all.push(v);
    return v;
  }
  async getLocalVariableCollectionsAsync(): Promise<FCollection[]> {
    return [...this.collections];
  }
  async getLocalVariablesAsync(): Promise<FVariable[]> {
    return [...this.all];
  }
  async getVariableByIdAsync(id: string): Promise<FVariable | null> {
    return this.all.find((v) => v.id === id) ?? null;
  }
  setBoundVariableForPaint(paint: FSolid, _field: "color", variable: FVariable | null): FSolid {
    if (variable === null) return { type: "SOLID", color: paint.color };
    const value = Object.values(variable.valuesByMode)[0] as FRGBA;
    return {
      type: "SOLID",
      color: { r: value.r, g: value.g, b: value.b },
      boundVariables: { color: { type: "VARIABLE_ALIAS", id: variable.id } },
    };
  }
}

export class FakeFigma implements FigmaApi {
  readonly mixed: symbol = Symbol("mixed");
  readonly root: { children: FakePage[] };
  readonly variables = new FakeVariables();
  readonly fonts = new Set<string>();
  currentPage: FakePage;

  constructor() {
    this.currentPage = new FakePage();
    this.currentPage.name = "Page 1";
    this.root = { children: [this.currentPage] };
  }
  requireFont(font: FFont): void {
    if (!this.fonts.has(`${font.family}/${font.style}`))
      throw new Error(`font ${font.family} ${font.style} is not loaded`);
  }
  async loadFontAsync(font: FFont): Promise<void> {
    this.fonts.add(`${font.family}/${font.style}`);
  }
  createPage(): FakePage {
    const page = new FakePage();
    this.root.children.push(page);
    return page;
  }
  createFrame(): FakeFrame {
    const f = new FakeFrame(this);
    this.currentPage.appendChild(f);
    return f;
  }
  createComponent(): FakeComponent {
    const c = new FakeComponent(this);
    this.currentPage.appendChild(c);
    return c;
  }
  createText(): FakeText {
    const t = new FakeText(this);
    this.currentPage.appendChild(t);
    return t;
  }
  createRectangle(): FakeRectangle {
    const r = new FakeRectangle();
    this.currentPage.appendChild(r);
    return r;
  }
  combineAsVariants(
    nodes: readonly FComponent[],
    parent: { appendChild(n: FNode): void },
  ): FakeComponentSet {
    if (nodes.length === 0) throw new Error("combineAsVariants needs components");
    const set = new FakeComponentSet(this);
    for (const n of nodes) set.appendChild(n);
    parent.appendChild(set);
    return set;
  }

  /** Every layer below `root` whose name is `name`, depth first. */
  findAll(root: FakeNode | FakePage, name: string): FakeNode[] {
    const out: FakeNode[] = [];
    const walk = (n: FakeNode | FakePage) => {
      if (n !== root && n.name === name) out.push(n as FakeNode);
      if (n instanceof FakeInstance && n !== root) return;
      for (const c of ("children" in n ? n.children : undefined) ?? []) walk(c);
    };
    walk(root);
    return out;
  }
  find<T extends FakeNode>(root: FakeNode | FakePage, name: string): T {
    const [found] = this.findAll(root, name);
    if (found === undefined) throw new Error(`no layer named ${name}`);
    return found as T;
  }
  /** Every page, layer, variable and collection that can hold plugin data. */
  everyDataHolderFor(): FakeBase[] {
    const out: FakeBase[] = [...this.variables.collections, ...this.variables.all];
    const walk = (n: FakeNode | FakePage) => {
      out.push(n);
      for (const c of ("children" in n ? n.children : undefined) ?? []) walk(c);
    };
    for (const page of this.root.children) walk(page);
    return out;
  }
}
