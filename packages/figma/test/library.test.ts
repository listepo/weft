// A library built from several catalogs (SPEC §10.4): one section per catalog, the tag lists every
// catalog, and screens read back as they do from a library of one catalog.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { readProject } from "@weft/catalog/node";
import { parse, serialize } from "@weft/core";
import { describe, test } from "vitest";
import { buildScreen, dataOf, displayTexts, ensureLibrary, readScreen } from "../src/index.ts";
import { FakeFigma, FakeSection, type FakeFrame } from "./fake-figma.ts";
import { tokens } from "./helpers.ts";

const { project } = readProject(
  new URL("../../../examples/project/weft.json", import.meta.url).pathname,
);
const sources = { catalogs: [coreCatalog, ...project.catalogs], kinds: project.kinds };
const catalog = project.catalog;

const sectionsOf = (figma: FakeFigma) => {
  const page = figma.root.children.find((p) => p.name === "Weft library");
  return (page?.children ?? []).filter((n): n is FakeSection => n instanceof FakeSection);
};
const kindsIn = (section: FakeSection) =>
  (section.children[0] as FakeFrame).children.map((c) => dataOf(c).getPluginData("weft.kind"));

describe("a library of several catalogs", () => {
  test("is tagged with every catalog and has a section per catalog", async () => {
    const figma = new FakeFigma();
    await ensureLibrary(figma, catalog, tokens, undefined, sources);
    const page = figma.root.children.find((p) => p.name === "Weft library");
    assert.equal(
      page === undefined ? undefined : dataOf(page).getPluginData("weft.library"),
      `weft-core@${coreCatalog.version} acme-ui@1.0.0 shop@1.1.0`,
    );
    const sections = sectionsOf(figma);
    assert.deepEqual(
      sections.map((s) => dataOf(s).getPluginData("weft.library")),
      ["weft-core", "acme-ui", "shop"],
    );
    const [core, acme, shop] = sections.map(kindsIn);
    assert.ok(core?.includes("button"));
    assert.deepEqual(acme, ["acme-button", "acme-rating", "acme-card", "acme-badge"]);
    assert.ok(shop?.every((k) => project.kinds[k]?.catalog === "shop"));
    assert.ok((sections[1]?.y ?? 0) > (sections[0]?.y ?? 0));
  });

  test("adds nothing when built again", async () => {
    const figma = new FakeFigma();
    await ensureLibrary(figma, catalog, tokens, undefined, sources);
    const before = sectionsOf(figma).map(kindsIn);
    await ensureLibrary(figma, catalog, tokens, undefined, sources);
    assert.deepEqual(sectionsOf(figma).map(kindsIn), before);
  });

  test("builds a screen that reads back unchanged", async () => {
    const figma = new FakeFigma();
    const library = await ensureLibrary(figma, catalog, tokens, undefined, sources);
    const markup =
      '<screen weft="0.3">\n  <acme-button variant="ghost">Buy</acme-button>\n</screen>\n';
    const { document } = parse(markup, { catalog, mode: "strict" });
    assert.ok(document !== undefined);
    const frame = await buildScreen(figma, document, {
      catalog,
      library,
      tokens,
      display: displayTexts(document),
    });
    const result = await readScreen(figma, frame, { catalog, tokens });
    assert.deepEqual(result.losses, []);
    assert.equal(serialize(result.document), serialize(document));
  });
});
