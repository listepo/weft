// Token modes (SPEC §10.3): the example project's light and dark contexts become modes of the
// library collection, and the modes read back into a resolver document with the same values.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { describe, test } from "vitest";
import { assertSameModes, exampleModes } from "../../design-tool/test/modes.ts";
import {
  buildRequest,
  ensureLibrary,
  exportRequest,
  finishExport,
  handleRequest,
  readModes,
} from "../src/index.ts";
import { FakeFigma, type FakeNode } from "./fake-figma.ts";
import { corpusMarkup } from "./helpers.ts";

const { tokens, modifier } = exampleModes();

async function collection(figma: FakeFigma) {
  const [c] = await figma.variables.getLocalVariableCollectionsAsync();
  assert.ok(c !== undefined);
  return c;
}

describe("token modes", () => {
  test("each context is a mode, the default context the default mode", async () => {
    const figma = new FakeFigma();
    const library = await ensureLibrary(figma, coreCatalog, tokens, modifier);
    assert.deepEqual(library.notes, []);
    const c = await collection(figma);
    assert.deepEqual(
      c.modes.map((m) => m.name),
      ["light", "dark"],
    );
    assert.equal(c.modes[0]?.modeId, c.defaultModeId);
    const brand = library.variables.get("color.brand");
    const dark = c.modes[1]?.modeId as string;
    assert.notDeepEqual(brand?.valuesByMode[c.defaultModeId], brand?.valuesByMode[dark]);
  });

  test("the modes read back into a resolver with the same values per context", async () => {
    const figma = new FakeFigma();
    await ensureLibrary(figma, coreCatalog, tokens, modifier);
    // A second build finds the modes it made instead of adding more.
    await ensureLibrary(figma, coreCatalog, tokens, modifier);
    assert.equal((await collection(figma)).modes.length, 2);
    assertSameModes(await readModes(figma, tokens), modifier);
  });

  test("a designer's change in one mode reads back in that context only", async () => {
    const figma = new FakeFigma();
    const library = await ensureLibrary(figma, coreCatalog, tokens, modifier);
    const c = await collection(figma);
    const dark = c.modes[1]?.modeId as string;
    library.variables.get("color.brand")?.setValueForMode(dark, { r: 1, g: 0, b: 0, a: 1 });
    const document = (await readModes(figma, tokens)) as {
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
      hex: "#ff0000",
    });
  });

  test("a plan without modes keeps the default mode and notes the rest", async () => {
    const figma = new FakeFigma();
    figma.variables.modeLimit = 1;
    const library = await ensureLibrary(figma, coreCatalog, tokens, modifier);
    assert.equal(library.notes.length, 1);
    assert.match(library.notes[0] as string, /"dark".*Limited to 1 modes only/);
    assert.equal((await collection(figma)).modes.length, 1);
    assert.equal(await readModes(figma, tokens), undefined);
  });

  test("without a modifier the collection keeps one mode and nothing is read back", async () => {
    const figma = new FakeFigma();
    await ensureLibrary(figma, coreCatalog, tokens);
    const c = await collection(figma);
    assert.deepEqual(
      c.modes.map((m) => m.name),
      ["Mode 1"],
    );
    assert.equal(await readModes(figma, tokens), undefined);
  });

  test("the plugin builds the modes and returns them with the export", async () => {
    const figma = new FakeFigma();
    const ui = { catalog: coreCatalog, tokens, modifier };
    const { request } = buildRequest(corpusMarkup("login"), ui);
    assert.ok(request);
    const built = await handleRequest(figma, [], request, { catalog: coreCatalog });
    assert.equal(built.type, "built", JSON.stringify(built));
    if (built.type !== "built") return;
    assert.equal(built.notes, undefined);
    const frame = figma.currentPage.children.find((c) => c.id === built.id) as FakeNode;
    const exported = await handleRequest(figma, [frame], exportRequest(ui), {
      catalog: coreCatalog,
    });
    assert.equal(exported.type, "exported", JSON.stringify(exported).slice(0, 200));
    if (exported.type !== "exported") return;
    const file = finishExport(exported, ui);
    assert.ok(file.resolver !== undefined);
    assertSameModes(JSON.parse(file.resolver), modifier);
  });

  test("a modifier with too many contexts is refused", async () => {
    const figma = new FakeFigma();
    const { request } = buildRequest(corpusMarkup("login"), { catalog: coreCatalog, tokens });
    const contexts = Array.from({ length: 65 }, (_, i) => [`c${i}`, []]);
    const reply = await handleRequest(
      figma,
      [],
      { ...request, modifier: { name: "m", default: "c0", contexts } },
      { catalog: coreCatalog },
    );
    assert.equal(reply.type, "error");
  });
});
