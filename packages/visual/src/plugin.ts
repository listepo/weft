// A Vite plugin that serves every corpus screen to the browser tests as modules:
// `virtual:weft-screens` lists the screens with their markup and pages, and
// `virtual:weft-component/<variant>/<screen>` is one compiled component, imported on demand.
import type { Plugin } from "vite";
import { screens, type Screen } from "./artifacts.ts";

const SCREENS = "virtual:weft-screens";
const COMPONENT = "virtual:weft-component/";

/** The component variants a screen has: generated, and generated after a round trip. */
export const VARIANTS = ["react", "solid", "react-back", "solid-back"] as const;
export type Variant = (typeof VARIANTS)[number];

function code(screen: Screen, variant: Variant): string {
  switch (variant) {
    case "react":
      return screen.react;
    case "solid":
      return screen.solid;
    case "react-back":
      return screen.back.react;
    case "solid-back":
      return screen.back.solid;
  }
}

export function weftScreens(): Plugin {
  let all: Promise<Screen[]> | undefined;
  const load = () => (all ??= screens());
  return {
    name: "weft-screens",
    resolveId(id) {
      return id === SCREENS || id.startsWith(COMPONENT) ? `\0${id}` : undefined;
    },
    async load(id) {
      if (id === `\0${SCREENS}`) {
        const list = await load();
        const pages = list.map(({ react: _r, solid: _s, back, ...rest }) => ({
          ...rest,
          back: { html: back.html, figma: back.figma, penpot: back.penpot },
        }));
        const imports = list.map(
          (s) =>
            `${JSON.stringify(s.name)}: {${VARIANTS.map(
              (v) =>
                `${JSON.stringify(v)}: () => import(${JSON.stringify(`${COMPONENT}${v}/${s.name}`)})`,
            ).join(", ")}}`,
        );
        return `export const screens = ${JSON.stringify(pages)};\nexport const components = {${imports.join(",\n")}};\n`;
      }
      if (id.startsWith(`\0${COMPONENT}`)) {
        const [variant, name] = id.slice(COMPONENT.length + 1).split("/");
        const screen = (await load()).find((s) => s.name === name);
        if (screen === undefined) throw new Error(`no corpus screen ${name}`);
        return code(screen, variant as Variant);
      }
      return undefined;
    },
  };
}
