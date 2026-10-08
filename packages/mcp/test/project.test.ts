import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { loadProject } from "@weft/catalog";
import { documentSchema } from "@weft/core/document-schema";
import { test } from "vitest";
import { LIMITS } from "../src/index.ts";
import { call, connect } from "./connect.ts";

// The example project, as a host would pass it: weft.json with every file name replaced by the
// file's JSON content (SPEC §10.1).
const dir = new URL("../../../examples/project/", import.meta.url);
const file = (name: string): unknown => JSON.parse(readFileSync(new URL(name, dir), "utf8"));
const PROJECT = {
  tokens: [file("tokens/base.tokens.json"), file("tokens/brand.tokens.json")],
  catalog: [file("catalogs/acme-ui.catalog.json"), file("catalog.json")],
  actions: (file("weft.json") as { actions: string[] }).actions,
  data: file("data.schema.json"),
};
const REVIEW = readFileSync(new URL("screens/review.weft", dir), "utf8");

type Diagnostics = { valid?: boolean; diagnostics: { code: string; path: string }[] };
const json = (text: string | undefined) => JSON.parse(text ?? "null") as Diagnostics;
const codes = (text: string | undefined) => json(text).diagnostics.map((d) => d.code);

test("weft_validate checks a screen against the project it is given", async () => {
  const { client, close } = await connect();
  const valid = await call(client, "weft_validate", {
    markup: REVIEW,
    strict: true,
    project: PROJECT,
  });
  assert.deepEqual(json(valid.blocks[0]), { valid: true, diagnostics: [] });

  // Without the project the screen uses a kind, tokens and actions nobody declared.
  const bare = await call(client, "weft_validate", { markup: REVIEW, strict: true });
  assert.ok(codes(bare.blocks[0]).includes("W401"));

  const misspelled = await call(client, "weft_validate", {
    markup: REVIEW.replace("$.busy", "$.bussy"),
    project: PROJECT,
  });
  assert.equal(json(misspelled.blocks[0]).valid, false);
  assert.deepEqual(codes(misspelled.blocks[0]), ["W315"]);
  await close();
});

test("project problems come first, with paths into the argument", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_validate", {
    markup: REVIEW,
    project: { ...PROJECT, actions: "cart.checkout" },
  });
  assert.equal(result.isError, false);
  const { valid, diagnostics } = json(result.blocks[0]);
  assert.equal(valid, false);
  assert.equal(diagnostics[0]?.code, "W701");
  assert.equal(diagnostics[0]?.path, "#/project/actions");

  // The other tools cannot work without a sound project, so they fail with its diagnostics.
  for (const [name, args] of [
    ["weft_format", { markup: REVIEW }],
    ["weft_patch", { markup: REVIEW, patches: [] }],
    ["weft_render", { markup: REVIEW }],
    ["weft_catalog", {}],
  ] as const) {
    const failed = await call(client, name, { ...args, project: { catalog: 5 } });
    assert.equal(failed.isError, true, name);
    assert.equal(json(failed.blocks[0]).diagnostics[0]?.path, "#/project/catalog", name);
  }
  await close();
});

test("weft_catalog lists the project's catalog", async () => {
  const { client, close } = await connect();
  const index = await call(client, "weft_catalog", { project: PROJECT });
  assert.match(index.blocks[0] ?? "", /^catalog shop 1\.1\.0/);
  assert.match(index.blocks[0] ?? "", /\nrating \|/);
  const rating = await call(client, "weft_catalog", { kind: "rating", project: PROJECT });
  assert.equal(rating.isError, false);
  const core = await call(client, "weft_catalog", { kind: "rating" });
  assert.equal(core.isError, true);
  await close();
});

test("weft_schema gives the schema of the project's merged catalog", async () => {
  const { client, close } = await connect();
  const result = await call(client, "weft_schema", { project: PROJECT });
  assert.equal(result.isError, false);
  const { catalog } = loadProject(PROJECT).project;
  assert.equal(result.blocks[0], documentSchema(catalog));
  const core = await call(client, "weft_schema", {});
  assert.notEqual(core.blocks[0], result.blocks[0]);
  assert.ok("rating" in (JSON.parse(result.blocks[0] ?? "") as { $defs: object }).$defs);
  await close();
});

test("weft_patch and weft_render work in the project", async () => {
  const { client, close } = await connect();
  const inserted = await call(client, "weft_patch", {
    markup: REVIEW,
    patches: [{ op: "set", id: "line-score", prop: "color", value: { token: "color.brand" } }],
    project: PROJECT,
  });
  assert.equal(inserted.isError, false, inserted.blocks.join("\n"));
  assert.match(inserted.blocks[0] ?? "", /color="\{token\.color\.brand\}"/);

  // The patched screen is checked against the data schema too.
  const unbound = await call(client, "weft_patch", {
    markup: REVIEW,
    patches: [{ op: "set", id: "checkout", prop: "disabled", value: { bind: "$.nope" } }],
    project: PROJECT,
  });
  assert.equal(unbound.isError, true);
  assert.deepEqual(codes(unbound.blocks[0]), ["W315"]);

  const rendered = await call(client, "weft_render", {
    markup: REVIEW,
    data: { cart: { items: [{ name: "Tea", price: 3, score: 4 }], empty: false }, busy: false },
    project: PROJECT,
  });
  assert.equal(rendered.isError, false, rendered.blocks.join("\n"));
  assert.match(rendered.blocks[0] ?? "", /Tea/);
  await close();
});

test("a project argument is size-limited and hostile content never throws", async () => {
  const { client, close } = await connect();
  const big = await call(client, "weft_validate", {
    markup: REVIEW,
    project: { actions: ["x".repeat(LIMITS.projectChars)] },
  });
  assert.equal(big.isError, true);
  for (const project of [null, 7, "weft.json", [], { tokens: ["../secret.json"] }]) {
    const result = await call(client, "weft_validate", { markup: REVIEW, project });
    assert.equal(result.isError, false, JSON.stringify(project));
    assert.equal(json(result.blocks[0]).valid, false, JSON.stringify(project));
  }
  // An unknown member only warns, and a "__proto__" member is data, not a prototype.
  const proto = await call(client, "weft_validate", {
    markup: REVIEW,
    project: JSON.parse('{"__proto__": {"actions": 1}}'),
  });
  assert.equal(proto.isError, false);
  assert.equal(json(proto.blocks[0]).diagnostics[0]?.path, "#/project/__proto__");
  assert.equal(({} as Record<string, unknown>)["actions"], undefined);
  await close();
});
