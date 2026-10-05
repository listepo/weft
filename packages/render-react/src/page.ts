// A complete static HTML page for one document, used by the browser test and the CLI.
import { createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Document } from "@weft/core";
import { render, type RenderOptions } from "./render.ts";

export type PageOptions = RenderOptions & {
  title?: string;
  /** The appearance the tokens were picked for; the browser draws its own parts to match. */
  colorScheme?: "light" | "dark" | undefined;
  /**
   * A stylesheet for the page's `<style>`, such as `weft css-base`. The reference renderer
   * resolves tokens itself, so it has no use for the token stylesheet; the base rules are what
   * a host adds to the components, and without them the page would look different.
   */
  styles?: string | undefined;
};

export function renderPage(document: Document, options: PageOptions): string {
  // The stylesheet is the caller's own text, written unescaped into the element.
  if (options.styles !== undefined && /<\/style/i.test(options.styles)) {
    throw new Error("the stylesheet would close its <style> element");
  }
  const page = h(
    "html",
    { lang: "en" },
    h(
      "head",
      null,
      h("meta", { charSet: "utf-8" }),
      h("meta", { name: "viewport", content: "width=device-width, initial-scale=1" }),
      options.colorScheme === undefined
        ? null
        : h("meta", { name: "color-scheme", content: options.colorScheme }),
      h("title", null, options.title ?? "Weft"),
      options.styles === undefined
        ? null
        : h("style", { dangerouslySetInnerHTML: { __html: options.styles } }),
    ),
    h("body", null, render(document, options)),
  );
  return `<!doctype html>\n${renderToStaticMarkup(page)}\n`;
}
