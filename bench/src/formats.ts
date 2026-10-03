import { parseA2ui } from "./adapters/a2ui.ts";
import { parseHtml } from "./adapters/html.ts";
import { parseJsx } from "./adapters/jsx.ts";
import type { Parsed } from "./adapters/types.ts";
import { parseWeft } from "./adapters/weft.ts";
import type { Format } from "./neutral.ts";

export const FILE_NAME: Record<Format, string> = {
  weft: "screen.weft",
  html: "screen.html",
  jsx: "screen.jsx",
  a2ui: "screen.a2ui.json",
};

export const parsers: Record<Format, (src: string) => Parsed> = {
  weft: parseWeft,
  html: parseHtml,
  jsx: parseJsx,
  a2ui: parseA2ui,
};

export const FENCE_LANG: Record<Format, string> = {
  weft: "xml",
  html: "html",
  jsx: "jsx",
  a2ui: "json",
};

/** Models usually wrap the document in a code fence; take its body, else the whole reply. */
export function extractDocument(reply: string): string {
  const fenced = /```[\w-]*\n([\s\S]*?)```/.exec(reply);
  return (fenced ? (fenced[1] as string) : reply).trim();
}
