// The Penpot shape tree every corpus screen builds, as a reviewed file snapshot per screen
// (`__snapshots__/layers/<screen>.txt`). The round trip proves the tree reads back; this makes any
// change to what a designer sees in Penpot (boards, flex and grid layout, fills, tokens, the Weft
// plugin data) a diff someone reviews. Update with `vitest -u` after reviewing the change.
import { expect, test } from "vitest";
import { layerTree } from "../../design-tool/test/layer-tree.ts";
import { childrenOf, type PShape } from "../src/index.ts";
import type { FakeBoard } from "./fake-penpot.ts";
import { built, corpusMarkup, corpusNames } from "./helpers.ts";

type Shape = { id: string; type: string; name: string; data: Map<string, string> };

function tree(root: FakeBoard): string {
  const ids = new Map<string, string>();
  const collect = (shape: PShape) => {
    ids.set((shape as unknown as Shape).id, `shape(${shape.name})`);
    for (const child of childrenOf(shape) ?? []) collect(child);
  };
  collect(root as unknown as PShape);
  return layerTree(root, {
    children: (shape) => childrenOf(shape as PShape),
    heading: (shape) => `${(shape as Shape).type} ${JSON.stringify((shape as Shape).name)}`,
    skip: new Set(["id", "type", "name", "parent", "penpot", "shapes", "data", "x", "y"]),
    defaults: {
      width: 100,
      height: 100,
      hidden: false,
      rotation: 0,
      borderRadius: 0,
      strokes: [],
      layoutCell: undefined,
      tokens: {},
      inCopy: false,
      head: false,
      componentOf: null,
      fillList: [],
      flex: undefined,
      grid: undefined,
      variantContainer: false,
      variantsReady: null,
      fontSize: "14",
      fontWeight: "400",
      growType: "fixed",
    },
    extra: (shape) =>
      [...(shape as Shape).data]
        .sort(([a], [b]) => (a < b ? -1 : 1))
        .map(([k, v]) => `${k} = ${v}`),
    ids,
    inline: new Set(["FakeFlex", "FakeGrid"]),
  });
}

for (const name of corpusNames) {
  test(`corpus ${name}: the Penpot shape tree`, async () => {
    const { root } = await built(corpusMarkup(name));
    await expect(tree(root)).toMatchFileSnapshot(`__snapshots__/layers/${name}.txt`);
  });
}
