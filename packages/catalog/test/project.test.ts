import assert from "node:assert/strict";
import { describe, test } from "vitest";
import { loadProjectText } from "../src/index.ts";
import { projectCases, runCase } from "./project-cases.ts";

for (const [name, c] of Object.entries(projectCases)) {
  test(`${name}: ${c.codes.join(", ") || "nothing"}`, () => {
    assert.deepEqual(
      runCase(c).diagnostics.map((d) => d.code),
      c.codes,
    );
  });
}

const run = (name: string) => {
  const c = projectCases[name];
  assert.ok(c);
  return runCase(c);
};

describe("token layers", () => {
  test("a later file overrides a token, and aliases read the override", () => {
    const { project } = run("token layers, later wins");
    assert.deepEqual(project.tokens?.get("color.brand"), { type: "color", value: "#ff0000" });
    assert.deepEqual(project.tokens?.get("color.text"), { type: "color", value: "#ff0000" });
    // The brand file sets no $type; the group type from the base file still applies.
    assert.deepEqual(project.tokens?.get("color.accent"), { type: "color", value: "#00ff00" });
  });

  test("the earlier file wins when it comes last", () => {
    const { project } = run("token layers, reversed");
    assert.deepEqual(project.tokens?.get("color.text"), { type: "color", value: "#000000" });
  });

  test("a __proto__ group stays an ordinary group", () => {
    const { project } = run("a __proto__ token group");
    assert.deepEqual(project.tokens?.get("__proto__.x"), { type: "color", value: "#fff" });
    assert.equal(Object.getPrototypeOf({}), Object.prototype);
  });
});

describe("catalog extension", () => {
  test("adds kinds, widens core kinds and takes the extension's name", () => {
    const { project } = run("a catalog extension");
    const { catalog } = project;
    assert.equal(catalog.name, "acme");
    assert.equal(catalog.version, "1.2.0");
    assert.equal(catalog.components["rating"]?.role, "img");
    const button = catalog.components["button"];
    assert.deepEqual(button?.props?.["variant"]?.values, [
      "primary",
      "secondary",
      "danger",
      "ghost",
    ]);
    assert.ok(button?.props?.["disabled"], "props the entry does not name are kept");
    assert.deepEqual(button?.events, ["press", "longpress"]);
    assert.deepEqual(button?.states, ["idle", "busy"]);
    assert.equal(catalog.components["text"]?.content, "mixed");
  });

  test("a narrowing change names what breaks and keeps the core definition", () => {
    const { project, diagnostics } = run("narrowing changes keep the core definition");
    assert.equal(project.catalog.components["button"]?.role, "button");
    assert.deepEqual(diagnostics[0], {
      code: "W707",
      severity: "error",
      message:
        'The extension of "button" would break existing screens, so it keeps its core definition: role changed from "button" to "link".',
      path: "#/catalog/components/button",
      expected: "a change that only widens the definition it extends (SPEC §8)",
      got: "components.button.role",
    });
  });
});

describe("several catalogs", () => {
  test("record which catalog defined and extended each kind, whatever the list order", () => {
    for (const name of [
      "a library and the project catalog",
      "the project catalog merges last wherever it is listed",
    ]) {
      const { project } = run(name);
      assert.equal(project.catalog.name, "shop", name);
      const variant = project.catalog.components["acme-button"]?.props?.["variant"];
      assert.deepEqual(variant?.values, ["default", "danger", "ghost"]);
      assert.deepEqual(project.kinds["acme-button"], { catalog: "acme-ui", extendedBy: ["shop"] });
      assert.deepEqual(project.kinds["rating"], { catalog: "shop" });
      assert.deepEqual(project.kinds["button"], { catalog: "weft-core", extendedBy: ["shop"] });
      assert.deepEqual(project.kinds["text"], { catalog: "weft-core" });
    }
    assert.deepEqual(run("a library and the project catalog").project.catalogs, [
      { name: "acme-ui", version: "1.0.0", prefix: "acme", source: "lib/acme-ui.json" },
      { name: "shop", version: "1.1.0", source: "shop.json" },
    ]);
  });

  test("a prefix claimed twice names both catalogs", () => {
    const [d] = run("two libraries with one prefix").diagnostics;
    assert.equal(d?.path, "#/catalog/1/prefix");
    assert.match(d?.message ?? "", /"acme-ui".*"acme-two"/);
  });

  test("a narrowed library kind keeps the library's definition", () => {
    const { project, diagnostics } = run("the project catalog may not narrow a library kind");
    assert.match(diagnostics[0]?.message ?? "", /keeps its definition from "acme-ui"/);
    assert.deepEqual(project.kinds["acme-button"], { catalog: "acme-ui" });
  });

  test("an unmet requirement points into the catalog that states it", () => {
    const [missing, old] = run("unmet requirements warn and the catalog still loads").diagnostics;
    assert.equal(missing?.path, "#/catalog/0/requires/acme-ui");
    assert.equal(missing?.severity, "warning");
    assert.equal(old?.message, '"old-ui" requires "weft-core" 1.0.0, but 0.2.0 is loaded.');
  });
});

describe("project diagnostics", () => {
  test("point below the tool argument for content", () => {
    const { diagnostics } = run("content: problems point below the argument");
    assert.deepEqual(
      diagnostics.map((d) => d.path),
      [
        "#/project/extra",
        "#/project/tokens/0",
        "#/project/tokens",
        "#/project/catalog",
        "#/project/data/properties/a/not",
      ],
    );
  });

  test("an unknown member is a warning in both modes, so an older tool reads a newer file", () => {
    const lenient = loadProjectText('{"tokenz": []}').diagnostics[0];
    assert.equal(lenient?.severity, "warning");
    assert.equal(lenient?.hint, 'did you mean "tokens"?');
    const strict = loadProjectText('{"tokenz": []}', { mode: "strict" }).diagnostics[0];
    assert.equal(strict?.severity, "warning");
  });

  test("a file name that leaves the project is never read", () => {
    const asked: string[] = [];
    loadProjectText(JSON.stringify({ tokens: ["../secret.json"], data: "/etc/passwd" }), {
      read: (name) => {
        asked.push(name);
        return undefined;
      },
    });
    assert.deepEqual(asked, []);
  });

  test("never throws on hostile input", () => {
    for (const text of ["", "null", "1", '"x"', "[", '{"tokens": [null]}', '{"catalog": "x"}']) {
      assert.doesNotThrow(() => loadProjectText(text, { read: () => "{]" }));
      assert.doesNotThrow(() => loadProjectText(text));
    }
  });
});

describe("tool sections", () => {
  // Sections are checked in the order SPEC §10.6 lists them, whatever the order in the file.
  test("valid settings are kept as written; invalid ones are left out", () => {
    const { project, diagnostics } = loadProjectText(
      JSON.stringify({
        validate: { mode: "strict" },
        render: { data: "sample.json", tokens: ["a.json", "../b.json"], outDir: 5 },
        mcp: { limits: { patches: 3, diagnostics: 0 } },
        export: { react: { outDir: "src" }, cobol: { outDir: "ios" } },
      }),
    );
    assert.deepEqual(project.settings, {
      validate: { mode: "strict" },
      render: { data: "sample.json", tokens: ["a.json"] },
      mcp: { limits: { patches: 3 } },
      export: { react: { outDir: "src" } },
    });
    assert.deepEqual(
      diagnostics.map((d) => [d.code, d.path, d.severity]),
      [
        ["W703", "#/render/tokens/1", "error"],
        ["W701", "#/render/outDir", "error"],
        ["W702", "#/export/cobol", "warning"],
        ["W701", "#/mcp/limits/diagnostics", "error"],
      ],
    );
  });

  test("a project without sections has empty settings", () => {
    assert.deepEqual(loadProjectText("{}").project.settings, {});
    assert.deepEqual(loadProjectText("[").project.settings, {});
  });
});
