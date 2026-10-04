// Stands in for @weft/core's WebAssembly loader in the sandbox bundle. A design tool's plugin
// sandbox (Figma's main thread, Penpot's SES compartment) has no WebAssembly, and the loader's
// top-level await cannot be bundled into the single classic script the sandbox runs. The sandbox
// uses only the parts of the packages that do not reach the core (`handleRequest` of
// @weft/design-tool); anything else fails here with a reason instead of a missing global.
const unavailable = (): never => {
  throw new Error(
    "the Weft core is WebAssembly and runs in the plugin UI, not in the plugin sandbox",
  );
};

export const wasm: unknown = new Proxy({}, { get: unavailable });
export const toJson = unavailable;
export const wellFormed = unavailable;
export const options = unavailable;
export const catalogHandle = unavailable;
