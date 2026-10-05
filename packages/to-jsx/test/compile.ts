// Turns generated source into a live component the way a host build would: oxc-transform with
// the automatic JSX runtime for React, babel-preset-solid's server output for SolidJS, then a
// module import. Bare specifiers are rewritten to the files this package resolves, because a
// `data:` module has no node_modules to search.
import { transformSync as babel } from "@babel/core";
import { createRequire } from "node:module";
import { createElement, type ComponentType } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { transformSync } from "oxc-transform";

export type Props = { data?: unknown; actions?: unknown; onChange?: unknown };

// Babel loads a preset from a path; the preset ships no types to import it by.
const SOLID_PRESET = createRequire(import.meta.url).resolve("babel-preset-solid");

// Node's resolution picks solid-js's server build, the one renderToString needs.
const SPECIFIERS: Record<string, string> = {
  react: import.meta.resolve("react"),
  "react/jsx-runtime": import.meta.resolve("react/jsx-runtime"),
  "solid-js": import.meta.resolve("solid-js"),
  "solid-js/web": import.meta.resolve("solid-js/web"),
};

function oxc(source: string, typescript: boolean, jsx: "preserve" | "react"): string {
  const result = transformSync(typescript ? "screen.tsx" : "screen.jsx", source, {
    jsx: jsx === "preserve" ? "preserve" : { runtime: "automatic" },
  });
  if (result.errors.length > 0) {
    throw new Error(result.errors.map((e) => e.message).join("\n"));
  }
  return result.code;
}

const resolveSpecifiers = (code: string): string =>
  code.replace(/from "([^"]+)"/g, (_, s: string) => {
    const url = SPECIFIERS[s];
    if (url === undefined) throw new Error(`unexpected import ${s}`);
    return `from ${JSON.stringify(url)}`;
  });

export function transform(source: string, typescript = false): string {
  return resolveSpecifiers(oxc(source, typescript, "react"));
}

/** Types stripped by oxc, then the JSX compiled the way a SolidStart server build does. */
export function transformSolid(source: string, typescript = false): string {
  const result = babel(oxc(source, typescript, "preserve"), {
    babelrc: false,
    configFile: false,
    filename: "screen.jsx",
    presets: [[SOLID_PRESET, { generate: "ssr", hydratable: false }]],
  });
  if (typeof result?.code !== "string") throw new Error("babel-preset-solid produced no code");
  return resolveSpecifiers(result.code);
}

const importModule = async <T>(code: string): Promise<T> =>
  (await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`)) as T;

export async function load(source: string, typescript = false): Promise<ComponentType<Props>> {
  return (await importModule<{ default: ComponentType<Props> }>(transform(source, typescript)))
    .default;
}

type SolidComponent = (props: Props) => unknown;

/** A generated SolidJS component, server-rendered to static markup. */
export async function loadSolid(
  source: string,
  typescript = false,
): Promise<(props: Props) => string> {
  const component = await importModule<{ default: SolidComponent }>(
    transformSolid(source, typescript),
  );
  // The renderer must be the instance the component imported, so it is loaded from the same file.
  const web = await importModule<{ renderToString: (fn: () => unknown) => string }>(
    `export { renderToString } from ${JSON.stringify(SPECIFIERS["solid-js/web"])};`,
  );
  return (props) => web.renderToString(() => component.default(props));
}

export const markup = (component: ComponentType<Props>, props: Props): string =>
  renderToStaticMarkup(createElement(component, props));

// Static markup with each tag's attributes sorted and React's separators between adjacent text
// nodes removed: neither attribute order nor how a text run is split is part of the rendering.
// Attribute names, an empty attribute and a void element's closing slash are spelled one way,
// because React writes `colSpan`, `checked=""` and `<input/>` where SolidJS writes `colspan`,
// `checked` and `<input>`, and HTML reads them alike. Two renderer habits that are not part of the screen are dropped: the image preload hints
// React 19 hoists in front of the markup, and the `value` SolidJS writes on a <select>, which HTML
// ignores (the chosen option carries `selected`). Every input comes from a server renderer, which
// always quotes values and escapes quotes.
export const normalizeMarkup = (html: string): string =>
  html
    .replace(/<!-- -->/g, "")
    .replace(/<link (?=[^>]*\brel="preload")[^>]*>/g, "")
    .replace(/<([a-z][a-z0-9-]*)((?:\s+[^\s=/>]+(?:="[^"]*")?)*)\s*\/?>/g, (_, tag, attrs) => {
      const sorted = ((attrs as string).match(/[^\s=]+(?:="[^"]*")?/g) ?? [])
        .filter((a) => tag !== "select" || !a.startsWith("value="))
        .map((a) => {
          const at = a.indexOf("=");
          return at < 0 ? `${a.toLowerCase()}=""` : a.slice(0, at).toLowerCase() + a.slice(at);
        })
        .sort();
      return `<${tag as string}${sorted.map((a) => ` ${a}`).join("")}>`;
    });
