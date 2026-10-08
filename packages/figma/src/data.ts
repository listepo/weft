// Where the Weft source lives on a Figma node: shared plugin data under `NAMESPACE`, so the Figma
// MCP server's `use_figma`, other plugins and the REST API (`plugin_data=shared`) can read it.
// Private plugin data, readable only by the plugin id that wrote it, is not used.
import type { PluginData } from "@weft/design-tool";
import type { FPluginData } from "./api.ts";

/** The shared plugin data namespace; Penpot's plugin uses the same one. */
export const NAMESPACE = "weft";

/** The node's Weft data, as the shared build and read-back see it. */
export function dataOf(node: FPluginData): PluginData {
  return {
    getPluginData: (key) => node.getSharedPluginData(NAMESPACE, key),
    setPluginData: (key, value) => node.setSharedPluginData(NAMESPACE, key, value),
  };
}
