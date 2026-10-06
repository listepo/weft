// HTML and JSX describe the same semantic markup, so both front-ends build this small element
// tree and share one conversion into the neutral tree.
import { collapse, node, normPath } from "./types.ts";
import type { NNode } from "../neutral.ts";

export interface HEl {
  tag: string;
  attrs: Record<string, string>;
  /** Bound attributes by name: "value", "disabled", "text", ...; a leading "!" negates. */
  bind: Record<string, string>;
  on: Record<string, string>;
  children: HChild[];
}
export type HChild = HEl | string | { bindText: string };

const isEl = (c: HChild): c is HEl => typeof c === "object" && "tag" in c;
const isBindText = (c: HChild): c is { bindText: string } =>
  typeof c === "object" && "bindText" in c;

function walkEls(el: HEl, visit: (e: HEl) => void): void {
  visit(el);
  for (const c of el.children) if (isEl(c)) walkEls(c, visit);
}

interface Text {
  name?: string;
  nameBind?: string;
}

const CONTROLS = new Set(["input", "select", "textarea"]);
const TEXT_TAGS = new Set(["p", "span", "strong", "em", "small"]);
const NAMED_BY_TEXT = new Set([
  "heading",
  "text",
  "link",
  "button",
  "column",
  "option",
  "menu-item",
  "tab",
]);

export function htmlLikeToNeutral(root: HEl): NNode {
  const byId = new Map<string, HEl>();
  const panels = new Map<string, HEl>();
  const labelOf = new Map<HEl, Text>();
  walkEls(root, (e) => {
    if (e.attrs.id) byId.set(e.attrs.id, e);
    if (e.attrs.role === "tabpanel" && e.attrs["aria-labelledby"])
      panels.set(e.attrs["aria-labelledby"], e);
  });

  const textOf = (el: HEl, loops: string[], skipControls = false): Text => {
    const parts: string[] = [];
    let nameBind: string | undefined;
    const visit = (e: HEl) => {
      if (e.bind.text) nameBind = ref(e.bind.text, loops);
      for (const c of e.children) {
        if (typeof c === "string") parts.push(c);
        else if (isBindText(c)) nameBind = ref(c.bindText, loops);
        else if (!(skipControls && CONTROLS.has(c.tag))) visit(c);
      }
    };
    visit(el);
    const name = collapse(parts.join(" "));
    return { ...(name ? { name } : {}), ...(nameBind ? { nameBind } : {}) };
  };

  const ref = (raw: string, loops: string[]): string =>
    (raw.startsWith("!") ? "!" : "") + normPath(raw.replace(/^!/, ""), loops);

  walkEls(root, (e) => {
    if (e.tag !== "label") return;
    let target: HEl | undefined = e.attrs.for ? byId.get(e.attrs.for) : undefined;
    if (!target) walkEls(e, (x) => (target ??= CONTROLS.has(x.tag) ? x : undefined));
    // Loop variables are unknown here; label bindings are re-normalised when the control converts.
    if (target) labelOf.set(target, textOf(e, [], true));
  });

  const kindOf = (el: HEl): string | null => {
    const role = el.attrs.role;
    const byRole: Record<string, string> = {
      alert: "alert",
      menu: "menu",
      menuitem: "menu-item",
      tablist: "tabs",
      tab: "tab",
      radiogroup: "radio-group",
      switch: "switch",
      dialog: "dialog",
    };
    if (role && byRole[role]) return byRole[role] as string;
    const t = el.tag;
    if (/^h[1-6]$/.test(t)) return "heading";
    if (TEXT_TAGS.has(t)) return "text";
    switch (t) {
      case "main":
        return "screen";
      case "section":
        return "section";
      case "form":
        return "form";
      case "a":
        return "link";
      case "button":
        return "button";
      case "input": {
        const type = el.attrs.type ?? "text";
        return type === "checkbox" ? "checkbox" : type === "radio" ? "radio" : "field";
      }
      case "textarea":
        return "field";
      case "select":
        return "select";
      case "option":
        return "option";
      case "ul":
      case "ol":
        return "list";
      case "li":
        return "item";
      case "table":
        return "table";
      case "th":
        return "column";
      case "tr":
        return "row";
      case "td":
        return "cell";
      case "img":
        return "image";
      case "dialog":
        return "dialog";
      case "template":
        return el.attrs["data-empty"] !== undefined ? "empty" : "each";
      default:
        return null;
    }
  };

  const convChildren = (el: HEl, ctx: Ctx, textKind: boolean): NNode[] => {
    const out: NNode[] = [];
    for (const c of el.children) {
      if (typeof c === "string") {
        const name = collapse(c);
        if (name && !textKind) out.push(node("text", { name }));
      } else if (isBindText(c)) {
        if (!textKind) out.push(node("text", { nameBind: ref(c.bindText, ctx.loops) }));
      } else out.push(...conv(c, ctx));
    }
    return out;
  };

  interface Ctx {
    loops: string[];
    form?: string | undefined;
  }

  function conv(el: HEl, ctx: Ctx): NNode[] {
    const a = el.attrs;
    if (el.tag === "label")
      return el.children
        .filter(isEl)
        .filter((c) => CONTROLS.has(c.tag))
        .flatMap((c) => conv(c, ctx));
    if (el.tag === "legend" || a.role === "tabpanel") return [];
    const kind = kindOf(el);
    if (kind === null) {
      if (el.tag === "tr") return [];
      return convChildren(el, ctx, false);
    }
    if (el.tag === "tr") {
      const cells = el.children.filter(isEl);
      if (cells.length && cells.every((c) => c.tag === "th"))
        return cells.flatMap((c) => conv(c, ctx));
    }
    const n = node(kind);
    const loops = kind === "each" && a["data-as"] ? [...ctx.loops, a["data-as"]] : ctx.loops;
    const inner: Ctx = { loops, form: kind === "form" ? el.on.submit : ctx.form };

    // name
    const own = textOf(el, ctx.loops);
    const aria =
      a["aria-label"] ??
      (a["aria-labelledby"]
        ? textOf(byId.get(a["aria-labelledby"]) ?? el, ctx.loops).name
        : undefined);
    const wrapped = labelOf.get(el);
    if (kind === "image") {
      if (a.alt) n.name = collapse(a.alt);
    } else if (NAMED_BY_TEXT.has(kind) || kind === "column") {
      // An accessible name from `label` wins over the text the element shows.
      if (el.bind.label) n.nameBind = ref(el.bind.label, ctx.loops);
      else {
        Object.assign(n, own);
        if (!n.name && !n.nameBind && aria) n.name = collapse(aria);
      }
    } else if (kind === "radio-group") {
      const legend = el.children.filter(isEl).find((c) => c.tag === "legend");
      const t = legend ? textOf(legend, ctx.loops) : {};
      if (t.name) n.name = t.name;
    } else if (aria) {
      n.name = collapse(aria);
    } else if (wrapped) {
      if (wrapped.name) n.name = wrapped.name;
      if (wrapped.nameBind)
        n.nameBind = wrapped.nameBind.startsWith("$.")
          ? wrapped.nameBind
          : normPath(wrapped.nameBind, loops);
    }

    // binding, value, flags
    const bindKey = ["value", "checked", "open", "src", "href", "selected"].find(
      (k) => el.bind[k] !== undefined,
    );
    if (bindKey && kind !== "radio") n.bind = ref(el.bind[bindKey] as string, ctx.loops);
    if (kind === "radio-group" && !n.bind) {
      const radio = el.children
        .filter(isEl)
        .flatMap((l) => (l.tag === "label" ? l.children.filter(isEl) : [l]))
        .find((r) => r.bind.groupValue);
      if (radio) n.bind = ref(radio.bind.groupValue as string, ctx.loops);
    }
    if (a.href && a.href !== "#" && kind === "link" && !n.bind) n.bind = a.href;
    if (kind === "radio" || kind === "option") n.value = a.value ?? "";
    if (kind === "button") n.variant = a["data-variant"] ?? "secondary";
    if (kind === "field") n.type = el.tag === "textarea" ? "multiline" : (a.type ?? "text");
    if (a.required !== undefined) n.required = true;
    if (kind === "heading") n.level = Number(el.tag.slice(1));
    if (a["aria-sort"]) n.sort = a["aria-sort"];
    if (a["data-state"]) n.state = a["data-state"];
    if (kind === "each") n.each = normPath(a["data-each"] ?? "", ctx.loops);
    for (const prop of ["disabled", "hidden"] as const) {
      if (el.bind[prop] !== undefined) n[prop] = ref(el.bind[prop] as string, ctx.loops);
      else if (a[prop] !== undefined) n[prop] = true;
    }
    Object.assign(n.on, el.on);
    if (kind === "button" && a.type === "submit" && !n.on.press && ctx.form) n.on.press = ctx.form;

    // children
    if (kind === "tab") {
      const panel = panels.get(a.id ?? "");
      n.children = panel ? convChildren(panel, inner, false) : [];
    } else if (kind === "column") {
      const btn = el.children.filter(isEl).find((c) => c.tag === "button");
      if (btn) Object.assign(n.on, btn.on);
    } else if (!NAMED_BY_TEXT.has(kind) && kind !== "option" && kind !== "image") {
      n.children = convChildren(el, inner, false);
    }
    return [n];
  }

  return conv(root, { loops: [] })[0] ?? node("screen");
}
