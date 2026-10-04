import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { loadTokens, tokenTypes } from "../src/tokens.ts";

const px = (value: number) => ({ value, unit: "px" });

test("group $type is inherited, nested groups can override it", () => {
  const { tokens, problems } = loadTokens({
    space: {
      $type: "dimension",
      md: { $value: px(16) },
      deep: { n: { $type: "number", $value: 3 } },
    },
  });
  assert.deepEqual(problems, []);
  assert.deepEqual(tokens.get("space.md"), { type: "dimension", value: px(16) });
  assert.equal(tokens.get("space.deep.n")?.type, "number");
});

test("aliases resolve through chains and take the target type when untyped", () => {
  const { tokens, problems } = loadTokens({
    a: { $type: "dimension", $value: px(1) },
    b: { $value: "{a}" },
    c: { $value: "{b}" },
    d: { $type: "number", $value: "{a}" },
  });
  assert.deepEqual(problems, []);
  assert.deepEqual(tokens.get("c"), { type: "dimension", value: px(1) });
  assert.equal(tokens.get("d")?.type, "number");
});

test("$root tokens are addressed with .$root", () => {
  const { tokens, problems } = loadTokens({
    blue: { $type: "number", $root: { $value: 1 }, light: { $value: 2 } },
    ref: { $value: "{blue.$root}" },
  });
  assert.deepEqual(problems, []);
  assert.equal(tokens.get("ref")?.value, 1);
  assert.equal(tokens.get("blue.light")?.value, 2);
});

test("alias cycles are reported and the tokens dropped", () => {
  const { tokens, problems } = loadTokens({
    $type: "number",
    a: { $value: "{b}" },
    b: { $value: "{a}" },
    s: { $value: "{s}" },
    ok: { $value: 1 },
  });
  assert.deepEqual([...tokens.keys()], ["ok"]);
  assert.equal(problems.filter((p) => p.code === "T005").length, 2);
  assert.ok(problems.some((p) => p.code === "T005" && p.message.includes("a -> b -> a")));
});

test("dangling aliases are reported with the path of the alias", () => {
  const { tokens, problems } = loadTokens({
    $type: "number",
    a: { $value: "{missing.token}" },
    b: { $value: "{a}" },
  });
  assert.equal(tokens.size, 0);
  assert.equal(problems.find((p) => p.path === "a")?.code, "T004");
  assert.equal(problems.find((p) => p.path === "b")?.code, "T004");
});

test("bad input is reported, never thrown", () => {
  for (const bad of [null, 1, "x", [], undefined]) {
    assert.equal(loadTokens(bad).problems[0]?.code, "T001");
  }
  const { tokens, problems } = loadTokens({
    untyped: { $value: 1 },
    "a.b": { $type: "number", $value: 1 },
    junk: 5,
    $type: 7,
  });
  assert.equal(tokens.size, 0);
  assert.deepEqual(problems.map((p) => p.code).sort(), ["T002", "T003", "T006", "T006"]);
});

test("tokenTypes maps path to $type", () => {
  const { tokens } = loadTokens({ s: { $type: "dimension", a: { $value: px(1) } } });
  assert.deepEqual([...tokenTypes(tokens)], [["s.a", "dimension"]]);
});

test("default tokens load without problems", () => {
  const json: unknown = JSON.parse(
    readFileSync(new URL("../tokens/default.tokens.json", import.meta.url), "utf8"),
  );
  const { tokens, problems } = loadTokens(json);
  assert.deepEqual(problems, []);
  const types = tokenTypes(tokens);
  assert.equal(types.get("space.md"), "dimension");
  assert.equal(types.get("color.action.primary"), "color");
  assert.deepEqual(tokens.get("color.action.primary"), tokens.get("color.blue.$root"));
});
