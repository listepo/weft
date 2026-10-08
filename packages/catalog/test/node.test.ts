import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { checkData, parse } from "@weft/core";
import { describe, test } from "vitest";
import { isResolver, tokenTypes, withAppearance } from "../src/index.ts";
import {
  chooseProject,
  findProject,
  projectPath,
  readProject,
  readResolver,
  readTokenLayers,
} from "../src/node.ts";

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
    assert.deepEqual(project.settings, {
      validate: { mode: "strict" },
      render: { data: "sample.data.json" },
    });
  });

  test("treats a member file over maxChars as unreadable", () => {
    const { diagnostics } = readProject(join(example, "weft.json"), { maxChars: 600 });
    assert.ok(diagnostics.some((d) => d.code === "W704" && d.path === "#/catalog/1"));
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

describe("project settings helpers", () => {
  test("chooseProject: an explicit file wins, --no-project means none", () => {
    const screen = join(example, "screens/cart.weft");
    assert.equal(chooseProject(screen, {}), join(example, "weft.json"));
    assert.equal(chooseProject(screen, { project: "other.json" }), "other.json");
    assert.equal(chooseProject(screen, { project: "other.json", noProject: true }), undefined);
  });

  test("projectPath resolves a name against the project directory", () => {
    assert.equal(
      projectPath(join(example, "weft.json"), "sample.data.json"),
      join(example, "sample.data.json"),
    );
  });

  test("readTokenLayers layers files and points diagnostics at the setting", () => {
    const file = join(example, "weft.json");
    const layered = readTokenLayers(
      file,
      ["tokens/base.tokens.json", "tokens/brand.tokens.json"],
      "#/render",
    );
    assert.deepEqual(layered.diagnostics, []);
    assert.deepEqual(layered.tokens?.get("color.star"), { type: "color", value: "#d97706" });
    const missing = readTokenLayers(file, ["tokens/none.json"], "#/render");
    assert.deepEqual(
      missing.diagnostics.map((d) => [d.code, d.path]),
      [["W704", "#/render/tokens/0"]],
    );
  });

  test("readResolver loads a resolver named on its own, with pointers into it", () => {
    const resolver = join(example, "tokens", "theme.resolver.json");
    assert.ok(isResolver(JSON.parse(readFileSync(resolver, "utf8"))));
    assert.ok(!isResolver({ resolutionOrder: { a: 1 } }));
    const layers = readResolver(resolver);
    assert.deepEqual(layers.diagnostics, []);
    assert.equal(layers.appearance?.dark.get("color.star")?.value, "#fbbf24");

    const dir = mkdtempSync(join(tmpdir(), "weft-resolver-"));
    try {
      const broken = join(dir, "broken.resolver.json");
      writeFileSync(
        broken,
        JSON.stringify({ version: "2025.10", resolutionOrder: [{ $ref: "#/sets/none" }] }),
      );
      assert.deepEqual(
        readResolver(broken).diagnostics.map((d) => [d.code, d.path]),
        [["W705", "#/resolutionOrder/0/$ref"]],
      );
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("withAppearance picks a side, else the default context, and no scheme without one", () => {
    const project = readProject(join(example, "weft.json")).project;
    const dark = withAppearance(project, "dark");
    assert.equal(dark.scheme, "dark");
    assert.equal(dark.tokens?.get("color.star")?.value, "#fbbf24");
    const byDefault = withAppearance(project);
    assert.equal(byDefault.scheme, "light");
    assert.equal(byDefault.tokens?.get("color.star")?.value, "#d97706");
    const plain = readTokenLayers(join(example, "weft.json"), ["tokens/base.tokens.json"], "#");
    assert.deepEqual(withAppearance(plain, "dark"), { tokens: plain.tokens, scheme: undefined });
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
