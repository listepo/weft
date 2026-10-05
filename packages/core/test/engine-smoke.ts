// Run by engines.test.ts under Node and Bun: loads the core the way a consumer does and prints
// the engine that answered, with one parse and one validation made through it.
import { parse, validate } from "../src/index.ts";
import { engine } from "../src/wasm.ts";
import { catalog } from "./catalog.ts";

const parsed = parse('<screen id="s"/>', { catalog });
const invalid = validate({ weft: "0.1", root: { kind: "nope" } }, { catalog });
console.log(
  JSON.stringify({ engine, root: parsed.document?.root.kind, invalid: invalid.length > 0 }),
);
