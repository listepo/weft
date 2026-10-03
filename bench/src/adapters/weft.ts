import { parse, type Child, type Node, type Value } from "@weft/core";
import { coreCatalog } from "@weft/catalog";
import { node, normPath, type Parsed } from "./types.ts";
import type { NNode } from "../neutral.ts";

/** The prop that carries each kind's data binding, compared as `NNode.bind`. */
const BOUND: Record<string, string> = {
  field: "value",
  checkbox: "checked",
  switch: "checked",
  "radio-group": "value",
  select: "value",
  image: "src",
  link: "href",
  dialog: "open",
  tabs: "selected",
};

const isNode = (c: Child): c is Node => typeof c !== "string";

/**
 * Model output is judged by the reference validator in strict mode, the default for writers
 * (SPEC §8). Token and action names are not checked: the primers do not list them.
 */
export function parseWeft(src: string): Parsed {
  const result = parse(src, { catalog: coreCatalog, mode: "strict" });
  const errors = result.diagnostics
    .filter((d) => d.severity === "error")
    .map((d) => `${d.code} ${d.path}: ${d.message}`);
  if (result.document === undefined) return { errors };
  return { tree: convert(result.document.root, [], undefined)[0] ?? node("screen"), errors };
}

function convert(n: Node, loops: string[], form: string | undefined): NNode[] {
  const props = n.props ?? {};
  const ref = (value: Value | undefined): string | undefined =>
    typeof value === "object" && "bind" in value
      ? (value.not === true ? "!" : "") + normPath(value.bind, loops)
      : undefined;
  /** The name or bound name a value supplies; empty when it supplies none. */
  const naming = (value: Value | undefined): Partial<NNode> => {
    const nameBind = ref(value);
    if (nameBind !== undefined) return { nameBind };
    return value === undefined ? {} : { name: String(value) };
  };
  const literal = (value: Value | undefined, fallback = ""): string =>
    typeof value === "string" ? value : fallback;

  const out = node(n.kind);
  const inner =
    n.kind === "each" && typeof props["as"] === "string" ? [...loops, props["as"]] : loops;
  const innerForm = n.kind === "form" ? n.on?.["submit"] : form;
  if (n.kind === "each") out.each = ref(props["in"]) ?? "";

  const content = coreCatalog.components[n.kind]?.content;
  const texts = (n.children ?? []).filter((c): c is string => !isNode(c));
  const children: NNode[] = [];
  if (content === "text") {
    // A text kind is named by its content or by its `text` prop, never both (SPEC §5.1).
    Object.assign(out, texts.length > 0 ? { name: texts.join(" ") } : naming(props["text"]));
  } else {
    Object.assign(out, naming(props["label"]));
    if (props["text"] !== undefined) children.push(node("text", naming(props["text"])));
  }

  const bound = BOUND[n.kind];
  if (bound) {
    const b = ref(props[bound]) ?? (n.kind === "link" ? literal(props["href"]) : "");
    if (b) out.bind = b;
  }
  if (n.kind === "radio" || n.kind === "option") out.value = literal(props["value"]);
  if (n.kind === "button") out.variant = literal(props["variant"], "secondary");
  if (n.kind === "field") out.type = literal(props["type"], "text");
  if (props["required"] === true) out.required = true;
  if (typeof props["level"] === "number") out.level = props["level"];
  if (literal(props["sort"])) out.sort = literal(props["sort"]);
  if (literal(props["state"])) out.state = literal(props["state"]);
  for (const flag of ["disabled", "hidden"] as const) {
    const b = ref(props[flag]);
    if (b) out[flag] = b;
    else if (props[flag] === true) out[flag] = true;
  }
  Object.assign(out.on, n.on);
  // A submit button fires its form's submit action, as `type="submit"` does in HTML and JSX.
  if (props["submit"] === true && !out.on["press"] && form) out.on["press"] = form;

  for (const c of n.children ?? []) {
    if (isNode(c)) children.push(...convert(c, inner, innerForm));
    else if (content !== "text") children.push(node("text", { name: c }));
  }
  for (const [name, list] of Object.entries(n.slots ?? {})) {
    const converted = list.filter(isNode).flatMap((c) => convert(c, inner, innerForm));
    // The empty state is kept apart so that it is not mistaken for list or table content.
    if (name === "empty") children.push(node("empty", { children: converted }));
    else children.push(...converted);
  }
  out.children = children;
  return [out];
}
