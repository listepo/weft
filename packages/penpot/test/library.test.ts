// A library built from several catalogs (SPEC §10.4): each component under its catalog's path, the
// tag lists every catalog, and screens read back as they do from a library of one catalog.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import { readProject } from "@weft/catalog/node";
import { parse, serialize } from "@weft/core";
import { describe, test } from "vitest";
import { buildScreen, displayTexts, ensureLibrary, readScreen } from "../src/index.ts";
import { dataOf } from "../src/layer.ts";
import { FakePenpot } from "./fake-penpot.ts";
import { tokens } from "./helpers.ts";

const { project } = readProject(
  new URL("../../../examples/project/weft.json", import.meta.url).pathname,
);
const sources = { catalogs: [coreCatalog, ...project.catalogs], kinds: project.kinds };
const catalog = project.catalog;

describe("a library of several catalogs", () => {
  test("is tagged with every catalog and groups each component under its catalog", async () => {
    const penpot = new FakePenpot();
    const library = await ensureLibrary(penpot, catalog, tokens, undefined, sources);
    assert.equal(
      dataOf(library.page).getPluginData("weft.library"),
      `weft-core@${coreCatalog.version} acme-ui@1.0.0 shop@1.1.0`,
    );
    for (const [kind, entry] of library.kinds) {
      for (const component of entry.variants.values())
        assert.equal(component.path, project.kinds[kind]?.catalog, kind);
    }
    assert.equal(library.kinds.get("acme-button")?.fallback.path, "acme-ui");
  });

  test("of one catalog leaves the components ungrouped", async () => {
    const library = await ensureLibrary(new FakePenpot(), coreCatalog, tokens);
    assert.equal(library.kinds.get("button")?.fallback.path, "");
  });

  test("builds a screen that reads back unchanged", async () => {
    const penpot = new FakePenpot();
    const library = await ensureLibrary(penpot, catalog, tokens, undefined, sources);
    const markup =
      '<screen weft="0.3">\n  <acme-button variant="ghost">Buy</acme-button>\n</screen>\n';
    const { document } = parse(markup, { catalog, mode: "strict" });
    assert.ok(document !== undefined);
    const { root } = await buildScreen(penpot, document, {
      catalog,
      library,
      tokens,
      display: displayTexts(document),
    });
    const result = await readScreen(root, { catalog, tokens });
    assert.deepEqual(result.losses, []);
    assert.equal(serialize(result.document), serialize(document));
  });
});
