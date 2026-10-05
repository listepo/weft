// The modules `src/plugin.ts` serves to the browser tests.
declare module "virtual:weft-appearance" {
  export const example: {
    data: unknown;
    html: string;
    css: string;
    reference: { light: string; dark: string };
  };
}
declare module "virtual:weft-appearance-component" {
  const component: unknown;
  export default component;
}
declare module "virtual:weft-screens" {
  type Component = { default: unknown };
  export const screens: {
    name: string;
    data: unknown;
    reference: string;
    html: string;
    css: string;
    back: { html: string; figma: string; penpot: string };
  }[];
  export const components: Record<
    string,
    Record<"react" | "solid" | "react-back" | "solid-back", () => Promise<Component>>
  >;
}
