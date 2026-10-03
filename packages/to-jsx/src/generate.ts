// Weft document → a self-contained React component (SPEC §9, "To JSX"). The mapping restates
// `@weft/render-react` kind by kind rather than sharing it: the renderer builds elements with
// closures over resolved values, while this module prints source text, so a shared table would
// only cover tag names. test/equivalence.test.ts renders both on every corpus screen and catalog
// example and compares the results, which is what keeps the two from drifting.
//
// The document is untrusted. No document string ever reaches the output except through
// `JSON.stringify` (a JS string literal) or a JSX text/attribute run whose characters are all
// inert; identifiers come only from this module or are checked against the loop-variable grammar.
import {
  ARIA_ROLES,
  BINDING,
  LOOP_VARIABLE,
  TOKEN,
  type Catalog,
  type ComponentDef,
  type Document,
  type PropDef,
} from "@weft/core";
import { RUNTIME } from "./runtime.ts";

export type ToJsxOptions = { catalog: Catalog; componentName?: string };

// Same bound as the renderer's expansion: deeper content is not rendered.
export const MAX_DEPTH = 200;
const COMPONENT_NAME = /^[A-Z][A-Za-z0-9]*$/;

type Rec = Record<string, unknown>;
const isRecord = (v: unknown): v is Rec => typeof v === "object" && v !== null && !Array.isArray(v);

// ---- Source fragments ----

// A string built at run time: literal parts and JS expressions, joined with `+`.
type Part = string | { js: string };
type Str = Part[];

function strJs(s: Str): string {
  const merged: Part[] = [];
  for (const p of s) {
    const last = merged.at(-1);
    if (typeof p === "string" && typeof last === "string") merged[merged.length - 1] = last + p;
    else merged.push(p);
  }
  if (merged.length === 0) return '""';
  return merged.map((p) => (typeof p === "string" ? JSON.stringify(p) : p.js)).join(" + ");
}

// A value known at compile time (`lit`) or computed by `js` at run time; `bool` marks an
// expression that already yields a boolean.
type E = { js: string; lit?: { v: unknown }; bool?: boolean };

function jsLiteral(v: unknown): string {
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "number") {
    if (Number.isNaN(v)) return "NaN";
    if (!Number.isFinite(v)) return v > 0 ? "Infinity" : "-Infinity";
    return String(v);
  }
  if (typeof v === "boolean") return String(v);
  if (v === null) return "null";
  // A literal array or object is not a SPEC value; only its truthiness can matter.
  if (Array.isArray(v)) return "[]";
  return "undefined";
}

const lit = (v: unknown): E => ({ js: jsLiteral(v), lit: { v } });
const dyn = (js: string, bool = false): E => (bool ? { js, bool } : { js });

// Compile-time twins of the `_text`, `_on` and `_num` helpers (runtime.ts), used to fold
// literals.
function toText(v: unknown): string {
  if (typeof v === "string") return v;
  if (typeof v === "number") return Number.isFinite(v) ? String(v) : "";
  if (typeof v === "boolean") return String(v);
  return "";
}
const truthy = (v: unknown): boolean => (v === "false" ? false : Boolean(v));
function clampNumber(v: unknown, def: PropDef): unknown {
  if (typeof v !== "number") return v;
  if (!Number.isFinite(v)) return undefined;
  let out = def.integer === true ? Math.round(v) : v;
  if (typeof def.min === "number") out = Math.max(out, def.min);
  if (typeof def.max === "number") out = Math.min(out, def.max);
  return out;
}

// Element tree printed as JSX.
type Code = (indent: string) => string;
type Attr = [string, { s: string } | { js: string } | true];
type J = { tag: string; attrs: Attr[]; kids: C[] };
type C = J | { text: string } | { code: Code };

const isJ = (c: C): c is J => "tag" in c;

// Characters that mean nothing to the JSX lexer inside text or a quoted attribute.
const INERT = /^[\p{L}\p{N} .,:;!?'()/#%+=@_*~^|$-]*$/u;
const INERT_TEXT = /^[^ ](?:[^ ]| (?! ))*[^ ]$|^[^ ]$/u;

function printAttr([name, v]: Attr): string {
  if (v === true) return ` ${name}`;
  if ("s" in v) return INERT.test(v.s) ? ` ${name}="${v.s}"` : ` ${name}={${JSON.stringify(v.s)}}`;
  return ` ${name}={${v.js}}`;
}

function printText(text: string): string {
  return INERT.test(text) && INERT_TEXT.test(text) ? text : `{${JSON.stringify(text)}}`;
}

function printJ(j: J, indent: string): string {
  const open = `<${j.tag}${j.attrs.map(printAttr).join("")}`;
  const close = j.tag === "" ? "</>" : `</${j.tag}>`;
  if (j.kids.length === 0) return j.tag === "" ? "<></>" : `${open} />`;
  const [only] = j.kids;
  if (j.kids.length === 1 && only && "text" in only)
    return `${open}>${printText(only.text)}${close}`;
  const inner = `${indent}  `;
  const body = j.kids.map((k) => inner + printC(k, inner)).join("\n");
  return `${open}>\n${body}\n${indent}${close}`;
}

function printC(c: C, indent: string): string {
  if (isJ(c)) return printJ(c, indent);
  if ("text" in c) return printText(c.text);
  return `{${c.code(indent)}}`;
}

// An element or expression in expression position.
function expr(c: C, indent: string): string {
  if (isJ(c)) return `(\n${indent}  ${printJ(c, `${indent}  `)}\n${indent})`;
  if ("text" in c) return JSON.stringify(c.text);
  return c.code(indent);
}

function arrayCode(items: Code[]): Code {
  if (items.length === 0) return () => "[]";
  return (indent) =>
    `[\n${items.map((i) => `${indent}  ${i(`${indent}  `)},`).join("\n")}\n${indent}]`;
}

// `(() => { const …; return <…>; })()`, for elements that need values computed once.
function block(lines: ((indent: string) => string)[], result: C): C {
  return {
    code: (indent) => {
      const inner = `${indent}  `;
      const decls = lines.map((l) => `${inner}${l(inner)}\n`).join("");
      return `(() => {\n${decls}${inner}return ${expr(result, inner)};\n${indent}})()`;
    },
  };
}

// ---- Names ----

// A loop variable that would shadow the component's parameters, a keyword or a global the
// output relies on is renamed; loop variables never contain `_`, so the renamed form is free.
const RESERVED = new Set(
  (
    "arguments async await break case catch class const continue data debugger default delete " +
    "do else enum eval export extends false finally for function if implements import in " +
    "instanceof interface let new null of package private protected public return static super " +
    "switch this throw true try typeof undefined var void while with yield actions onChange"
  ).split(" "),
);
const ident = (name: string): string => (RESERVED.has(name) ? `${name}_` : name);

// Optional chaining would reach inherited members (`constructor`, `length`, `toString`) where
// the renderer reads own properties only; paths through such names use the `_get` helper.
const INHERITED = new Set([
  "__proto__",
  ...[Object, Array, String, Number, Boolean].flatMap((c) =>
    Object.getOwnPropertyNames(c.prototype),
  ),
]);
const PLAIN_SEGMENT = /^[A-Za-z_][A-Za-z0-9_]*$/;

// ---- Scope and pieces ----

type Var = { ident: string; path: Str };
type Scope = {
  vars: ReadonlyMap<string, Var>;
  suffix: Str;
  item: Str | undefined;
  loops: number;
};

type NodePiece = { t: "node"; n: N; hidden: string | undefined };
type Piece =
  | { t: "text"; s: string }
  | NodePiece
  | { t: "each"; src: string; ident: string; index: string; inner: Piece[] };

type N = {
  kind: string;
  def: ComponentDef | undefined;
  docId: string;
  id: Str;
  props: Rec;
  on: Record<string, string>;
  children: unknown;
  slots: Rec;
  scope: Scope;
  depth: number;
};

// `group` is the enclosing radio-group, `form` the nearest enclosing form.
type Ctx = { group: N | undefined; form: N | undefined };

class Gen {
  readonly used = new Set<string>();
  fragment = false;

  readonly catalog: Catalog;

  constructor(catalog: Catalog) {
    this.catalog = catalog;
  }

  use(helper: string): string {
    this.used.add(helper);
    if (helper === "_keyed") this.fragment = true;
    return helper;
  }

  // ---- Values (SPEC §2.1, as resolved by the renderer) ----

  resolve(raw: unknown, scope: Scope): { e: E; path?: Str } {
    if (!isRecord(raw)) return { e: lit(raw) };
    const bind = raw["bind"];
    if (typeof bind !== "string") return { e: lit(undefined) };
    if (!BINDING.test(bind)) return { e: lit(undefined) };
    const dot = bind.indexOf(".");
    const name = bind.slice(1, dot < 0 ? undefined : dot);
    const rest = dot < 0 ? "" : bind.slice(dot);
    const segments = rest === "" ? [] : rest.slice(1).split(".");
    let base: string;
    let path: Str;
    if (name === "") {
      base = "data";
      path = ["$"];
    } else {
      const v = scope.vars.get(name);
      if (!v) return { e: lit(undefined) };
      base = v.ident;
      path = v.path;
    }
    const access = segments.every((s) => PLAIN_SEGMENT.test(s) && !INHERITED.has(s))
      ? base + segments.map((s) => `?.${s}`).join("")
      : `${this.use("_get")}(${base}, ${JSON.stringify(segments)})`;
    if (raw["not"] === true) return { e: dyn(`!${this.use("_on")}(${access})`, true) };
    return { e: dyn(access), path: [...path, rest] };
  }

  text(e: E): E {
    return e.lit ? lit(toText(e.lit.v)) : dyn(`${this.use("_text")}(${e.js})`);
  }

  flag(e: E): E {
    if (e.lit) return lit(truthy(e.lit.v));
    return e.bool ? e : dyn(`${this.use("_on")}(${e.js})`, true);
  }

  // A number prop is brought into its declared range as the renderer's `prop` does (SPEC §5.1).
  prop(n: N, name: string): { e: E; path?: Str } {
    const resolved = this.resolve(n.props[name], n.scope);
    const def = n.def?.props && Object.hasOwn(n.def.props, name) ? n.def.props[name] : undefined;
    if (def?.type !== "number") return resolved;
    const { e } = resolved;
    if (e.lit) return { ...resolved, e: lit(clampNumber(e.lit.v, def)) };
    const bounds = [def.integer === true, def.min, def.max].map(jsLiteral).join(", ");
    return { ...resolved, e: dyn(`${this.use("_num")}(${e.js}, ${bounds})`) };
  }

  textOf = (n: N, name: string): E => this.text(this.prop(n, name).e);
  flagOf = (n: N, name: string): E => this.flag(this.prop(n, name).e);

  label(n: N): E {
    const t = this.textOf(n, "label");
    return t.lit ? lit(String(t.lit.v).trim()) : dyn(`${t.js}.trim()`);
  }

  // SPEC §5.1: a text-bearing kind takes its text from content or from the `text` prop. As in
  // render-react's expansion, the prop counts only when the content renders nothing (content wins
  // when a lenient reader is given both) and its text is not blank.
  textPropShown(n: N): { text: E; when: E } {
    const text = this.textOf(n, "text");
    if (n.props["text"] === undefined) return { text, when: lit(false) };
    const content = this.anyShown(this.pieces(n.children, n.scope, n.depth + 1));
    const blank = text.lit
      ? lit(toText(text.lit.v).trim() === "")
      : dyn(`${text.js}.trim() === ""`);
    if (content.lit?.v === true || blank.lit?.v === true) return { text, when: lit(false) };
    const parts = [content, blank].filter((e) => !e.lit).map((e) => e.js);
    return { text, when: parts.length === 0 ? lit(true) : dyn(`!(${parts.join(" || ")})`, true) };
  }

  // The visible text of a `text`-content kind, joined and squashed as `contentText` does.
  ownText(n: N): E {
    const content = lit(contentText(n.children));
    const { text, when } = this.textPropShown(n);
    if (when.lit) return when.lit.v === true ? this.squash(text) : content;
    return dyn(`${when.js} ? ${this.squash(text).js} : ${content.js}`);
  }

  squash(e: E): E {
    if (e.lit) return lit(toText(e.lit.v).trim().replace(/\s+/g, " "));
    return dyn(`${this.use("_squash")}(${e.js})`);
  }

  // Whether any of these pieces renders, i.e. whether the renderer's expanded list is non-empty.
  anyShown(list: Piece[]): E {
    const parts: string[] = [];
    for (const p of list) {
      if (p.t === "text" || (p.t === "node" && p.hidden === undefined)) return lit(true);
      if (p.t === "node") {
        parts.push(`!${p.hidden}`);
        continue;
      }
      const inner = this.anyShown(p.inner);
      if (inner.lit?.v === false) continue;
      parts.push(`${p.src}.some((${p.ident}, ${p.index}) => ${inner.js})`);
    }
    return parts.length === 0 ? lit(false) : dyn(parts.join(" || "), true);
  }

  // ---- Pieces: `each` and `hidden` as the renderer's expansion reads them ----

  pieces(list: unknown, scope: Scope, depth: number): Piece[] {
    const out: Piece[] = [];
    if (!Array.isArray(list) || depth > MAX_DEPTH) return out;
    for (const child of list) {
      if (typeof child === "string") {
        if (child.trim() === "") continue;
        const last = out.at(-1);
        // Adjacent text is one DOM text node; the renderer joins it with a space.
        if (last?.t === "text") last.s = `${last.s} ${child}`;
        else out.push({ t: "text", s: child });
        continue;
      }
      if (!isRecord(child) || typeof child["kind"] !== "string") continue;
      const props = isRecord(child["props"]) ? child["props"] : {};
      if (child["kind"] === "each") {
        const each = this.each(child, props, scope, depth);
        if (each) out.push(each);
        continue;
      }
      const hidden = this.flag(this.resolve(props["hidden"], scope).e);
      if (hidden.lit?.v === true) continue;
      out.push({
        t: "node",
        n: this.node(child, child["kind"], props, scope, depth),
        hidden: hidden.lit ? undefined : hidden.js,
      });
    }
    return out;
  }

  each(node: Rec, props: Rec, scope: Scope, depth: number): Piece | undefined {
    const source = this.resolve(props["in"], scope);
    const as = props["as"];
    if (source.path === undefined || typeof as !== "string" || !LOOP_VARIABLE.test(as))
      return undefined;
    const index = `_i${scope.loops}`;
    const path: Str = [...source.path, ".", { js: index }];
    const v: Var = { ident: ident(as), path };
    const inner: Scope = {
      vars: new Map(scope.vars).set(as, v),
      suffix: [...scope.suffix, "[", { js: index }, "]"],
      item: path,
      loops: scope.loops + 1,
    };
    return {
      t: "each",
      src: `${this.use("_list")}(${source.e.js})`,
      ident: v.ident,
      index,
      inner: this.pieces(node["children"], inner, depth + 1),
    };
  }

  node(raw: Rec, kind: string, props: Rec, scope: Scope, depth: number): N {
    const docId = typeof raw["id"] === "string" ? raw["id"] : "";
    const on: Record<string, string> = {};
    if (isRecord(raw["on"])) {
      for (const [event, action] of Object.entries(raw["on"])) {
        if (typeof action === "string") on[event] = action;
      }
    }
    return {
      kind,
      def: Object.hasOwn(this.catalog.components, kind) ? this.catalog.components[kind] : undefined,
      docId,
      id: docId === "" ? [] : [docId, ...scope.suffix],
      props,
      on,
      children: raw["children"],
      slots: isRecord(raw["slots"]) ? raw["slots"] : {},
      scope,
      depth,
    };
  }

  declaresSlot = (n: N, name: string): boolean =>
    n.def?.slots !== undefined && Object.hasOwn(n.def.slots, name);

  // A component's content as render.ts `content` places it (SPEC §4.2): the header slot first,
  // then the default content, then every other slot by name, each named slot in its own box. A
  // declared `empty` slot is left to the kind that shows it.
  content(n: N, ctx: Ctx): C[] {
    const before: C[] = [];
    const after: C[] = [];
    for (const name of Object.keys(n.slots).sort()) {
      if (name === "empty" && this.declaresSlot(n, name)) continue;
      const box: J = {
        tag: "div",
        attrs: [["data-weft-slot", { s: name }]],
        kids: this.kids(this.pieces(n.slots[name], n.scope, n.depth + 1), ctx),
      };
      (name === "header" ? before : after).push(box);
    }
    const own = n.def?.content === "text" || n.def?.content === "mixed" ? this.textProp(n) : [];
    const body = this.kids(this.pieces(n.children, n.scope, n.depth + 1), ctx);
    return [...before, ...own, ...body, ...after];
  }

  // Whether the default slot has something to show besides empty loops, as render-react's
  // `shows` decides it (SPEC §5.1 `empty`): a static element counts even when hidden, a loop
  // only when its array has items, and table columns never count.
  filled(n: N): E {
    const loops: string[] = [];
    for (const c of Array.isArray(n.children) ? n.children : []) {
      if (!isRecord(c) || typeof c["kind"] !== "string" || c["kind"] === "column") continue;
      if (c["kind"] !== "each") return lit(true);
      const source = this.resolve(isRecord(c["props"]) ? c["props"]["in"] : undefined, n.scope).e;
      if (!source.lit) loops.push(`${this.use("_list")}(${source.js}).length > 0`);
      else if (Array.isArray(source.lit.v) && source.lit.v.length > 0) return lit(true);
    }
    return loops.length === 0 ? lit(false) : dyn(loops.join(" || "), true);
  }

  // Whether a declared `empty` slot replaces the default content (render-react `showsEmpty`).
  showsEmpty(n: N): E {
    if (!this.declaresSlot(n, "empty") || n.slots["empty"] === undefined) return lit(false);
    const state = this.textOf(n, "state");
    const filled = this.filled(n);
    if (state.lit?.v === "empty" || filled.lit?.v === false) return lit(true);
    if (state.lit && filled.lit) return lit(false);
    const parts = [
      state.lit ? undefined : `${state.js} === "empty"`,
      filled.lit ? undefined : `!(${filled.js})`,
    ];
    return dyn(parts.filter(Boolean).join(" || "), true);
  }

  // Pieces as JSX children.
  kids(
    list: Piece[],
    ctx: Ctx,
    map?: (p: NodePiece | { t: "text"; s: string }) => C | undefined,
  ): C[] {
    const out: C[] = [];
    for (const p of list) {
      if (p.t === "text") {
        const c = map ? map(p) : { text: p.s };
        if (c) out.push(c);
      } else if (p.t === "node") {
        const c = map ? map(p) : this.render(p.n, ctx);
        if (!c) continue;
        const hidden = p.hidden;
        out.push(hidden === undefined ? c : { code: (i) => `!${hidden} && ${expr(c, i)}` });
      } else {
        this.fragment = true;
        const inner = this.kids(p.inner, ctx, map);
        const frag: J = { tag: "Fragment", attrs: [["key", { js: p.index }]], kids: inner };
        out.push({ code: (i) => `${p.src}.map((${p.ident}, ${p.index}) => ${expr(frag, i)})` });
      }
    }
    return out;
  }

  // Pieces as an array expression, keeping the entries `make` returns.
  collect(
    list: Piece[],
    make: (p: NodePiece | { t: "text"; s: string }) => Code | undefined,
  ): Code {
    const items: Code[] = [];
    for (const p of list) {
      if (p.t === "each") {
        const inner = this.collect(p.inner, make);
        items.push((i) => `...${p.src}.flatMap((${p.ident}, ${p.index}) => ${inner(i)})`);
        continue;
      }
      const item = make(p);
      if (!item) continue;
      const hidden = p.t === "node" ? p.hidden : undefined;
      items.push(hidden === undefined ? item : (i) => `...(${hidden} ? [] : [${item(i)}])`);
    }
    return arrayCode(items);
  }

  // ---- Attributes ----

  attr(attrs: Attr[], name: string, e: E): void {
    if (e.lit) {
      const v = e.lit.v;
      if (v === undefined || v === null) return;
      attrs.push([name, typeof v === "string" ? { s: v } : { js: jsLiteral(v) }]);
    } else attrs.push([name, { js: e.js }]);
  }

  // An HTML boolean attribute: present when true, absent otherwise.
  boolAttr(attrs: Attr[], name: string, e: E): void {
    if (!e.lit) attrs.push([name, { js: e.js }]);
    else if (e.lit.v === true) attrs.push([name, true]);
  }

  orUndefined(e: E): E {
    if (e.lit) return lit(e.lit.v === "" ? undefined : e.lit.v);
    return dyn(`${e.js} || undefined`);
  }

  // The document id and state on every element that carries a declared role (render.ts `base`).
  base(n: N, named = true): Attr[] {
    const a: Attr[] = [];
    if (n.docId !== "")
      a.push([
        "data-weft-id",
        n.id.every((p) => typeof p === "string") ? { s: n.id.join("") } : { js: strJs(n.id) },
      ]);
    const state = this.textOf(n, "state");
    this.attr(a, "data-state", this.orUndefined(state));
    if (state.lit) {
      if (["loading", "busy", "submitting"].includes(String(state.lit.v)))
        a.push(["aria-busy", { s: "true" }]);
    } else a.push(["aria-busy", { js: `${this.use("_busy")}(${state.js})` }]);
    if (named) this.attr(a, "aria-label", this.orUndefined(this.label(n)));
    return a;
  }

  fire(n: N, event: string): string | undefined {
    const action = n.on[event];
    if (action === undefined) return undefined;
    const item = n.scope.item ? `, item: ${strJs(n.scope.item)}` : "";
    return `${this.use("_act")}(actions, { id: ${strJs(n.id)}, action: ${JSON.stringify(action)}${item} })`;
  }

  write(n: N, name: string, value: string): string | undefined {
    const path = this.prop(n, name).path;
    return path === undefined ? undefined : `onChange?.(${strJs(path)}, ${value})`;
  }

  handler(params: string, statements: (string | undefined)[]): { js: string } {
    const body = statements.filter((s): s is string => s !== undefined);
    if (body.length === 0) return { js: `${params} => {}` };
    if (body.length === 1) return { js: `${params} => ${body[0]}` };
    return { js: `${params} => { ${body.join("; ")}; }` };
  }

  pressable(n: N, attrs: Attr[]): void {
    const fire = this.fire(n, "press");
    if (fire === undefined) return;
    attrs.push(
      ["tabIndex", { js: "0" }],
      ["onClick", { js: `() => ${fire}` }],
      ["onKeyDown", { js: `(_e) => ${this.use("_press")}(_e, () => ${fire})` }],
    );
  }

  // ---- Kinds (render.ts `renderNode`) ----

  render(n: N, ctx: Ctx): C | undefined {
    if (!n.def) return this.fallback(n, ctx);
    const el = (tag: string, attrs: Attr[], kids: C[]): J => ({ tag, attrs, kids });
    switch (n.kind) {
      case "screen":
        return el("main", this.base(n), this.content(n, ctx));
      case "stack":
      case "grid":
        return el(
          "div",
          [...this.base(n, false), ["style", { js: this.layoutStyle(n) }]],
          this.content(n, ctx),
        );
      case "section":
        return el("section", this.base(n), this.content(n, ctx));
      case "heading":
        return this.heading(n);
      case "text": {
        const a = this.base(n, false);
        this.attr(a, "data-tone", this.orUndefined(this.textOf(n, "tone")));
        return el("div", a, this.textKids(this.ownText(n)));
      }
      case "image": {
        const a = this.base(n, false);
        this.attr(a, "alt", this.label(n));
        a.push(["src", { js: `${this.use("_url")}(${this.textOf(n, "src").js})` }]);
        return el("img", a, []);
      }
      case "link":
        return this.link(n);
      case "button":
        return this.button(n, ctx);
      case "form": {
        const a = this.base(n);
        a.push(["onSubmit", this.handler("(_e)", ["_e.preventDefault()", this.fire(n, "submit")])]);
        return el("form", a, this.content(n, { ...ctx, form: n }));
      }
      case "field":
        return this.field(n);
      case "checkbox":
      case "switch":
        return this.toggle(n);
      case "radio-group":
        return el(
          "div",
          [...this.base(n), ["role", { s: "radiogroup" }]],
          [...this.caption(this.label(n)), ...this.content(n, { ...ctx, group: n })],
        );
      case "radio":
        return this.radio(n, ctx);
      case "select":
        return this.select(n);
      case "option":
        return el("div", this.base(n, false), this.textKids(this.ownText(n)));
      case "list":
        return this.list(n, ctx);
      case "item": {
        const a = this.base(n);
        this.pressable(n, a);
        return el("li", a, this.content(n, ctx));
      }
      case "table":
        return this.table(n, ctx);
      case "tabs":
        return this.tabs(n, ctx);
      case "tab":
        return el("div", [...this.base(n), ["role", { s: "group" }]], this.content(n, ctx));
      case "dialog":
        return this.dialog(n, ctx);
      case "alert": {
        const a = [...this.base(n), ["role", { s: "alert" }] as Attr];
        this.attr(a, "data-tone", this.orUndefined(this.textOf(n, "tone")));
        return el("div", a, this.content(n, ctx));
      }
      case "menu":
        return el("div", [...this.base(n), ["role", { s: "menu" }]], this.content(n, ctx));
      case "menu-item": {
        const a: Attr[] = [...this.base(n), ["type", { s: "button" }], ["role", { s: "menuitem" }]];
        this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
        const fire = this.fire(n, "press");
        if (fire) a.push(["onClick", { js: `() => ${fire}` }]);
        return el("button", a, this.textKids(this.ownText(n)));
      }
      default:
        return this.fallback(n, ctx);
    }
  }

  textKids(e: E): C[] {
    if (e.lit) return e.lit.v === "" ? [] : [{ text: String(e.lit.v) }];
    return [{ code: () => e.js }];
  }

  // The visible caption of a control (render.ts `caption`): the name itself is `aria-label`, so
  // the caption is hidden from the tree rather than read twice, and absent when there is none.
  caption(name: E): C[] {
    const span: J = {
      tag: "span",
      attrs: [["aria-hidden", { s: "true" }]],
      kids: this.textKids(name),
    };
    if (name.lit) return name.lit.v === "" ? [] : [span];
    return [{ code: (i) => `${name.js} ? ${expr(span, i)} : null` }];
  }

  // The `text` prop as raw content, where a kind's content is rendered as child nodes (render.ts
  // `kids`) rather than joined like `contentText`.
  textProp(n: N): C[] {
    const { text, when } = this.textPropShown(n);
    if (when.lit) return when.lit.v === true ? this.textKids(text) : [];
    return [{ code: () => `${when.js} ? ${text.js} : null` }];
  }

  // Unknown kinds and extensions keep their children under their fallback role (SPEC §8).
  fallback(n: N, ctx: Ctx): C {
    const declared = n.def ? n.def.role : n.props["role"];
    const role = typeof declared === "string" && ARIA_ROLES.includes(declared) ? declared : "group";
    const kids = this.content(n, ctx);
    if (role === "none" || role === "presentation" || role === "generic") {
      return { tag: "div", attrs: this.base(n, false), kids };
    }
    const a = this.base(n);
    if (role === "region" || role === "form") {
      // Unnamed, both are generic (HTML-AAM), so the role is kept only with a label.
      const label = this.label(n);
      if (label.lit) {
        if (label.lit.v !== "") a.push(["role", { s: role }]);
      } else a.push(["role", { js: `${label.js} ? ${JSON.stringify(role)} : undefined` }]);
    } else a.push(["role", { s: role }]);
    return { tag: "div", attrs: a, kids };
  }

  layoutStyle(n: N): string {
    const entries: string[] = [];
    const gap = n.props["gap"];
    if (isRecord(gap) && typeof gap["token"] === "string" && TOKEN.test(gap["token"])) {
      entries.push(`gap: ${JSON.stringify(`var(--weft-${gap["token"].replaceAll(".", "-")})`)}`);
    }
    if (n.kind === "grid") {
      entries.push('display: "grid"');
      const columns = this.prop(n, "columns").e;
      if (columns.lit) {
        const c = columns.lit.v;
        if (typeof c === "number" && Number.isInteger(c) && c >= 1 && c <= 64) {
          entries.push(`gridTemplateColumns: ${JSON.stringify(`repeat(${c}, minmax(0, 1fr))`)}`);
        }
      } else entries.push(`gridTemplateColumns: ${this.use("_cols")}(${columns.js})`);
      return `{ ${entries.join(", ")} }`;
    }
    entries.push('display: "flex"');
    const direction = this.textOf(n, "direction");
    entries.push(
      direction.lit
        ? `flexDirection: ${direction.lit.v === "row" ? '"row"' : '"column"'}`
        : `flexDirection: ${direction.js} === "row" ? "row" : "column"`,
    );
    const align = this.textOf(n, "align");
    if (align.lit) {
      const key = String(align.lit.v);
      if (Object.hasOwn(ALIGN, key)) entries.push(`alignItems: ${JSON.stringify(ALIGN[key])}`);
    } else entries.push(`alignItems: ${this.use("_align")}(${align.js})`);
    const wrap = this.flagOf(n, "wrap");
    if (!wrap.lit) entries.push(`flexWrap: ${wrap.js} ? "wrap" : undefined`);
    else if (wrap.lit.v === true) entries.push('flexWrap: "wrap"');
    return `{ ${entries.join(", ")} }`;
  }

  // An element whose tag is chosen at run time.
  withTag(tagJs: string, j: J): C {
    return block([() => `const Tag = ${tagJs};`], { ...j, tag: "Tag" });
  }

  heading(n: N): C {
    const j: J = { tag: "", attrs: this.base(n), kids: this.textKids(this.ownText(n)) };
    const level = this.prop(n, "level").e;
    if (!level.lit) return this.withTag(`"h" + ${this.use("_level")}(${level.js})`, j);
    const v = level.lit.v;
    const num = typeof v === "string" ? Number(v) : v;
    const h = typeof num === "number" && Number.isInteger(num) && num >= 1 && num <= 6 ? num : 2;
    return { ...j, tag: `h${h}` };
  }

  link(n: N): C {
    const fire = this.fire(n, "press");
    const a: Attr[] = [...this.base(n), ["href", { js: "_href" }]];
    // Without an href an <a> is neither a link nor focusable; both are restored, and like a
    // native link it then activates on Enter only.
    a.push(["role", { js: '_href === undefined ? "link" : undefined' }]);
    a.push(["tabIndex", { js: "_href === undefined ? 0 : undefined" }]);
    if (fire) {
      a.push([
        "onKeyDown",
        {
          js: `_href === undefined ? (_e) => { if (_e.key !== "Enter" || _e.target !== _e.currentTarget) return; _e.preventDefault(); ${fire}; } : undefined`,
        },
      ]);
      a.push(["onClick", { js: `() => ${fire}` }]);
    }
    a.push(["style", { js: '{ display: "inline-block" }' }]);
    const href = `${this.use("_url")}(${this.textOf(n, "href").js})`;
    return block([() => `const _href = ${href};`], {
      tag: "a",
      attrs: a,
      kids: this.textKids(this.ownText(n)),
    });
  }

  button(n: N, ctx: Ctx): C {
    // `submit` is literal-only (SPEC §5.1), so a binding never turns a button into a submit button.
    const submit = n.props["submit"] === true || n.props["submit"] === "true";
    const a: Attr[] = [...this.base(n), ["type", { s: submit ? "submit" : "button" }]];
    this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
    this.attr(a, "data-variant", this.orUndefined(this.textOf(n, "variant")));
    // As in render.ts, the browser's own submission (also from Enter in a field) is cancelled and
    // reported here, so press and the form's submit each fire exactly once, in that order.
    const form = submit ? ctx.form : undefined;
    const click = [
      submit ? "_e.preventDefault()" : undefined,
      this.fire(n, "press"),
      form ? this.fire(form, "submit") : undefined,
    ];
    if (click.some(Boolean)) a.push(["onClick", this.handler("(_e)", click)]);
    return { tag: "button", attrs: a, kids: this.textKids(this.ownText(n)) };
  }

  field(n: N): C {
    const name = this.label(n);
    const error = this.textOf(n, "error");
    const type = this.textOf(n, "type");
    const errorId = strJs(["weft-", ...n.id, "-error"]);
    const value = this.textOf(n, "value");
    const a = this.base(n, false);
    this.attr(a, "aria-label", name);
    let tag: string | undefined;
    if (type.lit) {
      const t = String(type.lit.v);
      tag = t === "multiline" ? "textarea" : "input";
      if (t !== "multiline")
        a.push([
          "type",
          { s: ["text", "email", "password", "number", "search"].includes(t) ? t : "text" },
        ]);
    } else {
      a.push([
        "type",
        {
          js: `${type.js} === "multiline" ? undefined : ["text", "email", "password", "number", "search"].includes(${type.js}) ? ${type.js} : "text"`,
        },
      ]);
    }
    this.attr(a, "placeholder", this.orUndefined(this.textOf(n, "placeholder")));
    this.boolAttr(a, "required", this.flagOf(n, "required"));
    this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
    const state = this.textOf(n, "state");
    if (state.lit && error.lit) {
      if (state.lit.v === "invalid" || error.lit.v !== "") a.push(["aria-invalid", { s: "true" }]);
      if (error.lit.v !== "") a.push(["aria-describedby", { js: errorId }]);
    } else {
      a.push([
        "aria-invalid",
        { js: `${state.js} === "invalid" || ${error.js} !== "" ? "true" : undefined` },
      ]);
      a.push(["aria-describedby", { js: `${error.js} ? ${errorId} : undefined` }]);
    }
    const change = this.fire(n, "change");
    if (isBinding(n.props["value"])) {
      // A number input shows "" for anything that is not a valid float, so the value is
      // sanitized the same way before React sees it.
      const shown = type.lit
        ? type.lit.v === "number"
          ? `${this.use("_float")}(${value.js})`
          : value.js
        : `${type.js} === "number" ? ${this.use("_float")}(${value.js}) : ${value.js}`;
      a.push(["value", { js: shown }]);
      a.push([
        "onChange",
        this.handler("(_e)", [this.write(n, "value", "_e.currentTarget.value"), change]),
      ]);
    } else {
      this.attr(a, "defaultValue", value);
      if (change) a.push(["onChange", { js: `() => ${change}` }]);
    }
    const control: J = { tag: tag ?? "Tag", attrs: a, kids: [] };
    const controlC = tag
      ? control
      : this.withTag(`${type.js} === "multiline" ? "textarea" : "input"`, control);
    const errorEl: J = { tag: "div", attrs: [["id", { js: errorId }]], kids: this.textKids(error) };
    const kids: C[] = [
      {
        tag: "label",
        attrs: [],
        kids: [...this.caption(name), controlC],
      },
    ];
    if (error.lit) {
      if (error.lit.v !== "") kids.push(errorEl);
    } else kids.push({ code: (i) => `${error.js} ? ${expr(errorEl, i)} : null` });
    return { tag: "div", attrs: [["data-weft-field", { s: "" }]], kids };
  }

  toggle(n: N): C {
    const name = this.label(n);
    const a = this.base(n, false);
    a.push(["type", { s: "checkbox" }]);
    if (n.kind === "switch") a.push(["role", { s: "switch" }]);
    this.attr(a, "aria-label", name);
    this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
    const checked = this.flagOf(n, "checked");
    const change = this.fire(n, "change");
    if (isBinding(n.props["checked"])) {
      a.push(["checked", { js: checked.js }]);
      a.push([
        "onChange",
        this.handler("(_e)", [this.write(n, "checked", "_e.currentTarget.checked"), change]),
      ]);
    } else {
      this.boolAttr(a, "defaultChecked", checked);
      if (change) a.push(["onChange", { js: `() => ${change}` }]);
    }
    return {
      tag: "label",
      attrs: [],
      kids: [{ tag: "input", attrs: a, kids: [] }, ...this.caption(name)],
    };
  }

  radio(n: N, ctx: Ctx): C {
    const group = ctx.group;
    const label = this.label(n);
    const own = this.ownText(n);
    const name: E =
      label.lit && own.lit ? lit(label.lit.v || own.lit.v) : dyn(`${label.js} || ${own.js}`);
    const shown: E =
      label.lit && own.lit ? lit(own.lit.v || label.lit.v) : dyn(`${own.js} || ${label.js}`);
    const value = this.textOf(n, "value");
    const a = this.base(n, false);
    a.push(["type", { s: "radio" }]);
    if (group && group.docId !== "") a.push(["name", { js: strJs(["weft-", ...group.id]) }]);
    this.attr(a, "value", value);
    this.attr(a, "aria-label", name);
    this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
    let on: E = lit(false);
    if (group) {
      const gv = this.textOf(group, "value");
      on =
        gv.lit && value.lit
          ? lit(gv.lit.v !== "" && gv.lit.v === value.lit.v)
          : dyn(`${gv.js} !== "" && ${value.js} === ${gv.js}`, true);
    }
    const controlled = group !== undefined && isBinding(group.props["value"]);
    if (controlled) a.push(["checked", { js: on.js }]);
    else this.boolAttr(a, "defaultChecked", on);
    if (group) {
      const statements = [
        this.write(group, "value", this.prop(n, "value").e.js),
        this.fire(group, "change"),
      ];
      if (controlled || statements.some(Boolean))
        a.push(["onChange", this.handler("()", statements)]);
    }
    return {
      tag: "label",
      attrs: [],
      kids: [
        { tag: "input", attrs: a, kids: [] },
        // The visible text is the content; a `label` only overrides the accessible name.
        { tag: "span", attrs: [["aria-hidden", { s: "true" }]], kids: this.textKids(shown) },
      ],
    };
  }

  // Only options are rendered: the HTML parser drops anything else inside <select>.
  select(n: N): C {
    const options = this.collect(this.pieces(n.children, n.scope, n.depth + 1), (p) => {
      if (p.t !== "node" || p.n.kind !== "option") return undefined;
      const o = p.n;
      const a = this.base(o);
      this.attr(a, "value", this.textOf(o, "value"));
      const el: J = { tag: "option", attrs: a, kids: this.textKids(this.ownText(o)) };
      return (i) => `{ value: ${this.prop(o, "value").e.js}, el: ${expr(el, i)} }`;
    });
    const a = this.base(n, false);
    this.attr(a, "aria-label", this.label(n));
    this.boolAttr(a, "disabled", this.flagOf(n, "disabled"));
    const value = this.textOf(n, "value");
    const controlled = isBinding(n.props["value"]);
    if (controlled) a.push(["value", { js: value.js }]);
    else this.attr(a, "defaultValue", value);
    const statements = [
      this.write(n, "value", `_c ? _c.value : _e.currentTarget.value`),
      this.fire(n, "change"),
    ];
    if (statements[0]) {
      const text = this.use("_text");
      a.push([
        "onChange",
        this.handler("(_e)", [
          `const _c = _o.find((_x) => ${text}(_x.value) === _e.currentTarget.value)`,
          ...statements,
        ]),
      ]);
    } else if (controlled || statements[1])
      a.push(["onChange", this.handler("()", [statements[1]])]);
    const list = this.use("_keyed");
    const control = block([(i) => `const _o = ${options(i)};`], {
      tag: "select",
      attrs: a,
      kids: [{ code: () => `${list}(_o.map((_x) => _x.el))` }],
    });
    return { tag: "label", attrs: [], kids: [...this.caption(this.label(n)), control] };
  }

  // SPEC §5.1: the `empty` slot is shown instead of the items when there are none, or when the
  // state says `empty`.
  list(n: N, ctx: Ctx): C {
    const ordered = this.flagOf(n, "ordered");
    const content = this.content(n, ctx);
    const show = this.showsEmpty(n);
    let kids = content;
    if (show.lit?.v !== false) {
      // A list may only hold list items, so the empty content stands in a presentational one.
      const empty: J = {
        tag: "li",
        attrs: [
          ["role", { s: "none" }],
          ["data-weft-slot", { s: "empty" }],
        ],
        kids: this.kids(this.pieces(n.slots["empty"], n.scope, n.depth + 1), ctx),
      };
      const items: J = { tag: "", attrs: [], kids: content };
      kids = show.lit
        ? [empty]
        : [{ code: (i) => `${show.js} ? ${expr(empty, i)} : ${expr(items, i)}` }];
    }
    const j: J = { tag: "", attrs: this.base(n), kids };
    if (ordered.lit) return { ...j, tag: ordered.lit.v ? "ol" : "ul" };
    return this.withTag(`${ordered.js} ? "ol" : "ul"`, j);
  }

  // SPEC §5.1: `column` children form the header row in <thead>; other content goes to <tbody>,
  // wrapped in a row and cell when it is not a row.
  table(n: N, ctx: Ctx): C {
    const children = this.pieces(n.children, n.scope, n.depth + 1);
    const columns = this.collect(children, (p) => {
      if (p.t !== "node" || p.n.kind !== "column") return undefined;
      const c = this.column(p.n);
      return (i) => expr(c, i);
    });
    const rows = this.collect(children, (p) => {
      if (p.t === "node" && p.n.kind === "column") return undefined;
      const c: C | undefined =
        p.t === "text"
          ? { tag: "tr", attrs: [], kids: [{ tag: "td", attrs: [], kids: [{ text: p.s }] }] }
          : p.n.kind === "row"
            ? this.row(p.n, ctx)
            : this.cellWrap(this.render(p.n, ctx));
      return c ? (i) => expr(c, i) : undefined;
    });
    const keyed = this.use("_keyed");
    const head: J = {
      tag: "thead",
      attrs: [],
      kids: [{ tag: "tr", attrs: [], kids: [{ code: () => `${keyed}(_cols)` }] }],
    };
    const body: J = { tag: "tbody", attrs: [], kids: [{ code: () => `${keyed}(_rows)` }] };
    const kids: C[] = [{ code: (i) => `_cols.length > 0 ? ${expr(head, i)} : null` }];
    const show = this.showsEmpty(n);
    if (show.lit?.v === false)
      kids.push({ code: (i) => `_rows.length > 0 ? ${expr(body, i)} : null` });
    else {
      // A table keeps its column headers; the empty content spans them in one body row.
      const empty: J = {
        tag: "tbody",
        attrs: [],
        kids: [
          {
            tag: "tr",
            attrs: [["data-weft-slot", { s: "empty" }]],
            kids: [
              {
                tag: "td",
                attrs: [["colSpan", { js: "Math.max(1, _cols.length)" }]],
                kids: this.kids(this.pieces(n.slots["empty"], n.scope, n.depth + 1), ctx),
              },
            ],
          },
        ],
      };
      kids.push(
        show.lit
          ? empty
          : {
              code: (i) =>
                `${show.js} ? ${expr(empty, i)} : _rows.length > 0 ? ${expr(body, i)} : null`,
            },
      );
    }
    return block([(i) => `const _cols = ${columns(i)};`, (i) => `const _rows = ${rows(i)};`], {
      tag: "table",
      attrs: this.base(n),
      kids,
    });
  }

  cellWrap(c: C | undefined): C | undefined {
    return c ? { tag: "tr", attrs: [], kids: [{ tag: "td", attrs: [], kids: [c] }] } : undefined;
  }

  column(n: N): J {
    const a = [...this.base(n), ["scope", { s: "col" }] as Attr];
    const sort = this.textOf(n, "sort");
    if (sort.lit) {
      if (["none", "ascending", "descending"].includes(String(sort.lit.v)))
        a.push(["aria-sort", { s: String(sort.lit.v) }]);
    } else
      a.push([
        "aria-sort",
        { js: `["none", "ascending", "descending"].includes(${sort.js}) ? ${sort.js} : undefined` },
      ]);
    this.pressable(n, a);
    return { tag: "th", attrs: a, kids: this.textKids(this.ownText(n)) };
  }

  row(n: N, ctx: Ctx): J {
    const a = this.base(n);
    if (n.props["selected"] !== undefined) {
      const selected = this.flagOf(n, "selected");
      a.push([
        "aria-selected",
        selected.lit ? { s: String(selected.lit.v) } : { js: `String(${selected.js})` },
      ]);
    }
    this.pressable(n, a);
    // Content that is not a cell is wrapped in one so the HTML parser keeps it in the row.
    const kids = this.kids(this.pieces(n.children, n.scope, n.depth + 1), ctx, (p) => {
      if (p.t === "text") return { tag: "td", attrs: [], kids: [{ text: p.s }] };
      if (p.n.kind === "cell") return this.cell(p.n, ctx);
      const c = this.render(p.n, ctx);
      return c ? { tag: "td", attrs: [], kids: [c] } : undefined;
    });
    return { tag: "tr", attrs: a, kids };
  }

  cell(n: N, ctx: Ctx): J {
    return {
      tag: "td",
      attrs: this.base(n),
      kids: this.content(n, ctx),
    };
  }

  // SPEC §5.1: one `tabs` becomes a tablist of tab buttons followed by one tabpanel per tab, of
  // which only the selected one is shown.
  tabs(n: N, ctx: Ctx): C {
    const children = this.pieces(n.children, n.scope, n.depth + 1);
    const tabs = this.collect(children, (p) => {
      if (p.t !== "node" || p.n.kind !== "tab") return undefined;
      const t = p.n;
      const panel: J = { tag: "", attrs: [], kids: this.content(t, ctx) };
      const state = this.textOf(t, "state");
      return (i) =>
        `{ id: ${strJs(t.id)}, doc: ${JSON.stringify(t.docId)}, state: ${state.js}, label: ${this.label(t).js}, panel: ${expr(panel, i)} }`;
    });
    const other = this.kids(
      filterPieces(children, (p) => p.t === "text" || p.n.kind !== "tab"),
      ctx,
    );
    const choose = this.handler("(_k)", [
      "const _x = _t[_k]",
      "if (!_x || _k === _s) return",
      this.write(n, "selected", "_x.doc"),
      this.fire(n, "change"),
    ]);
    const selected = this.textOf(n, "selected");
    const button: J = {
      tag: "button",
      attrs: [
        ["key", { js: "_k" }],
        ["data-weft-id", { js: "_x.id || undefined" }],
        ["data-state", { js: "_x.state || undefined" }],
        ["aria-busy", { js: `${this.use("_busy")}(_x.state)` }],
        ["type", { s: "button" }],
        ["role", { s: "tab" }],
        ["id", { js: '"weft-" + _x.id + "-tab"' }],
        ["aria-selected", { js: "String(_k === _s)" }],
        ["aria-controls", { js: '"weft-" + _x.id + "-panel"' }],
        ["tabIndex", { js: "_k === _s ? 0 : -1" }],
        ["onClick", { js: "() => _choose(_k)" }],
        ["onKeyDown", { js: `(_e) => ${this.use("_tabKey")}(_e, _k, _t.length, _choose)` }],
      ],
      kids: [{ code: () => "_x.label" }],
    };
    const panel: J = {
      tag: "div",
      attrs: [
        ["key", { js: "_k" }],
        ["role", { s: "tabpanel" }],
        ["id", { js: '"weft-" + _x.id + "-panel"' }],
        ["aria-labelledby", { js: '"weft-" + _x.id + "-tab"' }],
        ["hidden", { js: "_k !== _s" }],
        ["tabIndex", { js: "0" }],
      ],
      kids: [{ code: () => "_x.panel" }],
    };
    const tablist: J = {
      tag: "div",
      attrs: [...this.base(n), ["role", { s: "tablist" }]],
      kids: [{ code: (i) => `_t.map((_x, _k) => ${expr(button, i)})` }, ...other],
    };
    return block(
      [
        (i) => `const _t = ${tabs(i)};`,
        () =>
          `const _s = Math.max(0, _t.findIndex((_x) => _x.doc === ${selected.js} || _x.id === ${selected.js}));`,
        () => `const _choose = ${choose.js};`,
      ],
      {
        tag: "div",
        attrs: [["data-weft-tabs", { s: "" }]],
        kids: [tablist, { code: (i) => `_t.map((_x, _k) => ${expr(panel, i)})` }],
      },
    );
  }

  dialog(n: N, ctx: Ctx): C | undefined {
    const open = this.flagOf(n, "open");
    if (open.lit && open.lit.v !== true) return undefined;
    const a = this.base(n);
    a.push(["open", true]);
    const modal = n.props["modal"] === undefined ? lit(true) : this.flagOf(n, "modal");
    if (modal.lit) {
      if (modal.lit.v === true) a.push(["aria-modal", { s: "true" }]);
    } else a.push(["aria-modal", { js: `${modal.js} ? "true" : undefined` }]);
    a.push([
      "onKeyDown",
      this.handler("(_e)", [
        'if (_e.key !== "Escape") return',
        "_e.preventDefault()",
        this.write(n, "open", "false"),
        this.fire(n, "close"),
      ]),
    ]);
    const el: J = { tag: "dialog", attrs: a, kids: this.content(n, ctx) };
    return open.lit ? el : { code: (i) => `${open.js} && ${expr(el, i)}` };
  }
}

// Strings of a `text`-content kind, joined as the renderer's `contentText` joins them.
function contentText(list: unknown): string {
  if (!Array.isArray(list)) return "";
  const parts: string[] = [];
  for (const c of list) if (typeof c === "string" && c.trim() !== "") parts.push(c.trim());
  return parts.join(" ").replace(/\s+/g, " ");
}

const ALIGN: Record<string, string> = {
  start: "flex-start",
  center: "center",
  end: "flex-end",
  stretch: "stretch",
};

const isBinding = (raw: unknown): boolean => isRecord(raw) && typeof raw["bind"] === "string";

function filterPieces(
  list: Piece[],
  keep: (p: NodePiece | { t: "text"; s: string }) => boolean,
): Piece[] {
  const out: Piece[] = [];
  for (const p of list) {
    if (p.t !== "each") {
      if (keep(p)) out.push(p);
      continue;
    }
    const inner = filterPieces(p.inner, keep);
    if (inner.length > 0) out.push({ ...p, inner });
  }
  return out;
}

export function toJsx(document: Document | unknown, options: ToJsxOptions): string {
  const name = options.componentName ?? "WeftScreen";
  if (!COMPONENT_NAME.test(name)) {
    throw new TypeError(`componentName must match ${String(COMPONENT_NAME)}`);
  }
  const gen = new Gen(options.catalog);
  const scope: Scope = { vars: new Map(), suffix: [], item: undefined, loops: 0 };
  const root = isRecord(document) ? document["root"] : undefined;
  const first = gen.pieces([root], scope, 0).find((p): p is NodePiece => p.t === "node");
  let body = "null";
  if (first) {
    const c = gen.render(first.n, { group: undefined, form: undefined });
    if (c) {
      const hidden = first.hidden;
      body = hidden === undefined ? expr(c, "  ") : `${hidden} ? null : ${expr(c, "  ")}`;
    }
  }
  const helpers = Object.keys(RUNTIME)
    .filter((h) => gen.used.has(h))
    .map((h) => RUNTIME[h]);
  const imports = gen.fragment ? 'import { Fragment } from "react";\n\n' : "";
  const component = `export default function ${name}({ data, actions, onChange }) {\n  return ${body};\n}\n`;
  return `${imports}${helpers.map((h) => `${h}\n\n`).join("")}${component}`;
}
