// Turns a canonical JSON document (SPEC §3) into the instance tree that both the React renderer
// and `expectedTree` read: `each` expanded with instance ids, `hidden` nodes removed, and every
// node paired with the scope its bindings resolve in. Both consumers share it so the renderer
// and the oracle it is tested against can never disagree about what is on screen.
import type { Catalog, ComponentDef, PropDef } from "@weft/core";
import { isRecord, resolveValue, toText, truthy, type Resolved, type Scope } from "./values.ts";

export type Inst = {
  kind: string;
  // Template id from the document, and the rendered instance id (`id[index]` inside `each`).
  docId: string;
  id: string;
  def: ComponentDef | undefined;
  props: Record<string, unknown>;
  on: Record<string, string>;
  children: InstChild[];
  slots: Record<string, InstChild[]>;
  scope: Scope;
  // Whether the default slot has something other than empty loops to show (SPEC §5.1 `empty`).
  filled: boolean;
};
export type InstChild = Inst | string;

// A hostile document can nest arbitrarily deep (or be a cyclic object graph when passed in
// memory); past this depth content is not rendered rather than overflowing the stack.
const MAX_DEPTH = 200;
const LOOP_VAR = /^[a-z][A-Za-z0-9]*$/;

export function expandRoot(root: unknown, catalog: Catalog, data: unknown): Inst | undefined {
  const scope: Scope = { data, vars: new Map(), suffix: "" };
  const [first] = expandChildren([root], scope, catalog, 0).filter(
    (c): c is Inst => typeof c !== "string",
  );
  return first;
}

function expandChildren(list: unknown, scope: Scope, catalog: Catalog, depth: number): InstChild[] {
  const out: InstChild[] = [];
  if (!Array.isArray(list) || depth > MAX_DEPTH) return out;
  const push = (c: InstChild) => {
    const last = out.at(-1);
    // Adjacent text would render as one DOM text node; merging here keeps both views equal.
    if (typeof c === "string" && typeof last === "string") out[out.length - 1] = `${last} ${c}`;
    else out.push(c);
  };
  for (const child of list) {
    if (typeof child === "string") {
      if (child.trim() !== "") push(child);
      continue;
    }
    if (!isRecord(child) || typeof child["kind"] !== "string") continue;
    const props = isRecord(child["props"]) ? child["props"] : {};
    if (child["kind"] === "each") {
      for (const c of expandEach(child, props, scope, catalog, depth)) push(c);
      continue;
    }
    if (truthy(resolveValue(props["hidden"], scope).value)) continue;
    push(expandNode(child, child["kind"], props, scope, catalog, depth));
  }
  return out;
}

function expandEach(
  node: Record<string, unknown>,
  props: Record<string, unknown>,
  scope: Scope,
  catalog: Catalog,
  depth: number,
): InstChild[] {
  const source = resolveValue(props["in"], scope);
  const as = props["as"];
  if (!Array.isArray(source.value) || source.path === undefined) return [];
  if (typeof as !== "string" || !LOOP_VAR.test(as)) return [];
  const out: InstChild[] = [];
  source.value.forEach((value, i) => {
    const path = `${source.path}.${i}`;
    const vars = new Map(scope.vars).set(as, { value, path });
    const inner: Scope = { data: scope.data, vars, suffix: `${scope.suffix}[${i}]`, item: path };
    out.push(...expandChildren(node["children"], inner, catalog, depth + 1));
  });
  return out;
}

function expandNode(
  node: Record<string, unknown>,
  kind: string,
  props: Record<string, unknown>,
  scope: Scope,
  catalog: Catalog,
  depth: number,
): Inst {
  const docId = typeof node["id"] === "string" ? node["id"] : "";
  const on: Record<string, string> = {};
  if (isRecord(node["on"])) {
    for (const [event, action] of Object.entries(node["on"])) {
      if (typeof action === "string") on[event] = action;
    }
  }
  const slots: Record<string, InstChild[]> = {};
  if (isRecord(node["slots"])) {
    for (const name of Object.keys(node["slots"]).sort()) {
      slots[name] = expandChildren(node["slots"][name], scope, catalog, depth + 1);
    }
  }
  const def = Object.hasOwn(catalog.components, kind) ? catalog.components[kind] : undefined;
  let children = expandChildren(node["children"], scope, catalog, depth + 1);
  // SPEC §5.1: every text-bearing kind takes its text from content or from the `text` prop. Turning
  // the prop into content here lets every later reading treat both spellings alike; content wins
  // when a lenient reader is given both.
  if ((def?.content === "text" || def?.content === "mixed") && children.length === 0) {
    const own = toText(resolveValue(props["text"], scope).value);
    if (own.trim() !== "") children = [own];
  }
  return {
    kind,
    docId,
    id: docId === "" ? "" : docId + scope.suffix,
    def,
    props,
    on,
    children,
    slots,
    scope,
    filled: Array.isArray(node["children"]) && node["children"].some((c) => shows(c, scope)),
  };
}

// SPEC §5.1 `empty`: a static element counts even when hidden, a loop only when its array has
// items. Table columns are the header, not rows, so they never count.
function shows(child: unknown, scope: Scope): boolean {
  if (!isRecord(child) || typeof child["kind"] !== "string" || child["kind"] === "column")
    return false;
  if (child["kind"] !== "each") return true;
  const source = isRecord(child["props"]) ? resolveValue(child["props"]["in"], scope).value : [];
  return Array.isArray(source) && source.length > 0;
}

// ---- Shared readings of props; the renderer and `expectedTree` must agree on each of these. ----

export function prop(n: Inst, name: string): Resolved {
  const resolved = resolveValue(n.props[name], n.scope);
  const def = n.def?.props && Object.hasOwn(n.def.props, name) ? n.def.props[name] : undefined;
  if (def?.type !== "number" || typeof resolved.value !== "number") return resolved;
  return { ...resolved, value: clamp(resolved.value, def) };
}

// SPEC §5.1: validation keeps literals in range, so this only ever moves bound data, which the
// renderer brings into the declared range rather than rendering something the catalog forbids.
function clamp(value: number, def: PropDef): number | undefined {
  if (!Number.isFinite(value)) return undefined;
  let v = def.integer === true ? Math.round(value) : value;
  if (typeof def.min === "number") v = Math.max(v, def.min);
  if (typeof def.max === "number") v = Math.min(v, def.max);
  return v;
}

export const text = (n: Inst, name: string): string => toText(prop(n, name).value);
export const flag = (n: Inst, name: string): boolean => truthy(prop(n, name).value);
export const label = (n: Inst): string => text(n, "label").trim();
export const state = (n: Inst): string => text(n, "state");

const declaresSlot = (n: Inst, name: string): boolean =>
  n.def?.slots !== undefined && Object.hasOwn(n.def.slots, name);

// SPEC §5.1: a declared `empty` slot replaces the items when there are none to show or the
// state says so.
export function showsEmpty(n: Inst): boolean {
  if (!declaresSlot(n, "empty") || n.slots["empty"] === undefined) return false;
  return state(n) === "empty" || !n.filled;
}

// A run of content in display order; `slot` names the slot it came from, absent for the default
// content.
export type Region = { slot?: string; list: InstChild[] };

// SPEC §4.2: slot placement is the component's. `header` leads, a declared `empty` stands in for
// the default content when it is shown and is left out otherwise, and every other slot follows.
export function regions(n: Inst): Region[] {
  const before: Region[] = [];
  const after: Region[] = [];
  for (const [slot, list] of Object.entries(n.slots)) {
    if (slot === "empty" && declaresSlot(n, slot)) continue;
    (slot === "header" ? before : after).push({ slot, list });
  }
  const body: Region = showsEmpty(n)
    ? { slot: "empty", list: n.slots["empty"] ?? [] }
    : { list: n.children };
  return [...before, body, ...after];
}

export const ordered = (n: Inst): InstChild[] => regions(n).flatMap((r) => r.list);

export const nodes = (list: InstChild[]): Inst[] =>
  list.filter((c): c is Inst => typeof c !== "string");

// An out-of-range level renders as 2, the level ARIA assumes for a heading without one.
export function headingLevel(n: Inst): number {
  const level = prop(n, "level").value;
  const num = typeof level === "string" ? Number(level) : level;
  return typeof num === "number" && Number.isInteger(num) && num >= 1 && num <= 6 ? num : 2;
}

// The text a subtree contributes to an accessible name computed from content (accname 2F),
// joined with spaces because the renderer emits no inline boxes between named parts.
export function contentText(list: InstChild[]): string {
  const parts: string[] = [];
  for (const c of list) {
    const t = typeof c === "string" ? c : contribution(c);
    if (t.trim() !== "") parts.push(t.trim());
  }
  return parts.join(" ").replace(/\s+/g, " ");
}

function contribution(n: Inst): string {
  if (n.def) {
    if (n.kind === "dialog" && !dialogOpen(n)) return "";
    // Embedded controls contribute their value, not their label (accname 2C exception, 2E).
    if (n.kind === "field") return [fieldValue(n), text(n, "error")].filter(Boolean).join(" ");
    if (n.kind === "select") return selectedOption(n)?.text ?? "";
  }
  const own = label(n);
  if (own !== "" && n.kind !== "text" && n.kind !== "stack" && n.kind !== "grid") return own;
  if (n.def) {
    if (n.kind === "checkbox" || n.kind === "switch" || n.kind === "image" || n.kind === "model") return "";
    if (n.kind === "tabs") {
      const tabs = nodes(n.children).filter((c) => c.kind === "tab");
      const sel = tabs[selectedTab(n, tabs)];
      return [...tabs.map(label), sel ? contentText(sel.children) : ""].join(" ").trim();
    }
  }
  return contentText(ordered(n));
}

export type FieldRole = "textbox" | "spinbutton" | "searchbox";

// SPEC §5.1: `field` is a textbox refined by `type` as ARIA requires.
export function fieldRole(n: Inst): FieldRole {
  const type = text(n, "type");
  return type === "number" ? "spinbutton" : type === "search" ? "searchbox" : "textbox";
}

// A field is invalid when its state says so or when it shows an error message.
export const fieldInvalid = (n: Inst): boolean => state(n) === "invalid" || text(n, "error") !== "";

const FLOAT = /^-?(?:\d+(?:\.\d+)?|\.\d+)(?:[eE][-+]?\d+)?$/;

// What the browser shows: a number input sanitizes anything that is not a valid float to "".
export function fieldValue(n: Inst): string {
  const value = text(n, "value");
  return text(n, "type") === "number" && !FLOAT.test(value) ? "" : value;
}

// ---- Number, date and colour controls (SPEC §5.1 notes) ----

// A number prop as a finite number: a number, or text written as one. Text such as "0x10" or
// "Infinity" is data that does not fit the prop, so it reads as absent.
export function numeric(v: unknown): number | undefined {
  if (typeof v === "number") return Number.isFinite(v) ? v : undefined;
  if (typeof v === "string" && FLOAT.test(v.trim())) {
    const n = Number(v);
    return Number.isFinite(n) ? n : undefined;
  }
  return undefined;
}

const num = (n: Inst, name: string): number | undefined => numeric(prop(n, name).value);

function decimals(x: number): number {
  const [mantissa = "", exponent] = String(x).split("e");
  const own = mantissa.split(".")[1]?.length ?? 0;
  return Math.max(0, own - (exponent === undefined ? 0 : Number(exponent)));
}

// Sums and products of decimal steps carry binary error (0.1 × 3); rounding to the decimals the
// inputs have gives back the number a person means.
export function tidy(x: number, ...inputs: number[]): number {
  return Number(x.toFixed(Math.min(100, Math.max(...inputs.map(decimals)))));
}

export type Span = { min: number; max: number; step: number };

// A `slider`: both bounds always exist, `max` below `min` is `min`, and a step that is not above 0
// is 1.
export function sliderSpan(n: Inst): Span {
  const min = num(n, "min") ?? 0;
  const max = Math.max(num(n, "max") ?? 100, min);
  const step = num(n, "step");
  return { min, max, step: step !== undefined && step > 0 ? step : 1 };
}

// What a range input shows: the value inside the bounds on the grid of steps from `min`, a tie
// going up and a point past the last step falling back to it.
export function sliderValue(n: Inst): number {
  const { min, max, step } = sliderSpan(n);
  const v = Math.min(Math.max(num(n, "value") ?? min, min), max);
  let on = min + Math.floor((v - min) / step + 0.5) * step;
  if (on > max) on -= step;
  return tidy(Math.max(on, min), min, step);
}

export type Bounds = { min?: number; max?: number; step: number };

// A `stepper`: a side without a bound is open.
export function stepperBounds(n: Inst): Bounds {
  const min = num(n, "min");
  const max = num(n, "max");
  const step = num(n, "step");
  const out: Bounds = { step: step !== undefined && step > 0 ? step : 1 };
  if (min !== undefined) out.min = min;
  if (max !== undefined) out.max = min === undefined ? max : Math.max(max, min);
  return out;
}

export function clampTo(v: number, bounds: Bounds): number {
  let out = v;
  if (bounds.min !== undefined) out = Math.max(out, bounds.min);
  if (bounds.max !== undefined) out = Math.min(out, bounds.max);
  return out;
}

export function stepperValue(n: Inst): number {
  return clampTo(num(n, "value") ?? 0, stepperBounds(n));
}

// One press of the plus or minus button: a step from the shown value, kept inside the bounds.
export function stepped(current: number, direction: 1 | -1, bounds: Bounds): number {
  return tidy(clampTo(current + direction * bounds.step, bounds), current, bounds.step);
}

export type DateType = "date" | "time" | "datetime";

export function dateType(n: Inst): DateType {
  const type = text(n, "type");
  return type === "time" || type === "datetime" ? type : "date";
}

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
const TIME = /^([01]\d|2[0-3]):[0-5]\d$/;

function validDate(s: string): boolean {
  const m = DATE.exec(s);
  if (!m) return false;
  const [year, month, day] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (year < 1 || month < 1 || month > 12 || day < 1) return false;
  const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const length = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1] ?? 0;
  return day <= length;
}

// The text of a date, time or both when it is written in the format of `type`, else "".
export function dateText(type: DateType, value: string): string {
  if (type === "date") return validDate(value) ? value : "";
  if (type === "time") return TIME.test(value) ? value : "";
  const [date = "", time = "", ...rest] = value.split("T");
  return rest.length === 0 && validDate(date) && TIME.test(time) ? value : "";
}

export const dateValue = (n: Inst, name: "value" | "min" | "max"): string =>
  dateText(dateType(n), text(n, name));

const HEX_COLOR = /^#[0-9a-fA-F]{6}$/;

// The colour a colour input shows: the value as lowercase `#rrggbb`, black when it is not one.
export function colorValue(n: Inst): string {
  const value = text(n, "value");
  return HEX_COLOR.test(value) ? value.toLowerCase() : "#000000";
}

export function options(n: Inst): Inst[] {
  return nodes(n.children).filter((c) => c.kind === "option");
}

// A single-select always has one option selected; with no match the browser picks the first.
export function selectedOption(n: Inst): { node: Inst; text: string } | undefined {
  const all = options(n);
  const value = text(n, "value");
  const hit = all.find((o) => text(o, "value") === value) ?? all[0];
  return hit ? { node: hit, text: label(hit) || contentText(hit.children) } : undefined;
}

// Index of the selected tab: the one whose id `selected` names, else the first.
export function selectedTab(tabs: Inst, list: Inst[]): number {
  const selected = text(tabs, "selected");
  const i = list.findIndex((t) => t.docId === selected || t.id === selected);
  return i < 0 ? 0 : i;
}

export const dialogOpen = (n: Inst): boolean => flag(n, "open");

export function radioChecked(radio: Inst, group: Inst | undefined): boolean {
  if (!group) return false;
  const value = text(group, "value");
  return value !== "" && text(radio, "value") === value;
}
