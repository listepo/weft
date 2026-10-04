import assert from "node:assert/strict";
import { test } from "vitest";
import { coreCatalog, diffCatalogs } from "../src/index.ts";
import { base, edit, rows } from "./diff-cases.ts";

for (const [name, next, level, paths] of rows) {
  test(`diffCatalogs: ${name} is ${level}`, () => {
    const result = diffCatalogs(base, next);
    assert.equal(result.level, level);
    assert.deepEqual(result.changes.map((c) => c.path).toSorted(), paths.toSorted());
    for (const change of result.changes) assert.match(change.message, /\S+\.$/);
  });
}

test("diffCatalogs is major one way and minor the other for added versus removed values", () => {
  const next = edit((c) => c["button"]!.props!["variant"]!.values!.push("quiet"));
  assert.equal(diffCatalogs(base, next).level, "minor");
  assert.equal(diffCatalogs(next, base).level, "major");
});

test("diffCatalogs of the core catalog with itself is none", () => {
  assert.deepEqual(diffCatalogs(coreCatalog, structuredClone(coreCatalog)), {
    level: "none",
    changes: [],
  });
});
