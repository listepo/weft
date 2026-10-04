// Stands in for @weft/core's WebAssembly loader in the main-thread bundle. Figma's main thread has
// no WebAssembly, and the loader's top-level await cannot be bundled into the single classic script
// Figma runs there. The main thread uses only the parts of the packages that do not reach the core
// (`@weft/figma`'s plugin.ts); anything else fails here with a reason instead of a missing global.
const unavailable = (): never => {
  throw new Error("the Weft core is WebAssembly and runs in the plugin UI, not in the main thread");
};

export const wasm: unknown = new Proxy({}, { get: unavailable });
export const toJson = unavailable;
export const wellFormed = unavailable;
export const options = unavailable;
export const catalogHandle = unavailable;
