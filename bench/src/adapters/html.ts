import { parseDocument, Parser } from "htmlparser2";
import { htmlLikeToNeutral, type HChild, type HEl } from "./html-like.ts";
import type { Parsed } from "./types.ts";

const VOID = new Set(["input", "img", "br", "hr", "meta", "link"]);
const MAY_CLOSE_IMPLICITLY = new Set([
  "p",
  "li",
  "td",
  "th",
  "tr",
  "option",
  "thead",
  "tbody",
  "dt",
  "dd",
  "body",
  "html",
  "head",
]);
const BIND = /^[a-z]+:\s*!?\$[A-Za-z_.0-9*]+$/;
const ACTION = /^[a-z]+:\s*[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*$/;

type DomNode = ReturnType<typeof parseDocument>["children"][number];
interface DomEl {
  type: string;
  name: string;
  attribs: Record<string, string>;
  children: DomNode[];
}
const isTag = (n: DomNode): n is DomNode & DomEl => n.type === "tag";

function entries(raw: string | undefined): [string, string][] {
  return (raw ?? "")
    .split(";")
    .map((s) => s.trim())
    .filter(Boolean)
    .map((s) => {
      const i = s.indexOf(":");
      return [s.slice(0, i).trim(), s.slice(i + 1).trim()];
    });
}

function toHEl(el: DomEl): HEl {
  const attrs = { ...el.attribs };
  const bind = Object.fromEntries(entries(attrs["data-bind"]));
  const on = Object.fromEntries(entries(attrs["data-action"]));
  delete attrs["data-bind"];
  delete attrs["data-action"];
  const children: HChild[] = el.children.flatMap((c): HChild[] => {
    if (c.type === "text") return [(c as unknown as { data: string }).data];
    return isTag(c) ? [toHEl(c)] : [];
  });
  return { tag: el.name, attrs, bind, on, children };
}

export function parseHtml(src: string): Parsed {
  const errors: string[] = [];
  const parser = new Parser({
    onclosetag(name, implied) {
      if (implied && !VOID.has(name) && !MAY_CLOSE_IMPLICITLY.has(name))
        errors.push(`element <${name}> is not closed`);
    },
    onattribute(name, value) {
      if (name === "data-bind")
        for (const [k, v] of entries(value))
          if (!BIND.test(`${k}:${v}`)) errors.push(`malformed data-bind entry "${k}:${v}"`);
      if (name === "data-action")
        for (const [k, v] of entries(value))
          if (!ACTION.test(`${k}:${v}`)) errors.push(`malformed data-action entry "${k}:${v}"`);
    },
  });
  parser.write(src);
  parser.end();
  const root = parseDocument(src).children.find(isTag);
  if (!root) return { errors: [...errors, "no element found"] };
  return { tree: htmlLikeToNeutral(toHEl(root)), errors };
}
