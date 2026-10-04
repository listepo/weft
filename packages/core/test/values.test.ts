import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { formatValue, readValue } from "../src/index.ts";

describe("single values", () => {
  test("read and written as attributes are", () => {
    assert.deepEqual(readValue("{$.user.name}"), { ok: true, value: { bind: "$.user.name" } });
    assert.deepEqual(readValue("{{brace"), { ok: true, value: "{brace" });
    assert.equal(readValue("{oops}").ok, false);
    for (const value of [
      "plain",
      "{brace",
      { bind: "$.a", not: true as const },
      { token: "space.md" },
    ]) {
      const read = readValue(formatValue(value));
      assert.deepEqual(read, { ok: true, value });
    }
  });
});
