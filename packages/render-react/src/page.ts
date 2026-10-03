// A complete static HTML page for one document, used by the browser test and the CLI.
import { createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Document } from "@weft/core";
import { render, type RenderOptions } from "./render.ts";

export type PageOptions = RenderOptions & { title?: string };

export function renderPage(document: Document, options: PageOptions): string {
  const page = h(
    "html",
    { lang: "en" },
    h(
      "head",
      null,
      h("meta", { charSet: "utf-8" }),
      h("meta", { name: "viewport", content: "width=device-width, initial-scale=1" }),
      h("title", null, options.title ?? "Weft"),
    ),
    h("body", null, render(document, options)),
  );
  return `<!doctype html>\n${renderToStaticMarkup(page)}\n`;
}
