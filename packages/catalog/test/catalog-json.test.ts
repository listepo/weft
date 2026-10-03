import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { coreCatalog } from "../src/core.ts";
import { catalogJsonPath } from "../src/generate.ts";

// Compared as data because oxfmt, not the generator, decides the file layout.
test("catalog.json is up to date; run `node src/generate.ts` in packages/catalog", () => {
  assert.deepEqual(JSON.parse(readFileSync(catalogJsonPath, "utf8")), coreCatalog);
});
