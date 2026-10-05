import { collapse, node, type Parsed } from "./types.ts";
import type { NNode } from "../neutral.ts";

// Basic catalog of A2UI v0.9 (specification/v0_9/catalogs/basic/catalog.json): property names per
// component, taken from the catalog's schemas. All components also accept id, accessibility, weight.
const COMMON = ["id", "component", "accessibility", "weight"];
const CATALOG: Record<string, { props: string[]; required: string[] }> = {
  Text: { props: ["text", "variant"], required: ["text"] },
  Image: { props: ["url", "description", "fit", "variant"], required: ["url"] },
  Icon: { props: ["name"], required: ["name"] },
  Video: { props: ["url"], required: ["url"] },
  AudioPlayer: { props: ["url", "description"], required: ["url"] },
  Row: { props: ["children", "justify", "align"], required: ["children"] },
  Column: { props: ["children", "justify", "align"], required: ["children"] },
  List: { props: ["children", "direction", "align"], required: ["children"] },
  Card: { props: ["child"], required: ["child"] },
  Tabs: { props: ["tabs"], required: ["tabs"] },
  Modal: { props: ["trigger", "content"], required: ["trigger", "content"] },
  Divider: { props: ["axis"], required: [] },
  Button: { props: ["child", "variant", "action", "checks"], required: ["child", "action"] },
  TextField: {
    props: ["label", "value", "variant", "validationRegexp", "checks"],
    required: ["label"],
  },
  CheckBox: { props: ["label", "value", "checks"], required: ["label", "value"] },
  ChoicePicker: {
    props: ["label", "variant", "options", "value", "displayStyle", "filterable", "checks"],
    required: ["options", "value"],
  },
  Slider: { props: ["label", "min", "max", "value"], required: ["value", "max"] },
  DateTimeInput: {
    props: ["value", "enableDate", "enableTime", "min", "max", "label"],
    required: ["value"],
  },
};
const ENVELOPE = ["createSurface", "updateComponents", "updateDataModel", "deleteSurface"];

type Json = any; // oxlint-disable-line typescript/no-explicit-any

/** JSON Pointer to the shared notation; a path without leading "/" is relative to the repetition item. */
function pointer(path: string): string {
  const segs = path.split("/").filter(Boolean);
  return path.startsWith("/") ? "$." + segs.join(".") : "$*." + segs.join(".");
}

const dynamic = (v: Json): { lit?: string; path?: string } =>
  v !== null && typeof v === "object" && typeof v.path === "string"
    ? { path: pointer(v.path) }
    : v === undefined
      ? {}
      : { lit: String(v) };

function disabledFrom(checks: Json): string | undefined {
  for (const c of Array.isArray(checks) ? checks : []) {
    const cond = c.condition ?? c;
    const path = cond?.args?.value?.path ?? cond?.path;
    if (typeof path !== "string") continue;
    if (cond.call === "not") return pointer(path);
    return "!" + pointer(path);
  }
  return undefined;
}

export function parseA2ui(src: string): Parsed {
  const errors: string[] = [];
  let json: Json;
  try {
    json = JSON.parse(src);
  } catch (e) {
    return { errors: [`invalid JSON: ${(e as Error).message}`] };
  }
  const messages: Json[] = Array.isArray(json)
    ? json
    : Array.isArray(json?.messages)
      ? json.messages
      : [json];
  const byId = new Map<string, Json>();
  for (const m of messages) {
    const keys = Object.keys(m ?? {}).filter((k) => k !== "version");
    if (keys.length !== 1 || !ENVELOPE.includes(keys[0] as string))
      errors.push("every message needs exactly one of " + ENVELOPE.join(", "));
    if (m?.version !== "v0.9") errors.push('every message needs "version": "v0.9"');
    for (const c of m?.updateComponents?.components ?? []) {
      if (!c?.id || typeof c.id !== "string") errors.push("a component has no id");
      else if (byId.has(c.id)) errors.push(`duplicate component id "${c.id}"`);
      else byId.set(c.id, c);
    }
  }
  for (const c of byId.values()) {
    const def = CATALOG[c.component];
    if (!def) {
      errors.push(`unknown component "${c.component}" (id ${c.id})`);
      continue;
    }
    for (const k of Object.keys(c))
      if (!COMMON.includes(k) && !def.props.includes(k))
        errors.push(`${c.component} "${c.id}" has no property "${k}"`);
    for (const r of def.required)
      if (!(r in c)) errors.push(`${c.component} "${c.id}" requires "${r}"`);
    const refs: unknown[] = [
      c.child,
      c.trigger,
      c.content,
      ...(Array.isArray(c.children) ? c.children : [c.children?.componentId]),
      ...(c.tabs ?? []).map((t: Json) => t.child),
    ];
    for (const r of refs)
      if (typeof r === "string" && !byId.has(r))
        errors.push(`${c.component} "${c.id}" references missing component "${r}"`);
  }
  if (!byId.has("root")) return { errors: [...errors, 'no component with id "root"'] };

  const labelOf = (v: Json): Partial<NNode> => {
    const d = dynamic(v);
    return d.path ? { nameBind: d.path } : d.lit !== undefined ? { name: collapse(d.lit) } : {};
  };
  const textOf = (id: string | undefined): Partial<NNode> => {
    const t = id ? byId.get(id) : undefined;
    const d = dynamic(t?.text);
    return d.path ? { nameBind: d.path } : d.lit !== undefined ? { name: collapse(d.lit) } : {};
  };
  const named = (c: Json): Partial<NNode> => labelOf(c.accessibility?.label);
  const seen = new Set<string>();

  const kids = (list: Json): NNode[] => {
    if (Array.isArray(list)) return list.flatMap((id: string) => conv(id));
    if (list?.componentId && typeof list.path === "string")
      return [
        node("each", {
          each: pointer(list.path).replace("$*.", "$."),
          children: conv(list.componentId),
        }),
      ];
    return [];
  };

  function conv(id: string): NNode[] {
    const c = byId.get(id);
    if (!c || seen.has(id)) return [];
    seen.add(id);
    switch (c.component) {
      case "Column":
      case "Row":
        return [node("stack", { ...named(c), children: kids(c.children) })];
      case "List":
        return [node("list", { ...named(c), children: kids(c.children) })];
      case "Card":
        return [node("card", { ...named(c), children: conv(c.child) })];
      case "Text": {
        const level = /^h([1-5])$/.exec(c.variant ?? "");
        return [
          node(level ? "heading" : "text", {
            ...labelOf(c.text),
            ...(level ? { level: Number(level[1]) } : {}),
          }),
        ];
      }
      case "Image": {
        const d = dynamic(c.url);
        return [node("image", { ...labelOf(c.description), ...(d.path ? { bind: d.path } : {}) })];
      }
      case "Button": {
        const link = c.variant === "borderless";
        // An accessibility label names the button instead of the text it shows.
        const label = named(c);
        const n = node(
          link ? "link" : "button",
          Object.keys(label).length ? label : textOf(c.child),
        );
        if (!link) n.variant = c.variant === "primary" ? "primary" : "secondary";
        const ev = c.action?.event?.name;
        if (typeof ev === "string") n.on.press = ev;
        const url = c.action?.functionCall?.args?.url;
        if (c.action?.functionCall?.call === "openUrl" && url?.path) n.bind = pointer(url.path);
        const disabled = disabledFrom(c.checks);
        if (disabled) n.disabled = disabled;
        return [n];
      }
      case "TextField": {
        const n = node("field", {
          ...labelOf(c.label),
          type:
            { obscured: "password", longText: "multiline", number: "number" }[
              c.variant as string
            ] ?? "text",
        });
        const d = dynamic(c.value);
        if (d.path) n.bind = d.path;
        if ((c.checks ?? []).some((k: Json) => (k.condition ?? k).call === "required"))
          n.required = true;
        return [n];
      }
      case "CheckBox": {
        const n = node("checkbox", labelOf(c.label));
        const d = dynamic(c.value);
        if (d.path) n.bind = d.path;
        return [n];
      }
      case "ChoicePicker": {
        const n = node("choice", labelOf(c.label));
        const d = dynamic(c.value);
        if (d.path) n.bind = d.path;
        n.children = (c.options ?? []).map((o: Json) =>
          node("option", { ...labelOf(o.label), value: o.value }),
        );
        return [n];
      }
      case "Tabs":
        return [
          node("tabs", {
            ...named(c),
            children: (c.tabs ?? []).map((t: Json) =>
              node("tab", { ...labelOf(t.title), children: conv(t.child) }),
            ),
          }),
        ];
      case "Modal":
        // A2UI renders the trigger in place and the content on demand.
        return [...conv(c.trigger), node("dialog", { ...named(c), children: conv(c.content) })];
      default:
        return [];
    }
  }
  const tree = node("screen", { children: conv("root").flatMap((r) => r.children) });
  return { tree, errors };
}
