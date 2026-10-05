// The WebAssembly build of the Rust core and catalog (crates/weft-wasm), built by
// `moon run root:wasm` into ../wasm. Every package function stays synchronous: the module is
// instantiated while this file is imported. `@weft/catalog` imports this file as `@weft/core/wasm`;
// it is not part of the public API. `./web.ts` loads the larger module with the web importers and
// generators and exports the same names, so a bundle that needs both can ship that module alone.
import init, * as module from "../wasm/weft.js";
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
else module.initSync({ module: fs.readFileSync(new URL("../wasm/weft_bg.wasm", import.meta.url)) });

export const catalogHandle = catalogHandles((json) => new wasm.Catalog(json));
