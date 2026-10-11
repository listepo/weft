// An in-memory Penpot file implementing the subset in `src/api.ts`, with the rules of Penpot's
// plugin API that the conversion depends on, taken from Penpot's sources (penpot/penpot, develop,
// frontend/src/app/plugins, checked 2026-10-05):
// - shared plugin data reads as missing (not `""`) for a key that was never set;
// - changes to a page that is not the active one are refused, and `createPage` does not open it;
// - a board keeps its children bottom to top, and a flex layout flows from the top one down; with
//   `flags.naturalChildOrdering`, a flex board's `children` follow the flow, and `appendChild`
//   adds to the top of any other board instead of the bottom;
// - a component copy's structure cannot change;
// - `createText("")` returns null and empty `characters` are refused;
// - tokens are applied by name, and `combineAsVariants` finishes after the call returns.
// Token themes follow `@penpot/plugin-types` 1.5.0 (`TokenTheme`, `TokenSet`): one theme of a
// group is on at a time, and turning a set on or off by hand turns every theme off.
// Typing a value over a token detaches it, as Penpot's UI does; whether the plugin API does the
// same is not documented, and the read-back handles both.
import type { PBlur, PFill, PStroke, PToken, PTokenType, PTrack } from "../src/api.ts";

const STRUCTURE = "Cannot change the structure of a component copy";

let serial = 0;
const nextId = (prefix: string) => `${prefix}-${++serial}`;

type Parent = FakeBoard | FakeGroup;
export type FakeShape = FakeBoard | FakeText | FakeGroup | FakeOther;

abstract class FakeBase {
  readonly id: string;
  name = "";
  x = 0;
  y = 0;
  width = 100;
  height = 100;
  hidden = false;
  rotation = 0;
  borderRadius = 0;
  backgroundBlur: PBlur | undefined;
  strokes: PStroke[] = [];
  layoutCell: { row?: number; column?: number } | undefined;
  parent: Parent | undefined;
  readonly tokens: Record<string, string | undefined> = {};
  /** Set on every shape of a component copy; the head is the copy's root. */
  inCopy = false;
  head = false;
  componentOf: FakeComponent | null = null;
  private readonly data = new Map<string, string>();
  private fillList: PFill[] | "mixed" = [];

  readonly penpot: FakePenpot;

  constructor(penpot: FakePenpot, prefix: string) {
    this.penpot = penpot;
    this.id = nextId(prefix);
  }

  get fills(): PFill[] | "mixed" {
    return this.fillList;
  }
  set fills(value: PFill[] | "mixed") {
    this.fillList = value;
    delete this.tokens["fill"];
  }

  getSharedPluginData(namespace: string, key: string): string {
    // Penpot returns nil for a missing key, whatever its types say.
    return this.data.get(`${namespace}/${key}`) as string;
  }
  setSharedPluginData(namespace: string, key: string, value: string): void {
    this.penpot.checkActive(this.page());
    this.data.set(`${namespace}/${key}`, value);
  }

  page(): FakePage | undefined {
    const top = (shape: FakeBase): FakeBase =>
      shape.parent === undefined ? shape : top(shape.parent);
    const root = top(this);
    return this.penpot.pages.find((p) => p.root === root);
  }

  applyToken(token: PToken, properties?: string[]): void {
    this.penpot.checkActive(this.page());
    const value = String(token.value);
    for (const property of properties ?? []) {
      this.setTokenValue(property, value);
      this.tokens[property] = token.name;
    }
  }

  protected setTokenValue(property: string, value: string): void {
    if (property === "fill") this.fillList = [colorFill(value)];
    else if (property === "strokeColor")
      this.strokes = this.strokes.map((s) => ({
        ...s,
        strokeColor: colorFill(value).fillColor ?? "",
      }));
  }

  resize(width: number, height: number): void {
    this.width = width;
    this.height = height;
  }

  remove(): void {
    if (this.parent === undefined) return;
    if (this.parent.inCopy) throw new Error(STRUCTURE);
    this.parent.detachChild(this as unknown as FakeShape);
  }

  isComponentMainInstance(): boolean {
    return !this.inCopy && this.componentOf !== null;
  }
  isComponentCopyInstance(): boolean {
    return this.inCopy;
  }
  isComponentHead(): boolean {
    return this.head;
  }
  component(): FakeComponent | null {
    return this.head ? this.componentOf : null;
  }

  /** Switches a copy to the variant with `value` at property `pos`, keeping its overrides. */
  switchVariant(pos: number, value: string): void {
    const current = this.component();
    if (!this.inCopy || current === null || current.container === undefined)
      throw new Error("switchVariant needs a copy of a variant");
    const name = current.container.properties[pos];
    if (name === undefined) throw new Error(`no variant property at ${pos}`);
    const wanted = { ...current.variantProps, [name]: value };
    const target = current.container.components.find((c) =>
      Object.entries(wanted).every(([k, v]) => c.variantProps[k] === v),
    );
    if (target === undefined) throw new Error(`no variant ${JSON.stringify(wanted)}`);
    const board = this as unknown as FakeBoard;
    const texts = new Map(
      board.descendants().flatMap((s) => (s instanceof FakeText ? [[s.name, s.characters]] : [])),
    );
    const fresh = target.main.cloneShape();
    board.replaceContent(fresh);
    for (const s of board.descendants())
      if (s instanceof FakeText && texts.has(s.name)) s.characters = texts.get(s.name) as string;
    board.componentOf = target;
  }

  combineAsVariants(ids: string[]): FakeBoard {
    const component = this.componentOf;
    if (!this.isComponentMainInstance() || component === null || this.parent === undefined)
      throw new Error("combineAsVariants needs a component's main board");
    const parent = this.parent;
    const container = new FakeBoard(this.penpot);
    container.variantContainer = true;
    container.fills = [];
    parent.insertRaw(parent.rawChildren().indexOf(this as unknown as FakeShape), container);
    const mains = [
      this as unknown as FakeBoard,
      ...ids.map((id) => {
        const found = this.penpot.library.local.components.find((c) => c.main.id === id);
        if (found === undefined) throw new Error(`no main board ${id}`);
        return found.main;
      }),
    ];
    const variants = new FakeVariants(mains.map((m) => m.componentOf as FakeComponent));
    for (const main of mains) {
      main.parent?.detachChild(main);
      container.pushRaw(main);
      const c = main.componentOf as FakeComponent;
      c.container = variants;
      c.props = { "Property 1": c.name };
    }
    // Penpot applies the combination after the call returns.
    setTimeout(() => (container.variantsReady = variants), 0);
    return container;
  }

  protected copyBase(to: FakeBase): void {
    to.name = this.name;
    to.x = this.x;
    to.y = this.y;
    to.width = this.width;
    to.height = this.height;
    to.hidden = this.hidden;
    to.rotation = this.rotation;
    to.borderRadius = this.borderRadius;
    to.backgroundBlur = this.backgroundBlur === undefined ? undefined : { ...this.backgroundBlur };
    to.fillList = structuredClone(this.fillList);
    to.strokes = structuredClone(this.strokes);
    Object.assign(to.tokens, this.tokens);
    for (const [k, v] of this.data) to.data.set(k, v);
    to.componentOf = this.componentOf;
  }

  abstract cloneShape(): FakeShape;

  /** A duplicate placed next to the shape, as the designer's Duplicate makes it. */
  clone(): FakeShape {
    const copy = this.cloneShape();
    const all =
      copy instanceof FakeBoard || copy instanceof FakeGroup
        ? [copy, ...copy.descendants()]
        : [copy];
    for (const s of all) s.inCopy = this.inCopy;
    copy.head = this.head;
    this.penpot.currentRoot().pushRaw(copy);
    return copy;
  }
}

const colorFill = (value: string): PFill => {
  const rgba = /^rgba\((\d+), (\d+), (\d+), ([\d.]+)\)$/.exec(value);
  if (rgba === null) return { fillColor: value, fillOpacity: 1 };
  const hex = rgba
    .slice(1, 4)
    .map((n) => Number(n).toString(16).padStart(2, "0"))
    .join("");
  return { fillColor: `#${hex}`, fillOpacity: Number(rgba[4]) };
};

class FakeLayout {
  alignItems?: "start" | "end" | "center" | "stretch";
  horizontalSizing: "fix" | "fill" | "auto" = "fix";
  verticalSizing: "fix" | "fill" | "auto" = "fix";
  private values: Record<string, number> = {
    rowGap: 0,
    columnGap: 0,
    topPadding: 0,
    rightPadding: 0,
    bottomPadding: 0,
    leftPadding: 0,
  };
  protected readonly owner: FakeBoard;
  constructor(owner: FakeBoard) {
    this.owner = owner;
  }
  private get(field: string): number {
    return this.values[field] ?? 0;
  }
  private set(field: string, token: string, value: number): void {
    this.values[field] = value;
    delete this.owner.tokens[token];
  }
  /** Sets a value as a token does, keeping the binding. */
  setFromToken(field: string, value: number): void {
    this.values[field] = value;
  }
  get rowGap() {
    return this.get("rowGap");
  }
  set rowGap(v) {
    this.set("rowGap", "rowGap", v);
  }
  get columnGap() {
    return this.get("columnGap");
  }
  set columnGap(v) {
    this.set("columnGap", "columnGap", v);
  }
  get topPadding() {
    return this.get("topPadding");
  }
  set topPadding(v) {
    this.set("topPadding", "paddingTop", v);
  }
  get rightPadding() {
    return this.get("rightPadding");
  }
  set rightPadding(v) {
    this.set("rightPadding", "paddingRight", v);
  }
  get bottomPadding() {
    return this.get("bottomPadding");
  }
  set bottomPadding(v) {
    this.set("bottomPadding", "paddingBottom", v);
  }
  get leftPadding() {
    return this.get("leftPadding");
  }
  set leftPadding(v) {
    this.set("leftPadding", "paddingLeft", v);
  }
  copyTo(to: FakeLayout): void {
    if (this.alignItems !== undefined) to.alignItems = this.alignItems;
    to.horizontalSizing = this.horizontalSizing;
    to.verticalSizing = this.verticalSizing;
    to.values = { ...this.values };
  }
}

class FakeFlex extends FakeLayout {
  dir: "row" | "row-reverse" | "column" | "column-reverse" = "row";
  wrap?: "wrap" | "nowrap";
}

class FakeGrid extends FakeLayout {
  columns: PTrack[] = [{ type: "flex", value: 1 }];
  rows: PTrack[] = [{ type: "flex", value: 1 }];
  addRow(type: PTrack["type"], value?: number): void {
    this.rows.push({ type, value: value ?? null });
  }
  addColumn(type: PTrack["type"], value?: number): void {
    this.columns.push({ type, value: value ?? null });
  }
  removeRow(index: number): void {
    this.rows.splice(index, 1);
  }
  removeColumn(index: number): void {
    this.columns.splice(index, 1);
  }
  appendChild(child: FakeShape, row: number, column: number): void {
    if (row < 1 || row > this.rows.length || column < 1 || column > this.columns.length)
      throw new Error(`no grid cell ${row},${column}`);
    this.owner.appendChild(child);
    child.layoutCell = { row, column };
  }
}

const LAYOUT_TOKEN_FIELD: Readonly<Record<string, string>> = {
  rowGap: "rowGap",
  columnGap: "columnGap",
  paddingTop: "topPadding",
  paddingRight: "rightPadding",
  paddingBottom: "bottomPadding",
  paddingLeft: "leftPadding",
};

export class FakeBoard extends FakeBase {
  readonly type = "board";
  flex: FakeFlex | undefined;
  grid: FakeGrid | undefined;
  variantContainer = false;
  variantsReady: FakeVariants | null = null;
  /** Bottom to top, as Penpot stores them; a flex board's flow is the reverse. */
  private shapes: FakeShape[] = [];

  constructor(penpot: FakePenpot) {
    super(penpot, "board");
    // A new board is white, as Penpot draws it.
    this.fills = [{ fillColor: "#FFFFFF", fillOpacity: 1 }];
  }

  override get fills(): PFill[] {
    return super.fills as PFill[];
  }
  override set fills(value: PFill[]) {
    super.fills = value;
  }

  get variants(): FakeVariants | null {
    return this.variantsReady;
  }

  private natural(): boolean {
    return this.penpot.flags.naturalChildOrdering && this.flex !== undefined;
  }

  get children(): FakeShape[] {
    return this.natural() ? this.flow() : [...this.shapes];
  }

  /** The order the canvas shows: a flex layout puts the top of the stack first. */
  flow(): FakeShape[] {
    return this.flex !== undefined ? [...this.shapes].reverse() : [...this.shapes];
  }

  /** A designer dragging a layer to `index` in the order the canvas shows. */
  moveInFlow(child: FakeShape, index: number): void {
    this.adopt(child);
    const flow = this.flow();
    flow.splice(index, 0, child);
    this.shapes = this.flex !== undefined ? flow.reverse() : flow;
  }

  rawChildren(): FakeShape[] {
    return this.shapes;
  }

  private adopt(child: FakeShape): void {
    if (this.inCopy || child.parent?.inCopy === true) throw new Error(STRUCTURE);
    this.penpot.checkActive(this.page());
    child.parent?.detachChild(child);
    child.parent = this;
    child.layoutCell = undefined;
  }

  appendChild(child: FakeShape): void {
    this.adopt(child);
    // Penpot puts the child at the bottom of the stack unless natural ordering is on and the
    // board has no flex layout; at the bottom of a flex board is the end of the flow.
    if (!this.penpot.flags.naturalChildOrdering || this.flex !== undefined)
      this.shapes.unshift(child);
    else this.shapes.push(child);
    if (this.grid !== undefined) {
      const last = this.shapes.reduce(
        (n, s) =>
          Math.max(
            n,
            ((s.layoutCell?.row ?? 1) - 1) * this.grid!.columns.length +
              (s.layoutCell?.column ?? 0),
          ),
        0,
      );
      const columns = Math.max(this.grid.columns.length, 1);
      child.layoutCell = { row: Math.floor(last / columns) + 1, column: (last % columns) + 1 };
    }
  }

  insertChild(index: number, child: FakeShape): void {
    this.adopt(child);
    if (!this.penpot.flags.naturalChildOrdering || this.flex !== undefined)
      this.shapes.splice(this.shapes.length - index, 0, child);
    else this.shapes.splice(index, 0, child);
  }

  /** Internal moves that are not a designer's or plugin's edit. */
  insertRaw(index: number, child: FakeShape): void {
    child.parent = this;
    this.shapes.splice(index, 0, child);
  }
  pushRaw(child: FakeShape): void {
    child.parent = this;
    this.shapes.push(child);
  }
  detachChild(child: FakeShape): void {
    this.shapes.splice(this.shapes.indexOf(child), 1);
    child.parent = undefined;
  }

  addFlexLayout(): FakeFlex {
    this.grid = undefined;
    this.flex = new FakeFlex(this);
    return this.flex;
  }
  addGridLayout(): FakeGrid {
    this.flex = undefined;
    this.grid = new FakeGrid(this);
    return this.grid;
  }
  isVariantContainer(): boolean {
    return this.variantContainer;
  }

  /** Detaches a component copy: it becomes a plain board that keeps its layers. */
  detach(): void {
    for (const s of [this, ...this.descendants()]) s.inCopy = false;
    this.head = false;
    this.componentOf = null;
  }

  protected override setTokenValue(property: string, value: string): void {
    const field = LAYOUT_TOKEN_FIELD[property];
    const layout = this.flex ?? this.grid;
    if (field !== undefined && layout !== undefined) layout.setFromToken(field, Number(value));
    else super.setTokenValue(property, value);
  }

  descendants(): FakeShape[] {
    return this.shapes.flatMap((s) => [
      s,
      ...(s instanceof FakeBoard || s instanceof FakeGroup ? s.descendants() : []),
    ]);
  }

  /** Takes the layout and children of `from`: a variant switch. */
  replaceContent(from: FakeBoard): void {
    for (const s of this.shapes) s.parent = undefined;
    this.shapes = from.shapes;
    for (const s of this.shapes) s.parent = this;
    this.flex = from.flex;
    this.grid = from.grid;
    this.fills = from.fills;
    this.strokes = from.strokes;
    this.borderRadius = from.borderRadius;
    this.backgroundBlur = from.backgroundBlur;
    for (const s of this.descendants()) s.inCopy = true;
  }

  cloneShape(): FakeBoard {
    const to = new FakeBoard(this.penpot);
    this.copyBase(to);
    to.variantContainer = this.variantContainer;
    if (this.flex !== undefined) {
      const flex = to.addFlexLayout();
      this.flex.copyTo(flex);
      flex.dir = this.flex.dir;
      if (this.flex.wrap !== undefined) flex.wrap = this.flex.wrap;
    }
    if (this.grid !== undefined) {
      const grid = to.addGridLayout();
      this.grid.copyTo(grid);
      grid.columns = [...this.grid.columns];
      grid.rows = [...this.grid.rows];
    }
    for (const s of this.shapes) {
      const c = s.cloneShape();
      c.layoutCell = s.layoutCell === undefined ? undefined : { ...s.layoutCell };
      to.pushRaw(c);
    }
    return to;
  }
}

export class FakeText extends FakeBase {
  readonly type = "text";
  fontSize = "14";
  fontWeight = "400";
  growType: "fixed" | "auto-width" | "auto-height" = "fixed";
  private text: string;

  constructor(penpot: FakePenpot, characters: string) {
    super(penpot, "text");
    this.text = characters;
  }
  get characters(): string {
    return this.text;
  }
  set characters(value: string) {
    if (value === "") throw new Error("characters cannot be empty");
    this.text = value;
  }
  cloneShape(): FakeText {
    const to = new FakeText(this.penpot, this.text);
    this.copyBase(to);
    to.fontSize = this.fontSize;
    to.fontWeight = this.fontWeight;
    to.growType = this.growType;
    return to;
  }
}

/** A group or boolean: children in z-order, positioned by hand. */
export class FakeGroup extends FakeBase {
  private shapes: FakeShape[] = [];
  readonly type: "group" | "boolean";
  constructor(penpot: FakePenpot, type: "group" | "boolean", children: FakeShape[] = []) {
    super(penpot, type);
    this.type = type;
    for (const c of children) {
      c.parent?.detachChild(c);
      c.parent = this;
      this.shapes.push(c);
    }
  }
  get children(): FakeShape[] {
    return [...this.shapes];
  }
  rawChildren(): FakeShape[] {
    return this.shapes;
  }
  insertRaw(index: number, child: FakeShape): void {
    child.parent = this;
    this.shapes.splice(index, 0, child);
  }
  detachChild(child: FakeShape): void {
    this.shapes.splice(this.shapes.indexOf(child), 1);
    child.parent = undefined;
  }
  descendants(): FakeShape[] {
    return this.shapes.flatMap((s) => [
      s,
      ...(s instanceof FakeBoard || s instanceof FakeGroup ? s.descendants() : []),
    ]);
  }
  cloneShape(): FakeGroup {
    const to = new FakeGroup(
      this.penpot,
      this.type,
      this.shapes.map((s) => s.cloneShape()),
    );
    this.copyBase(to);
    return to;
  }
}

export class FakeOther extends FakeBase {
  readonly type: "rectangle" | "ellipse" | "path" | "svg-raw" | "image";
  constructor(penpot: FakePenpot, type: FakeOther["type"]) {
    super(penpot, type);
    this.type = type;
  }
  cloneShape(): FakeOther {
    const to = new FakeOther(this.penpot, this.type);
    this.copyBase(to);
    return to;
  }
}

export class FakeVariants {
  properties: string[] = ["Property 1"];
  readonly components: FakeComponent[];
  constructor(components: FakeComponent[]) {
    this.components = components;
  }
  addProperty(): void {
    const name = `Property ${this.properties.length + 1}`;
    this.properties.push(name);
    for (const c of this.components) c.props[name] = "";
  }
  removeProperty(pos: number): void {
    const [name] = this.properties.splice(pos, 1);
    for (const c of this.components) if (name !== undefined) delete c.props[name];
  }
  renameProperty(pos: number, name: string): void {
    const old = this.properties[pos];
    if (old === undefined) throw new Error(`no variant property at ${pos}`);
    this.properties[pos] = name;
    for (const c of this.components) {
      c.props[name] = c.props[old] ?? "";
      delete c.props[old];
    }
  }
  variantComponents(): FakeComponent[] {
    return [...this.components];
  }
}

export class FakeComponent {
  readonly id = nextId("component");
  path = "";
  container: FakeVariants | undefined;
  props: Record<string, string> = {};
  readonly penpot: FakePenpot;
  readonly main: FakeBoard;
  constructor(penpot: FakePenpot, main: FakeBoard) {
    this.penpot = penpot;
    this.main = main;
  }
  get name(): string {
    return this.main.name;
  }
  set name(value: string) {
    this.main.name = value;
  }
  get variants(): FakeVariants | null {
    return this.container ?? null;
  }
  /** In property order, as Penpot reports them. */
  get variantProps(): Record<string, string> {
    const out: Record<string, string> = {};
    for (const p of this.container?.properties ?? []) out[p] = this.props[p] ?? "";
    return out;
  }
  setVariantProperty(pos: number, value: string): void {
    const name = this.container?.properties[pos];
    if (name === undefined) throw new Error(`no variant property at ${pos}`);
    this.props[name] = value;
  }
  isVariant(): this is FakeComponent & { variants: FakeVariants } {
    return this.container !== undefined;
  }
  mainInstance(): FakeBoard {
    return this.main;
  }
  /** A copy on the current page, at its origin. */
  instance(): FakeBoard {
    const copy = this.main.cloneShape();
    for (const s of [copy, ...copy.descendants()]) s.inCopy = true;
    copy.head = true;
    copy.componentOf = this;
    copy.x = 0;
    copy.y = 0;
    this.penpot.currentRoot().pushRaw(copy);
    return copy;
  }
}

export class FakeToken implements PToken {
  readonly id = nextId("token");
  readonly type: PTokenType;
  name: string;
  value: unknown;
  constructor(type: PTokenType, name: string, value: unknown) {
    this.type = type;
    this.name = name;
    this.value = value;
  }
}

export class FakeTokenSet {
  readonly id = nextId("set");
  readonly tokens: FakeToken[] = [];
  name: string;
  #active: boolean;
  private readonly catalog: FakeTokenCatalog;
  constructor(catalog: FakeTokenCatalog, name: string, active: boolean) {
    this.catalog = catalog;
    this.name = name;
    this.#active = active;
  }
  get active(): boolean {
    return this.#active;
  }
  /** Penpot: turning a set on or off by hand turns every theme off (a "custom" theme). */
  set active(value: boolean) {
    this.#active = value;
    for (const theme of this.catalog.themes) theme.active = false;
  }
  /** What a theme does to its sets, which leaves the other themes as they are. */
  activate(value: boolean): void {
    this.#active = value;
  }
  addToken(token: { type: PTokenType; name: string; value: string }): FakeToken {
    if (this.tokens.some((t) => t.name === token.name))
      throw new Error(`a token named ${token.name} exists`);
    const made = new FakeToken(token.type, token.name, token.value);
    this.tokens.push(made);
    return made;
  }
}

export class FakeTokenTheme {
  readonly id = nextId("theme");
  readonly activeSets: FakeTokenSet[] = [];
  group: string;
  name: string;
  active = false;
  private readonly catalog: FakeTokenCatalog;
  constructor(catalog: FakeTokenCatalog, group: string, name: string) {
    this.catalog = catalog;
    this.group = group;
    this.name = name;
  }
  /** Penpot: one theme per group is on; switching turns the other's sets off, then this one's on. */
  toggleActive(): void {
    if (this.active) {
      this.active = false;
      return;
    }
    for (const other of this.catalog.themes) {
      if (other === this || other.group !== this.group || !other.active) continue;
      other.active = false;
      for (const set of other.activeSets) if (!this.activeSets.includes(set)) set.activate(false);
    }
    this.active = true;
    for (const set of this.activeSets) set.activate(true);
  }
  addSet(setId: string): void {
    const set = this.catalog.sets.find((s) => s.id === setId);
    if (set === undefined) throw new Error(`no token set ${setId}`);
    if (!this.activeSets.includes(set)) this.activeSets.push(set);
  }
}

export class FakeTokenCatalog {
  readonly sets: FakeTokenSet[] = [];
  readonly themes: FakeTokenTheme[] = [];
  addSet(set: { name: string; active?: boolean }): FakeTokenSet {
    const made = new FakeTokenSet(this, set.name, set.active ?? false);
    this.sets.push(made);
    return made;
  }
  addTheme(theme: { group: string; name: string }): FakeTokenTheme {
    if (this.themes.some((t) => t.group === theme.group && t.name === theme.name))
      throw new Error(`a theme ${theme.group}/${theme.name} exists`);
    const made = new FakeTokenTheme(this, theme.group, theme.name);
    this.themes.push(made);
    return made;
  }
}

export class FakePage {
  readonly id = nextId("page");
  name = "Page";
  readonly root: FakeBoard;
  private readonly data = new Map<string, string>();
  constructor(penpot: FakePenpot) {
    this.root = new FakeBoard(penpot);
    this.root.fills = [];
  }
  getSharedPluginData(namespace: string, key: string): string {
    return this.data.get(`${namespace}/${key}`) as string;
  }
  setSharedPluginData(namespace: string, key: string, value: string): void {
    this.data.set(`${namespace}/${key}`, value);
  }
}

export class FakePenpot {
  readonly flags = { naturalChildOrdering: false };
  readonly pages: FakePage[] = [];
  private readonly libraryData = new Map<string, string>();
  currentPage: FakePage;
  readonly library = {
    local: {
      components: [] as FakeComponent[],
      getSharedPluginData: (namespace: string, key: string): string =>
        this.libraryData.get(`${namespace}/${key}`) as string,
      setSharedPluginData: (namespace: string, key: string, value: string): void => {
        this.libraryData.set(`${namespace}/${key}`, value);
      },
      tokens: new FakeTokenCatalog(),
      createComponent: (shapes: FakeShape[]): FakeComponent => {
        const [main] = shapes;
        if (!(main instanceof FakeBoard) || shapes.length !== 1)
          throw new Error("the fake makes components from one board");
        this.checkActive(main.page());
        const component = new FakeComponent(this, main);
        main.componentOf = component;
        main.head = true;
        this.library.local.components.push(component);
        return component;
      },
    },
  };

  constructor() {
    this.currentPage = new FakePage(this);
    this.pages.push(this.currentPage);
  }

  get currentFile() {
    return { pages: this.pages };
  }

  checkActive(page: FakePage | undefined): void {
    if (page !== undefined && page !== this.currentPage)
      throw new Error("Cannot modify a page that is not currently active");
  }

  currentRoot(): FakeBoard {
    return this.currentPage.root;
  }

  createPage(): FakePage {
    const page = new FakePage(this);
    this.pages.push(page);
    return page;
  }

  async openPage(id: string): Promise<void> {
    const page = this.pages.find((p) => p.id === id);
    if (page === undefined) throw new Error(`no page ${id}`);
    this.currentPage = page;
  }

  createBoard(): FakeBoard {
    const board = new FakeBoard(this);
    this.currentRoot().pushRaw(board);
    return board;
  }

  createRectangle(): FakeOther & { type: "rectangle" } {
    const rect = new FakeOther(this, "rectangle") as FakeOther & { type: "rectangle" };
    this.currentRoot().pushRaw(rect);
    return rect;
  }

  createText(text: string): FakeText | null {
    if (text === "") return null;
    const made = new FakeText(this, text);
    this.currentRoot().pushRaw(made);
    return made;
  }

  /** The first shape named `name` below `shape`, breadth first. */
  find<T extends FakeShape = FakeShape>(shape: FakeShape, name: string): T {
    const queue: FakeShape[] = [shape];
    while (queue.length > 0) {
      const at = queue.shift() as FakeShape;
      for (const c of "children" in at ? at.children : []) {
        if (c.name === name) return c as T;
        queue.push(c);
      }
    }
    throw new Error(`no shape named ${name}`);
  }

  token(path: string): FakeToken {
    for (const set of this.library.local.tokens.sets) {
      const found = set.tokens.find((t) => t.name === path);
      if (found !== undefined) return found;
    }
    throw new Error(`no token ${path}`);
  }
}
