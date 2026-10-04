// What a host leaves in its cache for a plugin: the plugin folder alone, without the workspace's
// `node_modules`. Claude Code copies the folder; Cursor extracts the marketplace entry's `source`
// folder from an archive. Neither brings anything from outside it.
import { cpSync } from "node:fs";
import { pluginDir, type PluginName } from "../build.ts";

/** Copies the plugin's folder to `target` and returns `target`. */
export function installCopy(plugin: PluginName, target: string): string {
  cpSync(pluginDir(plugin), target, {
    recursive: true,
    filter: (source) => !source.split("/").includes("node_modules"),
  });
  return target;
}
