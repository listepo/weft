// Turns generated source into a live component the way a host build would: oxc-transform with
// the automatic JSX runtime, then a module import. The bare `react` specifiers are rewritten to
// the files this package resolves, because a `data:` module has no node_modules to search.
import { createElement, type ComponentType } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { transformSync } from "oxc-transform";

export type Props = { data?: unknown; actions?: unknown; onChange?: unknown };

const SPECIFIERS: Record<string, string> = {
  react: import.meta.resolve("react"),
  "react/jsx-runtime": import.meta.resolve("react/jsx-runtime"),
};

export function transform(source: string): string {
  const result = transformSync("screen.jsx", source, { jsx: { runtime: "automatic" } });
  if (result.errors.length > 0) {
    throw new Error(result.errors.map((e) => e.message).join("\n"));
  }
  return result.code.replace(/from "(react(?:\/jsx-runtime)?)"/g, (_, s: string) => {
    const url = SPECIFIERS[s];
    if (url === undefined) throw new Error(`unexpected import ${s}`);
    return `from ${JSON.stringify(url)}`;
  });
}

export async function load(source: string): Promise<ComponentType<Props>> {
  const code = transform(source);
  const module = (await import(
    `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`
  )) as {
    default: ComponentType<Props>;
  };
  return module.default;
}

export const markup = (component: ComponentType<Props>, props: Props): string =>
  renderToStaticMarkup(createElement(component, props));
