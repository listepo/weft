// Builders for JSON documents and probes into rendered output, shared by the tests.
import { isValidElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { DomUtils, parseDocument } from "htmlparser2";
import { coreCatalog } from "@weft/catalog";
import type { Child, Document, Node, Value } from "@weft/core";
import { render, type RenderOptions } from "../src/index.ts";

export const b = (path: string): Value => ({ bind: path });
export const nb = (path: string): Value => ({ bind: path, not: true });

export function el(
  kind: string,
  id: string | undefined,
  props: Record<string, Value> = {},
  children: Child[] = [],
  extra: { on?: Record<string, string>; slots?: Record<string, Child[]> } = {},
): Node {
  const n: Node = { kind };
  if (id !== undefined) n.id = id;
  if (Object.keys(props).length > 0) n.props = props;
  if (extra.on) n.on = extra.on;
  if (extra.slots) n.slots = extra.slots;
  if (children.length > 0) n.children = children;
  return n;
}

export const doc = (...children: Child[]): Document => ({
  weft: "0.1",
  root: el("screen", "root", { weft: "0.1", label: "Test" }, children),
});

export type Options = Partial<RenderOptions>;
const full = (o: Options): RenderOptions => ({ catalog: coreCatalog, ...o });

export const html = (d: Document, o: Options = {}) => renderToStaticMarkup(render(d, full(o)));

type El = { name: string; attribs: Record<string, string> };

// Parses rendered markup and finds elements the way a test reads them: by document id.
export function dom(d: Document, o: Options = {}) {
  const root = parseDocument(html(d, o));
  const find = (test: (e: El) => boolean) =>
    DomUtils.findOne((e) => test(e as El), root.children, true) as (El & object) | null;
  return {
    root,
    byId(id: string): El {
      const e = find((x) => x.attribs["data-weft-id"] === id);
      if (!e) throw new Error(`no element with data-weft-id=${id}`);
      return e;
    },
    has: (id: string) => find((x) => x.attribs["data-weft-id"] === id) !== null,
    find,
    all: (test: (e: El) => boolean) =>
      DomUtils.findAll((e) => test(e as El), root.children) as unknown as El[],
    text: (e: El) => DomUtils.textContent(e as never),
  };
}

type Props = Record<string, unknown>;

// Walks the React element tree (render() emits host elements only) to reach event handlers.
export function propsOf(node: ReactNode, id: string): Props {
  const hit = walk(node, (p) => p["data-weft-id"] === id);
  if (!hit) throw new Error(`no element with data-weft-id=${id}`);
  return hit;
}

function walk(node: ReactNode, test: (p: Props) => boolean): Props | undefined {
  if (Array.isArray(node)) {
    for (const c of node) {
      const hit = walk(c as ReactNode, test);
      if (hit) return hit;
    }
    return undefined;
  }
  if (!isValidElement(node)) return undefined;
  const p = node.props as Props;
  if (test(p)) return p;
  return walk(p["children"] as ReactNode, test);
}

export const tree = (d: Document, o: Options = {}) => render(d, full(o));
