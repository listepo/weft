import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { test } from "vitest";
import { differential, fixturePath } from "./differential.ts";

// The Rust crate reads the same file in crates/weft-catalog/tests/differential.rs, so both
// implementations answer to one set of expected results. Regenerate with WEFT_UPDATE_FIXTURES=1.
test("the catalog differential fixture matches the TypeScript code", () => {
  const text = `${JSON.stringify(differential(), null, 1)}\n`;
  if (process.env["WEFT_UPDATE_FIXTURES"] === "1") {
    mkdirSync(new URL(".", fixturePath), { recursive: true });
    writeFileSync(fixturePath, text);
  }
  assert.ok(existsSync(fixturePath), "run with WEFT_UPDATE_FIXTURES=1 to create the fixture");
  assert.equal(
    readFileSync(fixturePath, "utf8"),
    text,
    "the TypeScript code changed; regenerate with WEFT_UPDATE_FIXTURES=1 and make the Rust crate agree",
  );
});
