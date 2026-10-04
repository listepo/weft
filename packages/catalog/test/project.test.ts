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
      expected: "a change that only widens the core definition (SPEC §8)",
      got: "components.button.role",
    });
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

  test("an unknown member is a warning in lenient mode and an error in strict mode", () => {
    const lenient = loadProjectText('{"tokenz": []}').diagnostics[0];
    assert.equal(lenient?.severity, "warning");
    assert.equal(lenient?.hint, 'did you mean "tokens"?');
    const strict = loadProjectText('{"tokenz": []}', { mode: "strict" }).diagnostics[0];
    assert.equal(strict?.severity, "error");
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
