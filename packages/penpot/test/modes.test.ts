// Token modes (SPEC §10.3): the example project's light and dark contexts become token themes of
// one group, and the themes read back into a resolver document with the same values.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { TOKEN_COLLECTION } from "@weft/design-tool";
import { describe, test } from "vitest";
import { assertSameModes, exampleModes } from "../../design-tool/test/modes.ts";
import {
  buildRequest,
  ensureLibrary,
  exportRequest,
  finishExport,
  handleRequest,
  MODES_GROUP,
  readThemes,
} from "../src/index.ts";
import { FakePenpot, type FakeShape } from "./fake-penpot.ts";
import { corpusMarkup } from "./helpers.ts";

const { tokens, modifier } = exampleModes();

describe("token themes", () => {
  test("each context is a theme of the modifier's group, the default one on", async () => {
    const penpot = new FakePenpot();
    await ensureLibrary(penpot, coreCatalog, tokens, modifier);
    const catalog = penpot.library.local.tokens;
    assert.deepEqual(
      catalog.themes.map((t) => [t.group, t.name, t.active, t.activeSets.map((s) => s.name)]),
      [
        ["theme", "light", true, [TOKEN_COLLECTION]],
        ["theme", "dark", false, [TOKEN_COLLECTION, `${MODES_GROUP}/theme/dark`]],
      ],
    );
    const dark = catalog.sets.find((s) => s.name === `${MODES_GROUP}/theme/dark`);
    assert.ok(dark !== undefined && !dark.active);
    // Only what differs from the default context is in the context's set.
    assert.ok(dark.tokens.some((t) => t.name === "color.brand"));
    assert.ok(!dark.tokens.some((t) => t.name === "space.md"));
  });

  test("the themes read back into a resolver with the same values per context", async () => {
    const penpot = new FakePenpot();
    await ensureLibrary(penpot, coreCatalog, tokens, modifier);
    // A second build finds the themes and sets it made instead of adding more.
    await ensureLibrary(penpot, coreCatalog, tokens, modifier);
    assert.equal(penpot.library.local.tokens.themes.length, 2);
    assert.equal(penpot.library.local.tokens.sets.length, 2);
    assertSameModes(readThemes(penpot.library.local.tokens, tokens), modifier);
  });

  test("a designer's change in a context's set reads back in that context only", async () => {
    const penpot = new FakePenpot();
    await ensureLibrary(penpot, coreCatalog, tokens, modifier);
    const catalog = penpot.library.local.tokens;
    const dark = catalog.sets.find((s) => s.name === `${MODES_GROUP}/theme/dark`);
    const brand = dark?.tokens.find((t) => t.name === "color.brand");
    assert.ok(brand !== undefined);
    brand.value = "rgba(255, 0, 0, 0.5)";
    const document = readThemes(catalog, tokens) as {
      modifiers: {
        theme: {
          contexts: { light: unknown[]; dark: [{ color: { brand: { $value: unknown } } }] };
        };
      };
    };
    const contexts = document.modifiers.theme.contexts;
    assert.deepEqual(contexts.light, []);
    assert.deepEqual(contexts.dark[0].color.brand.$value, {
      colorSpace: "srgb",
      components: [1, 0, 0],
      alpha: 0.5,
      hex: "#ff0000",
    });
  });

  test("an alias or a formula in a token is left out, not guessed", async () => {
    const penpot = new FakePenpot();
    await ensureLibrary(penpot, coreCatalog, tokens, modifier);
    const catalog = penpot.library.local.tokens;
    const set = catalog.sets.find((s) => s.name === `${MODES_GROUP}/theme/dark`);
    const brand = set?.tokens.find((t) => t.name === "color.brand");
    assert.ok(brand !== undefined);
    brand.value = "{color.ink}";
    const back = readThemes(catalog, tokens) as {
      modifiers: { theme: { contexts: Record<string, unknown[]> } };
    };
    // The brand colour falls back to the base set; the star colour still differs.
    const [dark] = back.modifiers.theme.contexts["dark"] as [{ color: Record<string, unknown> }];
    assert.deepEqual(Object.keys(dark.color), ["star"]);
  });

  test("without a modifier there are no themes and nothing is read back", async () => {
    const penpot = new FakePenpot();
    await ensureLibrary(penpot, coreCatalog, tokens);
    assert.deepEqual(penpot.library.local.tokens.themes, []);
    assert.equal(readThemes(penpot.library.local.tokens, tokens), undefined);
  });

  test("the plugin builds the themes and returns them with the export", async () => {
    const penpot = new FakePenpot();
    const ui = { catalog: coreCatalog, tokens, modifier };
    const { request } = buildRequest(corpusMarkup("login"), ui);
    assert.ok(request);
    const built = await handleRequest(penpot, [], request, { catalog: coreCatalog });
    assert.equal(built.type, "built", JSON.stringify(built));
    if (built.type !== "built") return;
    const root = penpot.currentPage.root.children.find((c) => c.id === built.id) as FakeShape;
    const exported = await handleRequest(penpot, [root], exportRequest(ui), {
      catalog: coreCatalog,
    });
    assert.equal(exported.type, "exported", JSON.stringify(exported).slice(0, 200));
    if (exported.type !== "exported") return;
    const file = finishExport(exported, ui);
    assert.ok(file.resolver !== undefined);
    assertSameModes(JSON.parse(file.resolver), modifier);
  });
});
