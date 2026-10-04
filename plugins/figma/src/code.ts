// The plugin's main code, run by Figma in its sandbox. It only wires the `figma` global to the
// pure handler in @weft/figma, so everything it does is tested without Figma.
import type { PluginAPI, SceneNode } from "@figma/plugin-typings/plugin-api-standalone.js";
import { coreCatalog, loadTokens } from "@weft/catalog";
import { handleRequest } from "@weft/figma";

declare const figma: PluginAPI;
declare const __html__: string;
/** The default token set, inlined by the bundler: the sandbox cannot read files. */
declare const WEFT_DEFAULT_TOKENS: unknown;

const { tokens } = loadTokens(WEFT_DEFAULT_TOKENS);

figma.showUI(__html__, { width: 400, height: 560, themeColors: true });

figma.ui.onmessage = async (message: unknown) => {
  const reply = await handleRequest(figma, figma.currentPage.selection, message, {
    catalog: coreCatalog,
    tokens,
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
