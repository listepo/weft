// Stands in for the wasm-bindgen glue of @weft/core in the UI bundle. The glue fetches
// `weft_bg.wasm` next to itself, but Figma loads the UI as one HTML string with no files beside it
// and no network, so the module's bytes are inlined (base64) and handed to the real glue.
import realInit, { initSync as realInitSync } from "../../../packages/core/wasm/weft.js";

export * from "../../../packages/core/wasm/weft.js";

/** Replaced by the build with the module's bytes in base64. */
declare const WEFT_WASM_BASE64: string;

// Decoded once, at load: the core instantiates the module while the UI loads anyway, and a single
// use keeps the minifier from copying the 1 MB string into each caller.
const BYTES = Uint8Array.from(atob(WEFT_WASM_BASE64), (c) => c.charCodeAt(0));

// Browsers compile asynchronously: Chromium refuses to compile a module over 4 KB synchronously
// on the page's main thread, so the browser path must take this one.
export default function init() {
  return realInit({ module_or_path: BYTES });
}

// The synchronous path runs only where a file system exists (Node, in the tests).
export function initSync() {
  return realInitSync({ module: BYTES });
}
