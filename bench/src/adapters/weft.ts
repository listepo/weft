import { parseDocument, Parser } from "htmlparser2";
import { collapse, node, normPath, type Parsed } from "./types.ts";
import type { NNode } from "../neutral.ts";
import { STRUCTURAL, UNIVERSAL_ATTRS, WEFT_KINDS } from "../weft-catalog.ts";

const SLOTS: Record<string, string[]> = {
  form: ["footer"],
  dialog: ["actions"],
  section: ["header"],
};
const NAME_PATTERN = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;
const ID_PATTERN = /^[A-Za-z][A-Za-z0-9_-]*$/;
const ACTION_PATTERN = /^[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*$/;
const BINDING = /^\{(!?)(\$[^{}]*)\}$/;
const TOKEN = /^\{token\.[a-z0-9.-]+\}$/;
const PATH =
  /^\$(\.[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*)(\.[A-Za-z_0-9][A-Za-z0-9_]*)*$/;

type El = ReturnType<typeof parseDocument>["children"][number];

interface TagEl {
  type: string;
  name: string;
  attribs: Record<string, string>;
  children: El[];
}
const isTag = (n: El): n is El & TagEl => n.type === "tag";
const isText = (n: El): n is El & { data: string } => n.type === "text";

/** Strict-syntax checks that the lenient XML parser would otherwise let through. */
function syntaxErrors(src: string): string[] {
  const errors: string[] = [];
  const parser = new Parser(
    {
      onattribute(name, _value, quote) {
        if (quote !== '"') errors.push(`attribute "${name}" must have a double-quoted value`);
      },
      onclosetag(name, implied) {
        // htmlparser2 reports self-closed elements as implied closes too, so look at the source.
        if (implied && !src.slice(0, parser.endIndex + 1).endsWith("/>"))
          errors.push(`element <${name}> is not closed`);
      },
      oncdatastart() {
        errors.push("CDATA sections are not allowed");
      },
      onprocessinginstruction() {
        errors.push("processing instructions and declarations are not allowed");
      },
      onerror(e) {
        errors.push(e.message);
      },
    },
    { xmlMode: true },
  );
  parser.write(src);
  parser.end();
  return errors;
}

export function validateWeft(src: string): string[] {
  const errors = syntaxErrors(src);
  const doc = parseDocument(src, { xmlMode: true });
  const roots = doc.children.filter(isTag);
  if (roots.length !== 1 || roots[0]?.name !== "screen")
    errors.push("the document must have exactly one root <screen>");
  const ids = new Set<string>();
  const check = (el: TagEl, parent: TagEl | null, scope: string[]) => {
    const { name, attribs: a } = el;
    if (!NAME_PATTERN.test(name)) errors.push(`invalid element name <${name}>`);
    if (name === "slot") {
      if (!parent || !(SLOTS[parent.name] ?? []).includes(a.name ?? ""))
        errors.push(`<slot name="${a.name}"> is not declared by <${parent?.name}>`);
      for (const c of el.children.filter(isTag)) check(c, parent, scope);
      return;
    }
    const def = WEFT_KINDS[name];
    if (!def && !name.startsWith("x-") && !STRUCTURAL.includes(name))
      errors.push(`unknown element <${name}>`);
    if (!a.id) errors.push(`<${name}> has no id`);
    else if (!ID_PATTERN.test(a.id)) errors.push(`invalid id "${a.id}"`);
    else if (ids.has(a.id)) errors.push(`duplicate id "${a.id}"`);
    else ids.add(a.id);
    for (const [attr, value] of Object.entries(a)) {
      if (attr === "id") continue;
      if (!NAME_PATTERN.test(attr)) errors.push(`invalid attribute name "${attr}" on <${name}>`);
      if (attr.startsWith("on-")) {
        if (!def?.events?.includes(attr.slice(3)))
          errors.push(`<${name}> does not declare event "${attr.slice(3)}"`);
        if (!ACTION_PATTERN.test(value)) errors.push(`invalid action name "${value}"`);
        continue;
      }
      const binding = BINDING.exec(value);
      if (binding) {
        if (!PATH.test(binding[2] as string)) errors.push(`invalid binding path "${value}"`);
        continue;
      }
      if (TOKEN.test(value)) continue;
      if (value.startsWith("{") && !value.startsWith("{{"))
        errors.push(`malformed binding "${value}"`);
      if (/\{[^}]*\}/.test(value) && !value.startsWith("{{"))
        errors.push(`text and bindings cannot be mixed in "${value}"`);
      if (name === "each") {
        if (attr !== "in" && attr !== "as") errors.push(`<each> does not take "${attr}"`);
        continue;
      }
      if (UNIVERSAL_ATTRS.includes(attr)) {
        if (attr === "hidden" && value !== "true" && value !== "false")
          errors.push(`hidden must be true or false, got "${value}"`);
        if (attr === "state" && def?.states && !def.states.includes(value))
          errors.push(`<${name}> has no state "${value}"`);
        continue;
      }
      const prop = def?.props[attr];
      if (def && prop === undefined) errors.push(`<${name}> has no attribute "${attr}"`);
      else if (prop === "boolean" && value !== "true" && value !== "false")
        errors.push(`${attr} must be true or false, got "${value}"`);
      else if (prop === "number" && Number.isNaN(Number(value)))
        errors.push(`${attr} must be a number, got "${value}"`);
      else if (Array.isArray(prop) && !prop.includes(value))
        errors.push(`${attr} must be one of ${prop.join("/")}, got "${value}"`);
    }
    for (const req of def?.required ?? [])
      if (!(req in a)) errors.push(`<${name}> requires "${req}"`);
    if (name === "each") {
      if (!a.in || !BINDING.test(a.in)) errors.push("<each> needs an `in` binding");
      if (!a.as || !/^[a-z][A-Za-z0-9]*$/.test(a.as)) errors.push("<each> needs a valid `as` name");
    }
    const inner = name === "each" && a.as ? [...scope, a.as] : scope;
    for (const [attr, value] of Object.entries(a)) {
      const m = BINDING.exec(value);
      const v = m && /^\$([A-Za-z_]\w*)/.exec(m[2] as string);
      if (v && !m[2]?.startsWith("$.") && !scope.includes(v[1] as string))
        errors.push(`binding ${value} in "${attr}" uses an unknown loop variable`);
    }
    for (const c of el.children.filter(isTag)) check(c, el, inner);
  };
  for (const r of roots) check(r, null, []);
  return errors;
}

const TEXT_NAMED = new Set([
  "heading",
  "text",
  "link",
  "button",
  "column",
  "radio",
  "option",
  "menu-item",
]);

export function parseWeft(src: string): Parsed {
  const errors = validateWeft(src);
  const doc = parseDocument(src, { xmlMode: true });
  const root = doc.children.find(isTag);
  if (!root) return { errors: [...errors, "no root element"] };

  const bindingOf = (raw: string | undefined, loops: string[]) => {
    const m = raw === undefined ? null : BINDING.exec(raw);
    return m ? { path: normPath(m[2] as string, loops), not: m[1] === "!" } : null;
  };
  const refOf = (raw: string | undefined, loops: string[]): string | undefined => {
    const b = bindingOf(raw, loops);
    return b ? (b.not ? "!" : "") + b.path : undefined;
  };

  const convert = (el: TagEl, loops: string[]): NNode[] => {
    const a = el.attribs;
    if (el.name === "slot") return el.children.filter(isTag).flatMap((c) => convert(c, loops));
    const inner = el.name === "each" && a.as ? [...loops, a.as] : loops;
    const n = node(el.name);
    const own = el.children
      .filter(isText)
      .map((t) => collapse(t.data))
      .filter(Boolean)
      .join(" ");
    const kids = el.children.filter(isTag).flatMap((c) => convert(c, inner));
    if (el.name === "each") n.each = bindingOf(a.in, loops)?.path ?? "";
    // Literal prop text is the name for kinds whose content is text; elsewhere it is a child.
    const valueBind = refOf(a.value, loops);
    if (TEXT_NAMED.has(el.name)) {
      if (own) n.name = own;
      else if (el.name === "heading" || el.name === "text") {
        if (valueBind) n.nameBind = valueBind;
        else if (a.value) n.name = collapse(a.value);
      }
    } else {
      const lb = refOf(a.label, loops);
      if (lb) n.nameBind = lb;
      else if (a.label !== undefined) n.name = collapse(a.label);
      if (own) kids.unshift(node("text", { name: own }));
    }
    const bound = {
      field: "value",
      checkbox: "checked",
      switch: "checked",
      "radio-group": "value",
      select: "value",
      image: "src",
      link: "href",
      dialog: "open",
      tabs: "selected",
    }[el.name];
    if (bound) {
      const b = refOf(a[bound], loops);
      if (b) n.bind = b;
    }
    if (el.name === "radio" || el.name === "option") n.value = a.value ?? "";
    if (el.name === "button") n.variant = a.variant ?? "secondary";
    if (el.name === "field") n.type = a.type ?? "text";
    if (a.required === "true") n.required = true;
    if (a.level) n.level = Number(a.level);
    if (a.sort) n.sort = a.sort;
    if (a.state) n.state = a.state;
    for (const prop of ["disabled", "hidden"] as const) {
      const raw = a[prop];
      if (raw === undefined) continue;
      const b = refOf(raw, loops);
      if (b) n[prop] = b;
      else if (raw === "true") n[prop] = true;
    }
    for (const [attr, action] of Object.entries(a))
      if (attr.startsWith("on-")) n.on[attr.slice(3)] = action;
    n.children = kids;
    return [n];
  };
  const tree = convert(root, [])[0];
  return tree ? { tree, errors } : { errors };
}
