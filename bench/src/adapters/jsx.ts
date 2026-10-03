import { parseSync } from "oxc-parser";
import { htmlLikeToNeutral, type HChild, type HEl } from "./html-like.ts";
import type { Parsed } from "./types.ts";

// The ESTree shape is wide and only a sliver of it is read here, so nodes stay loosely typed.
// oxlint-disable-next-line typescript/no-explicit-any
type Ast = any;

const BOUND_ATTRS = new Set([
  "value",
  "checked",
  "open",
  "src",
  "href",
  "selected",
  "disabled",
  "hidden",
]);
const EVENTS: Record<string, string> = {
  onClick: "press",
  onSubmit: "submit",
  onChange: "change",
  onClose: "close",
};
const COLLECTIONS = new Set(["ul", "ol", "table"]);
const ATTR_ALIASES: Record<string, string> = { className: "class", htmlFor: "for" };

const unwrap = (n: Ast): Ast =>
  n?.type === "ParenthesizedExpression" || n?.type === "ChainExpression" ? unwrap(n.expression) : n;

function pathOf(expr: Ast, scope: string[]): string | undefined {
  const e = unwrap(expr);
  if (!e) return undefined;
  if (e.type === "UnaryExpression" && e.operator === "!") {
    const inner = pathOf(e.argument, scope);
    return inner && !inner.startsWith("!") ? "!" + inner : undefined;
  }
  const parts: string[] = [];
  let cur = e;
  while (cur?.type === "MemberExpression" && !cur.computed) {
    parts.unshift(cur.property.name);
    cur = unwrap(cur.object);
  }
  if (cur?.type !== "Identifier" || (parts.length === 0 && cur.name !== "data")) return undefined;
  if (cur.name === "data") return parts.length ? "$." + parts.join(".") : undefined;
  return scope.includes(cur.name) ? "$" + [cur.name, ...parts].join(".") : undefined;
}

/** Named host actions called in a handler body: `actions.nav.reset()` -> "nav.reset"; `actions.set` is state plumbing. */
function actionIn(handler: Ast): string | undefined {
  let found: string | undefined;
  const visit = (n: Ast) => {
    if (!n || typeof n !== "object" || found) return;
    if (Array.isArray(n)) return n.forEach(visit);
    if (n.type === "CallExpression") {
      const chain: string[] = [];
      let cur = unwrap(n.callee);
      while (cur?.type === "MemberExpression" && !cur.computed) {
        chain.unshift(cur.property.name);
        cur = unwrap(cur.object);
      }
      if (
        cur?.type === "Identifier" &&
        cur.name === "actions" &&
        chain.length &&
        chain[0] !== "set"
      ) {
        found = chain.join(".");
        return;
      }
    }
    for (const v of Object.values(n)) visit(v);
  };
  visit(handler);
  return found;
}

function bodyElement(fn: Ast): Ast {
  const body = unwrap(fn.body);
  if (body.type !== "BlockStatement") return body;
  const ret = body.body.find((s: Ast) => s.type === "ReturnStatement");
  return unwrap(ret?.argument);
}

function toHEl(n: Ast, scope: string[]): HEl {
  const open = n.openingElement;
  const tag: string = n.type === "JSXFragment" ? "fragment" : open.name.name;
  const el: HEl = { tag, attrs: {}, bind: {}, on: {}, children: [] };
  for (const attr of open?.attributes ?? []) {
    if (attr.type !== "JSXAttribute") continue;
    const rawName: string = attr.name.name;
    const name = ATTR_ALIASES[rawName] ?? rawName;
    const v = attr.value;
    if (v === null) {
      el.attrs[name] = "";
    } else if (v.type === "Literal") {
      el.attrs[name] = String(v.value);
    } else if (v.type === "JSXExpressionContainer") {
      const x = unwrap(v.expression);
      const event = EVENTS[rawName];
      if (event) {
        const action = actionIn(x);
        if (action) el.on[event] = action;
      } else if (x.type === "Literal") {
        el.attrs[name] = String(x.value);
      } else if (x.type === "BinaryExpression" && x.operator === "===" && name === "checked") {
        const p = pathOf(x.left, scope);
        if (p) el.bind.groupValue = p;
      } else {
        const p = pathOf(x, scope);
        if (p && BOUND_ATTRS.has(name)) el.bind[name] = p;
      }
    }
  }
  el.children = (n.children ?? []).flatMap((c: Ast) => toChildren(c, scope));
  return el;
}

const negate = (p: string): string => (p.startsWith("!") ? p.slice(1) : "!" + p);

function toChildren(c: Ast, scope: string[]): HChild[] {
  if (c.type === "JSXText") return [c.value];
  if (c.type === "JSXElement" || c.type === "JSXFragment") return [toHEl(c, scope)];
  if (c.type !== "JSXExpressionContainer") return [];
  const x = unwrap(c.expression);
  if (x.type === "JSXEmptyExpression") return [];
  if (x.type === "Literal") return [String(x.value)];
  if (x.type === "JSXElement") return [toHEl(x, scope)];
  const p = pathOf(x, scope);
  if (p) return [{ bindText: p }];
  if (
    x.type === "CallExpression" &&
    unwrap(x.callee).type === "MemberExpression" &&
    unwrap(x.callee).property.name === "map"
  ) {
    const source = pathOf(unwrap(x.callee).object, scope);
    const fn = x.arguments[0];
    const param: string | undefined = fn?.params?.[0]?.name;
    if (source && param) {
      const child = bodyElement(fn);
      const inner = child?.type === "JSXElement" ? [toHEl(child, [...scope, param])] : [];
      return [
        {
          tag: "template",
          attrs: { "data-each": source, "data-as": param },
          bind: {},
          on: {},
          children: inner,
        },
      ];
    }
  }
  if (x.type === "LogicalExpression" && x.operator === "&&") {
    const cond = pathOf(x.left, scope);
    const right = unwrap(x.right);
    if (cond && right.type === "JSXElement") {
      const el = toHEl(right, scope);
      el.bind.hidden = negate(cond);
      return [el];
    }
  }
  if (x.type === "ConditionalExpression") {
    const branches: HEl[] = [x.consequent, x.alternate]
      .map(unwrap)
      .filter((b: Ast) => b.type === "JSXElement")
      .map((b: Ast) => toHEl(b, scope));
    // `items.length > 0 ? <ul>…</ul> : <p>…</p>` is a collection with an empty state, written in
    // HTML as `<template data-empty>` inside the list.
    const test = unwrap(x.test);
    const list = branches.find((b) => COLLECTIONS.has(b.tag));
    const empty = branches.find((b) => b !== list);
    if (
      list &&
      empty &&
      test.type === "BinaryExpression" &&
      unwrap(test.left)?.type === "MemberExpression" &&
      unwrap(test.left).property.name === "length"
    ) {
      list.children.push({
        tag: "template",
        attrs: { "data-empty": "" },
        bind: {},
        on: {},
        children: [empty],
      });
      return [list];
    }
    return branches;
  }
  return [];
}

function firstJsx(n: Ast): Ast {
  if (!n || typeof n !== "object") return undefined;
  if (Array.isArray(n)) {
    for (const x of n) {
      const f = firstJsx(x);
      if (f) return f;
    }
    return undefined;
  }
  if (n.type === "JSXElement") return n;
  for (const v of Object.values(n)) {
    const f = firstJsx(v);
    if (f) return f;
  }
  return undefined;
}

export function parseJsx(src: string): Parsed {
  const result = parseSync("screen.jsx", src, { lang: "jsx" });
  const errors = result.errors.map((e) => e.message);
  const root = firstJsx(result.program);
  if (!root) return { errors: [...errors, "no JSX element found"] };
  return { tree: htmlLikeToNeutral(toHEl(root, [])), errors };
}
