import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";
import { LISTED } from "../src/tools/capabilities.ts";
import { call, connect } from "./connect.ts";

const dir = new URL("../../../examples/project/", import.meta.url);
const file = (name: string): unknown => JSON.parse(readFileSync(new URL(name, dir), "utf8"));
const PROJECT = {
  tokens: [file("tokens/base.tokens.json"), file("tokens/brand.tokens.json")],
  catalog: file("catalog.json"),
  actions: (file("weft.json") as { actions: string[] }).actions,
  data: file("data.schema.json"),
};

const capabilities = async (
  overrides: Parameters<typeof connect>[0] = {},
  args: Record<string, unknown> = {},
) => {
  const { client, close } = await connect(overrides);
  const result = await call(client, "weft_capabilities", args);
  await close();
  return { ...result, json: JSON.parse(result.blocks[0] ?? "null") };
};

test("a host with no configuration advertises the core catalog and nothing it does not check", async () => {
  const { isError, json } = await capabilities();
  assert.equal(isError, false);
  assert.deepEqual(json, {
    weft: "0.1",
    catalogs: [{ name: "weft-core", version: json.catalogs[0].version }],
  });
});

test("tokens and actions are listed when the host checks them", async () => {
  const { json } = await capabilities({
    tokens: new Map([
      ["space.md", "dimension"],
      ["color.brand", "color"],
    ]),
    actions: ["cart.checkout", "auth.submit"],
  });
  assert.deepEqual(json.tokens, { count: 2, names: ["color.brand", "space.md"] });
  assert.deepEqual(json.actions, { count: 2, names: ["auth.submit", "cart.checkout"] });
  assert.equal("data" in json, false);
});

test("a long list is cut and says so", async () => {
  const actions = Array.from({ length: LISTED + 5 }, (_, i) => `a.n${i}`);
  const { json } = await capabilities({ actions });
  assert.equal(json.actions.count, LISTED + 5);
  assert.equal(json.actions.names.length, LISTED);
  assert.equal(json.actions.truncated, true);
});

test("a project argument is advertised in place of the host's configuration", async () => {
  const { isError, json } = await capabilities({ actions: ["host.only"] }, { project: PROJECT });
  assert.equal(isError, false);
  assert.deepEqual(
    json.catalogs.map((c: { name: string }) => c.name),
    ["weft-core", "shop"],
  );
  assert.equal(json.data, true);
  assert.deepEqual(json.actions.names, [...PROJECT.actions].toSorted());
  assert.ok(json.tokens.names.length > 0);
});

test("a broken project is reported, not advertised", async () => {
  const { isError, blocks } = await capabilities({}, { project: { ...PROJECT, actions: "x" } });
  assert.equal(isError, true);
  assert.match(blocks[0] ?? "", /W701/);
});
