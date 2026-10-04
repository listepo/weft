// Helpers the generated component carries with it, so that it needs nothing from Weft at run time
// (crates/weft-web/src/jsx/runtime.rs). Only the helpers a document uses are emitted, in this
// order.
import { tables } from "./generate.ts";

export const RUNTIME: Readonly<Record<string, string>> = Object.freeze({ ...tables.runtime });
