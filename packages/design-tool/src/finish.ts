// The steps of a build and a read that need the WebAssembly core. They run wherever the core does:
// in Node, and in a plugin's UI iframe, never in the tool's plugin sandbox.
import { tokenTypes, type Token } from "@weft/catalog";
import {
  canonicalize,
  formatValue,
  readValue,
  validate,
  type Catalog,
  type Document,
  type Node,
} from "@weft/core";
import { literal, type LossLog } from "@weft/from-aria";
import type { Layer } from "./layer.ts";
import { isRawText, readLayers, type ReadOptions, type ReadResult } from "./read.ts";

/** The props a layer shows as text, and so the ones a designer can type a value into. */
const SHOWN_PROPS = ["text", "label"] as const;

function eachNode(root: Node, visit: (node: Node) => void): void {
  const stack = [root];
  for (let node = stack.pop(); node !== undefined; node = stack.pop()) {
    visit(node);
    for (const child of node.children ?? []) if (typeof child !== "string") stack.push(child);
    for (const list of Object.values(node.slots ?? {}))
      for (const child of list) if (typeof child !== "string") stack.push(child);
  }
}

/** The `display` that `buildScreen` needs: every `text` and `label` value in attribute form. */
export function displayTexts(document: Document): Record<string, string> {
  const display: Record<string, string> = {};
  eachNode(document.root, (node) => {
    for (const name of SHOWN_PROPS) {
      const value = node.props?.[name];
      if (value !== undefined) display[JSON.stringify(value)] = formatValue(value);
    }
  });
  return display;
}

/** Replaces each `RawText` left by `readLayers` with the value the core reads from it. */
function readRawTexts(document: Document, log: LossLog): void {
  eachNode(document.root, (node) => {
    const props = node.props as Record<string, unknown> | undefined;
    for (const name of SHOWN_PROPS) {
      const pending = props?.[name];
      if (props === undefined || !isRawText(pending)) continue;
      const read = readValue(pending.raw);
      // Text that is not a well-formed reference is kept as the designer typed it.
      const value = read.ok ? read.value : pending.raw;
      const settled = typeof value === "string" ? literal(log, pending.path, value) : value;
      if (pending.content === true && typeof settled === "string") {
        delete props[name];
        node.children = [settled];
      } else {
        props[name] = settled;
      }
    }
  });
}

/** Reads typed text, canonicalizes what `readLayers` returned and adds validation diagnostics. */
export function finishRead(
  read: ReadResult,
  options: { catalog: Catalog; tokens?: ReadonlyMap<string, Token> | undefined },
): ReadResult {
  const losses = [...read.losses];
  readRawTexts(read.document, { losses });
  const document: Document = canonicalize(read.document);
  const tokens = options.tokens === undefined ? undefined : tokenTypes(new Map(options.tokens));
  const diagnostics = [
    ...read.diagnostics,
    ...validate(document, { catalog: options.catalog, tokens }),
  ];
  return { document, losses, diagnostics };
}

/** Reads a layer back into a canonical, validated Weft document. */
export async function readScreen(layer: Layer, options: ReadOptions): Promise<ReadResult> {
  return finishRead(await readLayers(layer, options), options);
}
