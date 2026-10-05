// The web build of the Rust core (crates/weft-wasm with its `web` feature), built by
// `moon run root:wasm` into ../wasm-web: the core plus the HTML and JSX importers and generators of
// crates/weft-web, behind `@weft/from-aria` and `@weft/to-jsx`. It exports the same names as
// `./wasm.ts`, whose module is a subset of this one, so a bundle can redirect that loader here and
// ship one module. Imported as `@weft/core/web`; not part of the public API.
import init, * as module from "../wasm-web/weft.js";
import { catalogHandles } from "./boundary.ts";
import { type Engine, loadNative } from "./native.ts";

const native = loadNative();
/** Which engine answered at load time; the addon has every export of the module. */
export const engine: Engine = native === undefined ? "wasm" : "native";
export const wasm = (native ?? module) as typeof module;
export { options, toJson, wellFormed, type CoreOptions } from "./boundary.ts";

// Where a file system exists (Node, Deno, Bun) the bytes are read synchronously; browsers fetch
// them and compile while streaming.
const fs = globalThis.process?.getBuiltinModule?.("node:fs");
if (native !== undefined) {
  // The addon needs no module bytes.
} else if (fs === undefined) await init();
else
  module.initSync({
    module: fs.readFileSync(new URL("../wasm-web/weft_bg.wasm", import.meta.url)),
  });

export const catalogHandle = catalogHandles((json) => new wasm.Catalog(json));
