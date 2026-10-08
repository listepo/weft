// Grid anchors travel from a Figma node, REST or plugin, onto the shared layer, and a foreign
// grid is then read in cell order.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import type { Node } from "@weft/core";
import { readLayers } from "@weft/design-tool";
import { describe, test } from "vitest";
import { figmaLayers } from "../src/layer.ts";
import { NO_VARIABLES, restNode } from "../src/rest.ts";
import { FakeFigma, type FakeFrame } from "./fake-figma.ts";

function texts(node: Node): string[] {
  return (node.children ?? []).map((child) => {
    assert.ok(typeof child === "object");
    const text = child.children?.[0];
    assert.ok(typeof text === "string");
    return text;
  });
}

async function foreignGrid(layer: Parameters<ReturnType<typeof figmaLayers>>[0]): Promise<Node> {
  const result = await readLayers(figmaLayers(NO_VARIABLES)(layer), { catalog: coreCatalog });
  const child = result.document.root.children?.[0];
  assert.ok(typeof child === "object");
  assert.equal(child.kind, "grid");
  return child;
}

describe("grid anchors from the REST node", () => {
  test("are read onto the layer and order the children", async () => {
    const node = restNode(
      {
        id: "1:1",
        name: "Cells",
        type: "FRAME",
        layoutMode: "GRID",
        gridColumnCount: 2,
        fills: [],
        children: [
          {
            id: "1:2",
            name: "b",
            type: "TEXT",
            characters: "B",
            gridRowAnchorIndex: 0,
            gridColumnAnchorIndex: 1,
          },
          {
            id: "1:4",
            name: "pinned",
            type: "TEXT",
            characters: "P",
            layoutPositioning: "ABSOLUTE",
            gridRowAnchorIndex: 1,
            gridColumnAnchorIndex: 0,
          },
          {
            id: "1:3",
            name: "a",
            type: "TEXT",
            characters: "A",
            gridRowAnchorIndex: 0,
            gridColumnAnchorIndex: 0,
          },
        ],
      },
      {},
    );
    const grid = await foreignGrid(node);
    assert.deepEqual(texts(grid), ["A", "P", "B"]);
  });

  test("a field that is not a number is absent, so z-order stays", async () => {
    const node = restNode(
      {
        id: "1:1",
        name: "Cells",
        type: "FRAME",
        layoutMode: "GRID",
        gridColumnCount: 2,
        fills: [],
        children: [
          { id: "1:2", name: "b", type: "TEXT", characters: "B", gridRowAnchorIndex: "0" },
          { id: "1:3", name: "a", type: "TEXT", characters: "A", gridColumnAnchorIndex: null },
        ],
      },
      {},
    );
    const grid = await foreignGrid(node);
    assert.deepEqual(texts(grid), ["B", "A"]);
  });
});

describe("grid anchors from the plugin node", () => {
  test("are read onto the layer and order the children", async () => {
    const figma = new FakeFigma();
    await figma.loadFontAsync({ family: "Inter", style: "Regular" });
    const frame: FakeFrame = figma.createFrame();
    frame.name = "Cells";
    frame.layoutMode = "GRID";
    frame.gridColumnCount = 2;
    frame.fills = [];
    const later = figma.createText();
    later.characters = "B";
    later.gridRowAnchorIndex = 0;
    later.gridColumnAnchorIndex = 1;
    const earlier = figma.createText();
    earlier.characters = "A";
    earlier.gridRowAnchorIndex = 0;
    earlier.gridColumnAnchorIndex = 0;
    frame.appendChild(later);
    frame.appendChild(earlier);
    const grid = await foreignGrid(frame);
    assert.deepEqual(texts(grid), ["A", "B"]);
  });
});
