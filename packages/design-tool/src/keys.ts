// What a plugin stores on design-tool layers (plugin data, one string per key) and how it is read
// back. Stored data is untrusted on the way in: a file can be copied, edited by another version of
// the plugin or crafted, so every entry is parsed and checked, never assumed.
import { ValueSchema, type Node, type Value } from "@weft/core";
import { z } from "zod";

/** Plugin data as both tools offer it: `""` stands for a key that is not set. */
export interface PluginData {
  getPluginData(key: string): string;
  setPluginData(key: string, value: string): void;
}

export const KEY = {
  /** On a layer built from a Weft element: its kind, id, props and events (`Source`). */
  source: "weft.source",
  /** On the layer built from the root: the document's `weft` version. */
  document: "weft.document",
  /** The tool's id the layer had when it was built, to tell the original from a copy. */
  origin: "weft.origin",
  /** On a text layer that holds a text child of its parent. */
  text: "weft.text",
  /** On the text layer that shows a container's `label`. */
  label: "weft.label",
  /** On a frame that holds a named slot: the slot name. */
  slot: "weft.slot",
  /** A fingerprint of the visual properties the build set, to notice visual edits. */
  style: "weft.style",
  /** On library components (and Figma component sets): the catalog kind. */
  kind: "weft.kind",
  /** On the library page: the catalog name and version it was built from. */
  library: "weft.library",
  /** On a library variable (Figma): the design token path. */
  token: "weft.token",
} as const;

/**
 * What a layer remembers of its Weft element. Element children come from the layer tree; only a
 * leaf (a library instance) keeps its text content here, to tell a text edit from the original.
 */
export type Source = {
  kind: string;
  id?: string | undefined;
  props?: Record<string, Value> | undefined;
  on?: Record<string, string> | undefined;
  children?: string[] | undefined;
  /** The text and label the layer showed when built, to tell a designer's edit from the build. */
  shown?: Shown | undefined;
};

export type Shown = { text?: string | undefined; label?: string | undefined };

// Figma allows 100 kB per entry; a larger value cannot be ours in either tool.
const MAX_ENTRY = 100_000;

const SourceSchema = z.strictObject({
  kind: z.string(),
  id: z.string().optional(),
  props: z.record(z.string(), ValueSchema).optional(),
  on: z.record(z.string(), z.string()).optional(),
  children: z.array(z.string()).optional(),
  shown: z.strictObject({ text: z.string().optional(), label: z.string().optional() }).optional(),
});

export function sourceOf(node: Node, leaf: boolean, shown: Shown): Source {
  const source: Source = { kind: node.kind };
  if (node.id !== undefined) source.id = node.id;
  if (node.props !== undefined) source.props = node.props;
  if (node.on !== undefined) source.on = node.on;
  if (leaf && node.children !== undefined)
    source.children = node.children.filter((c) => typeof c === "string");
  if (shown.text !== undefined || shown.label !== undefined) source.shown = shown;
  return source;
}

function readJson(layer: PluginData, key: string): unknown {
  const raw = layer.getPluginData(key);
  if (raw === "" || raw.length > MAX_ENTRY) return undefined;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return undefined;
  }
}

export function readSource(layer: PluginData): Source | undefined {
  const parsed = SourceSchema.safeParse(readJson(layer, KEY.source));
  if (!parsed.success) return undefined;
  const { kind, id, props, on, children, shown } = parsed.data;
  // Rebuilt with own properties only, so a key such as `__proto__` stays data.
  const source: Source = { kind };
  if (id !== undefined) source.id = id;
  if (props !== undefined) source.props = Object.fromEntries(Object.entries(props));
  if (on !== undefined) source.on = Object.fromEntries(Object.entries(on));
  if (children !== undefined) source.children = children;
  if (shown !== undefined) source.shown = { text: shown.text, label: shown.label };
  return source;
}

export function writeJson(layer: PluginData, key: string, value: unknown): void {
  layer.setPluginData(key, JSON.stringify(value));
}

export function readVersion(layer: PluginData): string | undefined {
  const parsed = z.strictObject({ weft: z.string() }).safeParse(readJson(layer, KEY.document));
  return parsed.success ? parsed.data.weft : undefined;
}

/** A marker key that holds a short plain string (`kind`, `slot`, `token`), or undefined. */
export function readMark(layer: PluginData, key: string): string | undefined {
  const raw = layer.getPluginData(key);
  return raw === "" || raw.length > 1_000 ? undefined : raw;
}
