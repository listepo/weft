// A build plugin both plugin builds use to swap the core's WebAssembly loader for another file.
import type { Plugin } from "vite";

/** Resolves `from` to `to` for every importer except `to` itself, which imports the original. */
export function redirect(from: string, to: string): Plugin {
  return {
    name: `weft-redirect-${from}`,
    enforce: "pre",
    async resolveId(source, importer, options) {
      if (importer === to) return null;
      const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
      return resolved?.id === from ? to : null;
    },
  };
}
