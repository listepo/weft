// Weft to Figma: the shared build of @weft/design-tool, drawn with Figma frames, auto layout and
// library instances. Every element remembers its Weft source in plugin data, so that reading the
// layers back returns the document as written.
import type { Token } from "@weft/catalog";
import type { Catalog, Document } from "@weft/core";
import {
  buildScreen as buildShared,
  findBelow,
  type BuildHost,
  type Display,
} from "@weft/design-tool";
import type { FComponent, FFrame, FigmaApi, FInstance, FNode, FText } from "./api.ts";
import { isContainer } from "./api.ts";
import { styleKey } from "./layer.ts";
import { autoLayout, bindNumber, drawText, loadFonts, paint, type Library } from "./library.ts";

export type BuildOptions = {
  catalog: Catalog;
  library: Library;
  tokens?: ReadonlyMap<string, Token> | undefined;
  /** The document's `text` and `label` values in attribute form, from `displayTexts`. */
  display: Display;
};

const SCREEN_WIDTH = 480;
const SCREEN_GAP = 120;

type Built = FFrame | FInstance | FText;

function figmaHost(api: FigmaApi, library: Library): BuildHost<Built, FComponent> {
  let right = 0;
  return {
    async prepare() {
      await loadFonts(api);
      right = api.currentPage.children.reduce((x, n) => Math.max(x, n.x + n.width + SCREEN_GAP), 0);
    },
    instance(main, values) {
      const instance = main.createInstance();
      if (values !== undefined) instance.setProperties(values);
      return instance;
    },
    frame(view, fill) {
      const frame = api.createFrame();
      autoLayout(frame, view.mode === "row" ? "HORIZONTAL" : "VERTICAL", view.gap.px, view.padding);
      frame.fills = fill === undefined ? [] : [paint(api, library, fill)];
      if (view.material !== undefined) {
        // The tint is the fill, its opacity the paint's; the blur is Figma's own background blur.
        frame.fills = [
          { type: "SOLID", color: view.material.color, opacity: view.material.opacity },
        ];
        frame.effects = [{ type: "BACKGROUND_BLUR", radius: view.material.blur, visible: true }];
      }
      if (view.mode === "grid") {
        frame.layoutMode = "GRID";
        frame.gridColumnCount = view.columns;
        bindNumber(frame, ["gridRowGap", "gridColumnGap"], library, view.gap.token, view.gap.px);
      } else {
        if (view.wrap) frame.layoutWrap = "WRAP";
        frame.counterAxisAlignItems = { start: "MIN", center: "CENTER", end: "MAX" }[
          view.align
        ] as FFrame["counterAxisAlignItems"];
        bindNumber(frame, ["itemSpacing"], library, view.gap.token, view.gap.px);
      }
      return frame;
    },
    text: (drawing) => drawText(api, library, drawing),
    setText: (layer, name, value) => setLayerText(api, layer, name, value),
    append(parent, child) {
      if (parent.type === "TEXT") throw new Error("a text layer cannot hold layers");
      parent.appendChild(child);
    },
    marks: (layer) => layer,
    style: (layer, withSpacing) => (layer.type === "TEXT" ? "" : styleKey(layer, withSpacing)),
    place(root, screen) {
      api.currentPage.appendChild(root);
      root.x = right;
      root.y = 0;
      if (root.type === "FRAME" && screen) {
        root.resize(SCREEN_WIDTH, Math.max(root.height, 1));
        root.counterAxisSizingMode = "FIXED";
      }
    },
  };
}

/**
 * Builds the document on the current page, to the right of what is there, and returns its frame.
 * The document must be canonical, as `parse` and `canonicalize` return it: canonicalizing here
 * would need the WebAssembly core, which Figma's main thread cannot run.
 */
export async function buildScreen(
  api: FigmaApi,
  document: Document,
  options: BuildOptions,
): Promise<FFrame | FInstance> {
  const { library, ...rest } = options;
  const root = await buildShared(figmaHost(api, library), document, {
    ...rest,
    kinds: library.kinds,
  });
  if (root.type === "TEXT") throw new Error("a document root cannot be a text layer");
  return root;
}

/** The first text layer with this name, searched breadth-first below a layer. */
export const findText = (layer: FNode, name: string): FText | undefined =>
  findBelow<FNode>(
    layer,
    (c) => c.type === "TEXT" && c.name === name,
    (n) => (isContainer(n) ? n.children : undefined),
  ) as FText | undefined;

async function setLayerText(
  api: FigmaApi,
  layer: FNode,
  name: string,
  value: string,
): Promise<void> {
  const text = findText(layer, name);
  if (text === undefined) return;
  if (typeof text.fontName !== "symbol") await api.loadFontAsync(text.fontName);
  text.characters = value;
}
