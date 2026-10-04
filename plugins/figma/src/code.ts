// The plugin's main code, run by Figma in its main-thread sandbox. It only wires the `figma`
// global to the handler in @weft/figma; parsing and serializing happen in the UI (`ui.ts`).
import type { PluginAPI, SceneNode } from "@figma/plugin-typings/plugin-api-standalone.js";
import { coreCatalog } from "@weft/catalog";
import { handleRequest } from "@weft/figma";

declare const figma: PluginAPI;
declare const __html__: string;

figma.showUI(__html__, { width: 400, height: 560, themeColors: true });

figma.ui.onmessage = async (message: unknown) => {
  const reply = await handleRequest(figma, figma.currentPage.selection, message, {
    catalog: coreCatalog,
  });
  if (reply.type === "built") {
    const node = await figma.getNodeByIdAsync(reply.id);
    if (node !== null && "visible" in node) {
      const scene = node as SceneNode;
      figma.currentPage.selection = [scene];
      figma.viewport.scrollAndZoomIntoView([scene]);
    }
  }
  figma.ui.postMessage(reply);
};
