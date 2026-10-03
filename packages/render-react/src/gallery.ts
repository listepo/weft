// Writes one static page per corpus screen, rendered with its data and the default tokens, plus an
// index linking them, for looking at the renderer's output in a browser:
//   node src/gallery.ts [out-dir]     (default: packages/render-react/gallery/, git-ignored)
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createElement as h } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { corpusScreens } from "./corpus.ts";
import { renderPage } from "./page.ts";

const out = process.argv[2] ?? fileURLToPath(new URL("../gallery/", import.meta.url));
const { tokens } = loadTokens(
  JSON.parse(
    readFileSync(new URL("../../catalog/tokens/default.tokens.json", import.meta.url), "utf8"),
  ),
);

mkdirSync(out, { recursive: true });
const written: string[] = [];
for (const { name, document, diagnostics, data } of corpusScreens()) {
  if (document === undefined) {
    console.error(`${name}: not rendered, ${diagnostics.length} diagnostics`);
    continue;
  }
  writeFileSync(
    `${out}/${name}.html`,
    renderPage(document, { catalog: coreCatalog, data, tokens, title: `Weft corpus: ${name}` }),
  );
  written.push(name);
}

const index = h(
  "html",
  { lang: "en" },
  h("head", null, h("meta", { charSet: "utf-8" }), h("title", null, "Weft corpus gallery")),
  h(
    "body",
    null,
    h("h1", null, "Weft corpus gallery"),
    h("ul", null, ...written.map((name) => h("li", null, h("a", { href: `${name}.html` }, name)))),
  ),
);
writeFileSync(`${out}/index.html`, `<!doctype html>\n${renderToStaticMarkup(index)}\n`);
console.log(`${written.length} screens and index.html written to ${out}`);
