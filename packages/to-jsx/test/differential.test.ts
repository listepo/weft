import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { test } from "vitest";
import { reactCases, reactFixture } from "./differential.ts";

// crates/weft-web/tests/react.rs reads the same file, so the package and the crate answer to one
// set of expected outputs. Regenerate with WEFT_UPDATE_FIXTURES=1.
test("the React fixture matches the package", { timeout: 120_000 }, () => {
  const text = `${JSON.stringify(reactCases(), null, 1)}\n`;
  if (process.env["WEFT_UPDATE_FIXTURES"] === "1") {
    mkdirSync(new URL(".", reactFixture), { recursive: true });
    writeFileSync(reactFixture, text);
  }
  assert.ok(existsSync(reactFixture), "run with WEFT_UPDATE_FIXTURES=1 to create the fixture");
  assert.equal(
    readFileSync(reactFixture, "utf8"),
    text,
    "the generator changed; regenerate with WEFT_UPDATE_FIXTURES=1 and make the Rust crate agree",
  );
});
