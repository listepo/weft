import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { checkData, parse } from "@weft/core";
import { describe, test } from "vitest";
import { tokenTypes } from "../src/index.ts";
import { findProject, readProject } from "../src/node.ts";

const example = fileURLToPath(new URL("../../../examples/project/", import.meta.url));

describe("findProject", () => {
  test("finds the project file above a screen", () => {
    assert.equal(findProject(join(example, "screens/cart.weft")), join(example, "weft.json"));
  });

  test("takes the nearest project file", () => {
    const dir = mkdtempSync(join(tmpdir(), "weft-find-"));
    try {
      mkdirSync(join(dir, "a/b"), { recursive: true });
      writeFileSync(join(dir, "weft.json"), "{}");
      writeFileSync(join(dir, "a/weft.json"), "{}");
      assert.equal(findProject(join(dir, "a/b/screen.weft")), join(dir, "a/weft.json"));
      // A directory named weft.json is not a project file.
      mkdirSync(join(dir, "a/b/weft.json"));
      assert.equal(findProject(join(dir, "a/b/screen.weft")), join(dir, "a/weft.json"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("readProject", () => {
  test("loads the example project without problems", () => {
    const { project, diagnostics } = readProject(join(example, "weft.json"));
    assert.deepEqual(diagnostics, []);
    assert.equal(project.catalog.name, "shop");
    assert.ok(project.catalog.components["rating"]);
    // The base file aliases the brand colour, which the brand file overrides.
    assert.deepEqual(project.tokens?.get("color.star"), { type: "color", value: "#d97706" });
    assert.deepEqual(project.actions, ["cart.checkout", "cart.remove", "nav.back"]);
    assert.ok(project.data);
  });

  test("reports a missing member file instead of throwing", () => {
    const dir = mkdtempSync(join(tmpdir(), "weft-read-"));
    try {
      writeFileSync(join(dir, "weft.json"), '{"catalog": "missing.json", "data": "sub"}');
      mkdirSync(join(dir, "sub"));
      const { diagnostics } = readProject(join(dir, "weft.json"));
      assert.deepEqual(
        diagnostics.map((d) => [d.code, d.path]),
        [
          ["W704", "#/catalog"],
          ["W704", "#/data"],
        ],
      );
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

test("every example screen is valid in strict mode with its project", () => {
  const { project } = readProject(join(example, "weft.json"));
  const screens = readdirSync(join(example, "screens"));
  assert.ok(screens.length >= 2);
  for (const name of screens) {
    const { catalog, tokens, actions, data } = project;
    const markup = readFileSync(join(example, "screens", name), "utf8");
    const result = parse(markup, {
      catalog,
      mode: "strict",
      tokens: tokens && tokenTypes(tokens),
      actions,
    });
    assert.deepEqual(result.diagnostics, [], name);
    assert.ok(result.document && data);
    assert.deepEqual(checkData(result.document, { catalog, data }), [], name);
  }
});
