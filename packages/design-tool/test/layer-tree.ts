// A design tool's layer tree as reviewable text, for the per-tool file snapshots. The fakes keep
// layers as plain objects, so a layer is printed from its own properties: what differs from the
// fake's defaults, the plugin data Weft stores on it, and its children, indented.
//
// Ids come from counters shared by every fake in the process, so they depend on test order; they
// are replaced by the names of what they point at, which keeps a snapshot stable.

export type TreeOptions = {
  /** The layer's children, or undefined for a leaf. */
  children: (layer: object) => readonly object[] | undefined;
  /** The layer's type and name for its heading line. */
  heading: (layer: object) => string;
  /** Own properties left out: links, ids and geometry the fakes do not compute. */
  skip: ReadonlySet<string>;
  /** A property equal to its default is left out, so a layer lists only what was set. */
  defaults: Readonly<Record<string, unknown>>;
  /** Lines a layer adds below its heading: its text and the plugin data on it. */
  extra: (layer: object) => string[];
  /** Id → what to print in its place. */
  ids: ReadonlyMap<string, string>;
  /** Classes printed with their fields (a layout's settings); any other object is a link to a
   * layer or component and is printed by its name. */
  inline?: ReadonlySet<string>;
};

const hex = (n: number) =>
  Math.round(n * 255)
    .toString(16)
    .padStart(2, "0");

/** JSON with colors as hex and linked objects (layers, components) by their name. */
function show(value: unknown, inline: ReadonlySet<string>): string {
  return JSON.stringify(value, (_key, v: unknown) => {
    if (v instanceof Map) return Object.fromEntries([...v].sort(([a], [b]) => (a < b ? -1 : 1)));
    if (v === null || typeof v !== "object" || Array.isArray(v)) return v;
    const proto = Object.getPrototypeOf(v);
    if (proto !== Object.prototype && proto !== null && !inline.has(proto.constructor.name)) {
      const name = (v as { name?: unknown }).name;
      return `<${typeof name === "string" ? name : proto.constructor.name}>`;
    }
    const c = v as Record<string, unknown>;
    if (c.type === "VARIABLE_ALIAS" && typeof c.id === "string") return c.id;
    if (typeof c.r === "number" && typeof c.g === "number" && typeof c.b === "number") {
      const alpha = typeof c.a === "number" && c.a !== 1 ? hex(c.a) : "";
      return `#${hex(c.r)}${hex(c.g)}${hex(c.b)}${alpha}`;
    }
    return v;
  });
}

export function layerTree(root: object, options: TreeOptions): string {
  const lines: string[] = [];
  const inline = options.inline ?? new Set();
  const walk = (layer: object, depth: number) => {
    const pad = "  ".repeat(depth);
    const props = Object.entries(layer)
      .filter(([k]) => !options.skip.has(k))
      .map(([k, v]) => [k, show(v, inline)] as const)
      .filter(([k, v]) => !(k in options.defaults) || show(options.defaults[k], inline) !== v)
      .map(([k, v]) => `${k}=${v}`);
    lines.push(`${pad}${options.heading(layer)}${props.length > 0 ? ` ${props.join(" ")}` : ""}`);
    for (const line of options.extra(layer)) lines.push(`${pad}  | ${line}`);
    for (const child of options.children(layer) ?? []) walk(child, depth + 1);
  };
  walk(root, 0);
  const text = `${lines.join("\n")}\n`;
  if (options.ids.size === 0) return text;
  const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // Whole ids only, so `1:12` is not replaced inside `1:123`.
  const any = new RegExp(
    `(?<![\\w:-])(?:${[...options.ids.keys()].map(escape).join("|")})(?!\\w)`,
    "g",
  );
  return text.replace(any, (id) => options.ids.get(id) ?? id);
}
