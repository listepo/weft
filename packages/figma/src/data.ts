// Where the Weft source lives on a Figma node: shared plugin data under `NAMESPACE`, so the Figma
// MCP server's `use_figma`, other plugins and the REST API (`plugin_data=shared`) can read it.
// Files built before T14.2 hold it in private plugin data, which only this plugin's id can read;
// those values are still read, and moved to shared data when touched.
import type { PluginData } from "@weft/design-tool";
import type { FPluginData } from "./api.ts";

/** The shared plugin data namespace; Penpot's plugin uses the same one. */
export const NAMESPACE = "weft";

/** Best effort: a node that cannot be written (a REST node, a file opened read-only) still reads. */
function tryWrite(write: () => void): void {
  try {
    write();
  } catch {
    // The value stays where it is and is moved on a later write.
  }
}

/** The node's Weft data: shared first, then private, which is moved to shared when read. */
export function dataOf(node: FPluginData): PluginData {
  return {
    getPluginData: (key) => {
      const shared = node.getSharedPluginData(NAMESPACE, key);
      if (shared !== "") return shared;
      const legacy = node.getPluginData(key);
      if (legacy !== "")
        tryWrite(() => {
          node.setSharedPluginData(NAMESPACE, key, legacy);
          node.setPluginData(key, "");
        });
      return legacy;
    },
    setPluginData: (key, value) => {
      node.setSharedPluginData(NAMESPACE, key, value);
      if (node.getPluginData(key) !== "") tryWrite(() => node.setPluginData(key, ""));
    },
  };
}
