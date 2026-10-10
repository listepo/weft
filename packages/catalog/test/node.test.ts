import assert from "node:assert/strict";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
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

  test("does not follow a symlink out of the project directory", () => {
    const dir = mkdtempSync(join(tmpdir(), "weft-symlink-"));
    const secret = join(tmpdir(), `weft-secret-${process.pid}.json`);
    try {
      writeFileSync(secret, JSON.stringify({ leaked: { $type: "color", $value: "#ff0000" } }));
      writeFileSync(join(dir, "weft.json"), '{"tokens": "outside.json"}');
      symlinkSync(secret, join(dir, "outside.json"));
      const { project, diagnostics } = readProject(join(dir, "weft.json"));
      assert.ok(diagnostics.some((d) => d.code === "W704" && d.path === "#/tokens"));
      assert.equal(project.tokens?.get("leaked"), undefined);
    } finally {
      rmSync(dir, { recursive: true, force: true });
      rmSync(secret, { force: true });
    }
  });

  test("does not follow a catalog list entry that links out of the project directory", () => {
    const dir = mkdtempSync(join(tmpdir(), "weft-symlink-catalog-"));
    const outside = join(tmpdir(), `weft-outside-catalog-${process.pid}.json`);
    try {
      writeFileSync(outside, "{}");
      writeFileSync(join(dir, "weft.json"), '{"catalog": ["outside.json"]}');
      symlinkSync(outside, join(dir, "outside.json"));
      const { diagnostics } = readProject(join(dir, "weft.json"));
      assert.ok(diagnostics.some((d) => d.code === "W704" && d.path === "#/catalog/0"));
    } finally {
      rmSync(dir, { recursive: true, force: true });
      rmSync(outside, { force: true });
    }
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

describe("readProject with a catalog package", () => {
  // `@acme/ui` in `node_modules` above the project directory, as a workspace installs it.
  const installed = (body: (root: string, pkg: string) => void) => {
    const root = mkdtempSync(join(tmpdir(), "weft-package-"));
    const pkg = join(root, "node_modules/@acme/ui");
    try {
      mkdirSync(join(pkg, "dist"), { recursive: true });
      writeFileSync(join(pkg, "package.json"), '{"weft": {"catalog": "dist/weft.json"}}');
      const acme = readFileSync(join(example, "catalogs/acme-ui.catalog.json"), "utf8");
      writeFileSync(join(pkg, "dist/weft.json"), acme);
      // The library's fragments travel with it, relative to its catalog file.
      cpSync(join(example, "catalogs/fragments"), join(pkg, "dist/fragments"), { recursive: true });
      mkdirSync(join(root, "app/deep"), { recursive: true });
      writeFileSync(join(root, "app/deep/weft.json"), '{"catalog": [{"package": "@acme/ui"}]}');
      body(root, pkg);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  };

  test("finds it in node_modules of a parent and reads the catalog it names", () => {
    installed((root) => {
      const { project, diagnostics } = readProject(join(root, "app/deep/weft.json"));
      assert.deepEqual(diagnostics, []);
      assert.deepEqual(project.catalogs, [
        { name: "acme-ui", version: "1.0.0", prefix: "acme", source: "@acme/ui" },
      ]);
      assert.equal(project.kinds["acme-button"]?.catalog, "acme-ui");
    });
  });

  test("reports a package that is not installed", () => {
    installed((root) => {
      rmSync(join(root, "node_modules"), { recursive: true });
      const { diagnostics } = readProject(join(root, "app/deep/weft.json"));
      assert.deepEqual(
        diagnostics.map((d) => [d.code, d.path]),
        [["W704", "#/catalog/0/package"]],
      );
    });
  });

  test("does not follow a catalog file that links out of the package", () => {
    installed((root, pkg) => {
      writeFileSync(join(root, "outside.json"), readFileSync(join(pkg, "dist/weft.json")));
      rmSync(join(pkg, "dist/weft.json"));
      symlinkSync(join(root, "outside.json"), join(pkg, "dist/weft.json"));
      const { diagnostics } = readProject(join(root, "app/deep/weft.json"));
      assert.deepEqual(
        diagnostics.map((d) => [d.code, d.path]),
        [["W704", "#/catalog/0/package"]],
      );
    });
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
