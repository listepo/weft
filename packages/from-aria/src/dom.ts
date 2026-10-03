// Rendered HTML → Weft (SPEC §9, "From a running UI"): pages from a Weft renderer, whose
// `data-weft-id` attributes give the ids back, or any semantic HTML. The HTML is untrusted: it is
// only parsed, never run, and size, depth and node count are bounded.
import { ARIA_ROLES, diagnostic, ID, type Diagnostic } from "@weft/core";
import { parseDocument } from "htmlparser2";
import { buildDocument, emptyResult, limitReached, MAX_DEPTH, MAX_NODES, squash } from "./build.ts";
import type { ImportOptions, ImportResult, Loss, LossKind, Scalar, Sem } from "./types.ts";

export const MAX_HTML_LENGTH = 5_000_000;

// HTML-AAM implicit roles of the elements user interfaces commonly use. Elements not listed (and
// the text-level ones such as strong or code) are generic: their content is read in place.
export const IMPLICIT_ROLES: Readonly<Record<string, string>> = {
  article: "article",
  aside: "complementary",
  button: "button",
  dialog: "dialog",
  fieldset: "group",
  // A form or section is a landmark only when named; here they always keep their kind.
  form: "form",
  h1: "heading",
  h2: "heading",
  h3: "heading",
  h4: "heading",
  h5: "heading",
  h6: "heading",
  hr: "separator",
  li: "listitem",
  main: "main",
  nav: "navigation",
  ol: "list",
  option: "option",
  output: "status",
  p: "paragraph",
  progress: "progressbar",
  section: "region",
  table: "table",
  tbody: "rowgroup",
  td: "cell",
  textarea: "textbox",
  tfoot: "rowgroup",
  thead: "rowgroup",
  tr: "row",
  ul: "list",
};

// `<input>` roles by `type`; any other type (text, email, password, tel, url, …) is a textbox.
export const INPUT_ROLES: Readonly<Record<string, string>> = {
  button: "button",
  checkbox: "checkbox",
  image: "button",
  number: "spinbutton",
  radio: "radio",
  range: "slider",
  reset: "button",
  search: "searchbox",
  submit: "button",
};

const SKIPPED = new Set([
  "base",
  "head",
  "link",
  "meta",
  "noscript",
  "script",
  "style",
  "template",
  "title",
]);
const CONTROLS = new Set(["input", "select", "textarea", "button"]);
const ROLES: ReadonlySet<string> = new Set(ARIA_ROLES);
const INSTANCE = /^([A-Za-z][A-Za-z0-9_-]*)((?:\[\d+\])*)$/;
const ALIGN: Readonly<Record<string, string>> = {
  "flex-start": "start",
  center: "center",
  "flex-end": "end",
  stretch: "stretch",
};

type HNode = {
  type: string;
  name?: string;
  data?: string;
  attribs?: Record<string, string>;
  children?: HNode[];
};

// The id a rendered instance `id[2][0]` gets in the import: the indexes appended with hyphens.
export function instanceId(raw: string): string | undefined {
  const m = INSTANCE.exec(raw);
  if (!m) return undefined;
  const indexes = [...(m[2] ?? "").matchAll(/\d+/g)].map((x) => x[0]);
  return [m[1], ...indexes].join("-");
}

type Ctx = {
  byHtmlId: Map<string, HNode>;
  labelFor: Map<string, HNode>;
  ids: Map<HNode, string>;
  notes: Map<HNode, { kind: LossKind; note: string }[]>;
  consumed: Set<HNode>;
  // Open <form> elements around the current one: only there does a submit button submit.
  forms: number;
  nodes: number;
  truncated: boolean;
};

const elements = (n: HNode): HNode[] => (n.children ?? []).filter(isElement);
function isElement(n: HNode): boolean {
  return (
    (n.type === "tag" || n.type === "script" || n.type === "style") && typeof n.name === "string"
  );
}

// One pass over the whole tree before conversion: ids and labels may be referenced from
// anywhere, and generated ids must avoid every id the page carries.
function prescan(root: HNode, ctx: Ctx): void {
  const seen = new Set<string>();
  const stack: HNode[] = [...elements(root)].reverse();
  let count = 0;
  while (stack.length > 0 && count++ < MAX_NODES * 4) {
    const el = stack.pop()!;
    const a = el.attribs ?? {};
    if (a["id"] !== undefined && !ctx.byHtmlId.has(a["id"])) ctx.byHtmlId.set(a["id"], el);
    if (el.name === "label" && a["for"] !== undefined && !ctx.labelFor.has(a["for"]))
      ctx.labelFor.set(a["for"], el);
    const raw = a["data-weft-id"];
    if (raw !== undefined) {
      const id = instanceId(raw);
      const notes: { kind: LossKind; note: string }[] = [];
      if (id === undefined || !ID.test(id)) {
        notes.push({
          kind: "ids",
          note: `data-weft-id "${raw.slice(0, 80)}" is not a valid id; a new id is generated`,
        });
      } else if (seen.has(id)) {
        notes.push({
          kind: "ids",
          note: `data-weft-id "${raw.slice(0, 80)}" repeats an earlier id; a new id is generated`,
        });
      } else {
        seen.add(id);
        ctx.ids.set(el, id);
        if (id !== raw)
          notes.push({
            kind: "repetition",
            note: `instance ${raw} of a repeated element is imported as the static element ${id}`,
          });
      }
      if (notes.length > 0) ctx.notes.set(el, notes);
    }
    stack.push(...elements(el).reverse());
  }
}

function roleOf(tag: string, a: Record<string, string>): string {
  for (const r of (a["role"] ?? "").trim().split(/\s+/)) if (ROLES.has(r)) return r;
  if (tag === "a") return a["href"] !== undefined ? "link" : "generic";
  if (tag === "input") {
    const type = (a["type"] ?? "text").toLowerCase();
    return Object.hasOwn(INPUT_ROLES, type) ? INPUT_ROLES[type]! : "textbox";
  }
  if (tag === "img") return a["alt"] === "" ? "presentation" : "img";
  if (tag === "th") return a["scope"] === "row" ? "rowheader" : "columnheader";
  if (tag === "select")
    return a["multiple"] !== undefined || Number(a["size"]) > 1 ? "listbox" : "combobox";
  return Object.hasOwn(IMPLICIT_ROLES, tag) ? IMPLICIT_ROLES[tag]! : "generic";
}

// Text a person sees in a subtree, without hidden parts; `skipControls` leaves out the control
// a label wraps.
function textContent(n: HNode, depth = 0, skipControls = false): string {
  if (n.type === "text") return n.data ?? "";
  if (!isElement(n) || depth > MAX_DEPTH) return "";
  const a = n.attribs ?? {};
  if (SKIPPED.has(n.name!) || a["aria-hidden"] === "true" || a["hidden"] !== undefined) return "";
  if (skipControls && CONTROLS.has(n.name!)) return "";
  return (n.children ?? []).map((c) => textContent(c, depth + 1, skipControls)).join("");
}

// A simplified accessible name (not full accname 1.2): aria-labelledby, aria-label, the alt of an
// image, the label of a form control (`<label for>` or a wrapping `<label>`), a table caption, the
// value of an input button, then title. Names computed from content are taken from the content
// itself where the catalog says a kind shows text.
function accName(el: HNode, tag: string, role: string, label: HNode | undefined, ctx: Ctx): string {
  const a = el.attribs ?? {};
  const by = (a["aria-labelledby"] ?? "")
    .split(/\s+/)
    .flatMap((id) => {
      const target = id === "" ? undefined : ctx.byHtmlId.get(id);
      return target ? [textContent(target)] : [];
    })
    .join(" ");
  if (squash(by) !== "") return squash(by);
  if (squash(a["aria-label"] ?? "") !== "") return squash(a["aria-label"] ?? "");
  if (role === "img" && a["alt"] !== undefined) return squash(a["alt"]);
  if (CONTROLS.has(tag) && tag !== "button") {
    const owner = (a["id"] !== undefined ? ctx.labelFor.get(a["id"]) : undefined) ?? label;
    const text = owner ? squash(textContent(owner, 0, true)) : "";
    if (text !== "") return text;
    if (tag === "input" && role === "button") return squash(a["value"] ?? "");
  }
  if (tag === "table") {
    const caption = elements(el).find((c) => c.name === "caption");
    if (caption) return squash(textContent(caption));
  }
  return squash(a["title"] ?? "");
}

function parseStyle(style: string | undefined): Map<string, string> {
  const out = new Map<string, string>();
  for (const decl of (style ?? "").split(";")) {
    const colon = decl.indexOf(":");
    if (colon > 0) out.set(decl.slice(0, colon).trim().toLowerCase(), decl.slice(colon + 1).trim());
  }
  return out;
}

// Weft renderer markup for the role-less layout and text kinds (SPEC §9: they add no node of
// their own), recognised only on elements that carry a Weft id.
function layout(s: Sem, el: HNode): void {
  const style = parseStyle(el.attribs?.["style"]);
  const display = style.get("display");
  const hasElements = elements(el).length > 0;
  if (display === "grid") {
    s.kind = "grid";
    const columns = /^repeat\(\s*(\d+)\s*,/.exec(style.get("grid-template-columns") ?? "")?.[1];
    s.props["columns"] = columns ?? "1";
  } else if (display === "flex" || hasElements) {
    s.kind = "stack";
    if (style.get("flex-direction") === "row") s.props["direction"] = "row";
    const align = style.get("align-items");
    if (align !== undefined && Object.hasOwn(ALIGN, align)) s.props["align"] = ALIGN[align]!;
    if (style.get("flex-wrap") === "wrap") s.props["wrap"] = "true";
  } else {
    s.kind = "text";
  }
  const gap = style.get("gap");
  if (gap !== undefined) {
    (s.notes ??= []).push({
      kind: gap.startsWith("var(--weft-") ? "tokens" : "layout",
      note: `gap ${gap.slice(0, 80)} cannot be mapped back to a design token`,
    });
  }
}

function element(el: HNode, ctx: Ctx, depth: number, label: HNode | undefined): Sem | undefined {
  const tag = el.name!.toLowerCase();
  const a = el.attribs ?? {};
  if (SKIPPED.has(tag) || ctx.consumed.has(el) || a["aria-hidden"] === "true") return undefined;
  if (depth > MAX_DEPTH || ++ctx.nodes > MAX_NODES) {
    ctx.truncated = true;
    return undefined;
  }
  const role = roleOf(tag, a);
  // Hidden content is not part of the UI, except inactive tab panels, which hold a tab's content.
  if (a["hidden"] !== undefined && role !== "tabpanel") return undefined;
  if (tag === "input" && (a["type"] ?? "").toLowerCase() === "hidden") return undefined;
  if (tag === "img" && role === "presentation") return undefined;
  if (tag === "caption") return undefined;

  const s: Sem = {
    role,
    name: accName(el, tag, role, label, ctx),
    states: {},
    props: {},
    children: [],
  };
  const id = ctx.ids.get(el);
  if (id !== undefined) s.id = id;
  const notes = ctx.notes.get(el);
  if (notes) s.notes = [...notes];
  if (a["id"] !== undefined) s.ref = a["id"];
  if (a["aria-labelledby"] !== undefined) s.labelledBy = a["aria-labelledby"].split(/\s+/);

  const states: Record<string, Scalar> = s.states;
  if (a["disabled"] !== undefined || a["aria-disabled"] === "true") states["disabled"] = true;
  if (a["checked"] !== undefined || a["aria-checked"] === "true") states["checked"] = true;
  if ((tag === "option" && a["selected"] !== undefined) || a["aria-selected"] === "true")
    states["selected"] = true;
  const level = /^h([1-6])$/.exec(tag)?.[1] ?? a["aria-level"];
  if (level !== undefined && /^\d+$/.test(level)) states["level"] = Number(level);
  if (a["aria-busy"] === "true") states["busy"] = true;

  const p = s.props;
  if (a["data-state"] !== undefined) p["state"] = a["data-state"];
  if (a["data-variant"] !== undefined) p["variant"] = a["data-variant"];
  if (a["data-tone"] !== undefined) p["tone"] = a["data-tone"];
  if (a["aria-sort"] !== undefined) p["sort"] = a["aria-sort"];
  if (a["aria-modal"] !== undefined) p["modal"] = a["aria-modal"] === "true" ? "true" : "false";
  if (a["required"] !== undefined || a["aria-required"] === "true") p["required"] = "true";
  if (a["placeholder"] !== undefined) p["placeholder"] = a["placeholder"];
  if (tag === "a" && a["href"] !== undefined) p["href"] = a["href"];
  if (tag === "img" && a["src"] !== undefined) p["src"] = a["src"];
  if (tag === "ol") p["ordered"] = "true";
  if (tag === "input") {
    const type = (a["type"] ?? "text").toLowerCase();
    if (role === "textbox" || role === "spinbutton" || role === "searchbox") {
      if (type !== "text") p["type"] = type;
    }
    if (a["value"] !== undefined && role !== "checkbox" && role !== "switch")
      p["value"] = a["value"];
    if ((type === "submit" || type === "image") && ctx.forms > 0) p["submit"] = "true";
  }
  if (tag === "textarea") {
    p["type"] = "multiline";
    p["value"] = textContent(el);
  }
  if (tag === "option") p["value"] = a["value"] ?? squash(textContent(el));
  // HTML: a button without a type submits its form.
  if (tag === "button" && (a["type"] ?? "submit").toLowerCase() === "submit" && ctx.forms > 0)
    p["submit"] = "true";

  // An error message the control points at is that control's `error`, not separate text.
  const described = (a["aria-describedby"] ?? "").split(/\s+/).flatMap((ref) => {
    const target = ref === "" ? undefined : ctx.byHtmlId.get(ref);
    return target ? [target] : [];
  });
  if (described.length > 0 && CONTROLS.has(tag)) {
    p["error"] = squash(described.map((d) => textContent(d)).join(" "));
    for (const d of described) ctx.consumed.add(d);
  }
  if (a["aria-invalid"] === "true" && p["error"] === undefined) states["invalid"] = true;

  if (
    id !== undefined &&
    role === "generic" &&
    a["role"] === undefined &&
    (tag === "div" || tag === "span")
  )
    layout(s, el);

  if (tag !== "input" && tag !== "textarea" && tag !== "img") {
    if (tag === "form") ctx.forms++;
    // What a control holds (a select's options) is its own content, not the name a wrapping
    // label gives it.
    const within = tag === "label" ? el : CONTROLS.has(tag) ? undefined : label;
    s.children = children(el, ctx, depth, within);
    if (tag === "form") ctx.forms--;
  }
  return s;
}

function children(el: HNode, ctx: Ctx, depth: number, label: HNode | undefined): Sem[] {
  const out: Sem[] = [];
  for (const c of el.children ?? []) {
    if (c.type === "text") {
      // Text inside a label is the name of its control, not content of its own.
      const t = c.data ?? "";
      if (label === undefined && t.trim() !== "")
        out.push({ role: "text", name: t, states: {}, props: {}, children: [] });
      continue;
    }
    if (c.type === "tag" && c.name === "br") {
      out.push({ role: "text", name: " ", states: {}, props: {}, children: [] });
      continue;
    }
    if (!isElement(c)) continue;
    const s = element(c, ctx, depth + 1, label);
    if (s) out.push(s);
  }
  return out;
}

// Never recoverable from rendered HTML, whatever it holds.
const DOM_LOSSES: readonly [LossKind, string][] = [
  ["bindings", "values are the resolved values the page shows, not bindings"],
  ["actions", "event handlers and their action names are not in the HTML"],
  ["tokens", "design token references are rendered as CSS and cannot be mapped back"],
  ["slots", "slot membership is not in the HTML; slot content is imported as default content"],
  ["hidden", "elements a renderer leaves out (hidden, closed dialogs) are not in the HTML"],
];

export function fromDom(html: string, options: ImportOptions): ImportResult {
  const diagnostics: Diagnostic[] = [];
  if (typeof html !== "string") {
    diagnostics.push(
      diagnostic("W601", {
        path: "#",
        message: "The input is not a string of HTML.",
        expected: "HTML text",
      }),
    );
    return emptyResult(diagnostics);
  }
  try {
    let text = html;
    if (text.length > MAX_HTML_LENGTH) {
      text = text.slice(0, MAX_HTML_LENGTH);
      limitReached(diagnostics, "#", `is longer than ${MAX_HTML_LENGTH} characters`);
    }
    const dom = parseDocument(text) as unknown as HNode;
    const ctx: Ctx = {
      byHtmlId: new Map(),
      labelFor: new Map(),
      ids: new Map(),
      notes: new Map(),
      consumed: new Set(),
      forms: 0,
      nodes: 0,
      truncated: false,
    };
    prescan(dom, ctx);
    const body = findBody(dom) ?? dom;
    const sems = children(body, ctx, 0, undefined);
    if (ctx.truncated) limitReached(diagnostics, "#", "is larger or deeper than the import limit");
    const built = buildDocument(sems, {
      catalog: options.catalog,
      reserved: ctx.ids.values(),
      diagnostics,
    });
    const losses: Loss[] = DOM_LOSSES.map(([kind, note]) => ({ kind, path: built.rootPath, note }));
    return {
      document: built.document,
      losses: [...losses, ...built.losses],
      diagnostics: built.diagnostics,
    };
  } catch (error) {
    // Only a malformed catalog can get here; any HTML string parses.
    diagnostics.push(
      diagnostic("W601", {
        path: "#",
        message: "The HTML could not be mapped with this catalog.",
        expected: "a catalog with the shape of SPEC §5",
        got: error instanceof Error ? error.message.slice(0, 200) : undefined,
      }),
    );
    return emptyResult(diagnostics);
  }
}

function findBody(root: HNode): HNode | undefined {
  const stack = elements(root);
  for (let i = 0; i < stack.length && i < 64; i++) {
    const el = stack[i]!;
    if (el.name === "body") return el;
    if (el.name === "html") stack.push(...elements(el));
  }
  return undefined;
}
