import assert from "node:assert/strict";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { test } from "vitest";
import { buildCases, buildFixture, domCases, domFixture } from "./differential.ts";

// The Rust crates read the same files (crates/weft-import/tests/build.rs and
// crates/weft-web/tests/from_dom.rs), so both implementations answer to one set of expected
// results. Regenerate with WEFT_UPDATE_FIXTURES=1.
function check(url: URL, cases: unknown): void {
  const text = `${JSON.stringify(cases, null, 1)}\n`;
  if (process.env["WEFT_UPDATE_FIXTURES"] === "1") {
    mkdirSync(new URL(".", url), { recursive: true });
    writeFileSync(url, text);
  }
  assert.ok(existsSync(url), "run with WEFT_UPDATE_FIXTURES=1 to create the fixture");
  assert.equal(
    readFileSync(url, "utf8"),
    text,
    "the importer changed; regenerate with WEFT_UPDATE_FIXTURES=1 and make the Rust crates agree",
  );
}

test("the role tree fixture matches the package", { timeout: 120_000 }, () => {
  check(buildFixture, buildCases());
});

test("the HTML import fixture matches the package", { timeout: 120_000 }, () => {
  check(domFixture, domCases());
});
