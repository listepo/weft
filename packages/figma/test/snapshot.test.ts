// The Figma layer tree every corpus screen builds, as a reviewed file snapshot per screen
// (`__snapshots__/layers/<screen>.txt`). The round trip proves the tree reads back; this makes any
// change to what a designer sees in Figma (layers, auto layout, fills, variables, the Weft plugin
// data) a diff someone reviews. Update with `vitest -u` after reviewing the change.
import { expect, test } from "vitest";
import { layerTree } from "../../design-tool/test/layer-tree.ts";
import { NAMESPACE } from "../src/data.ts";
import type { FakeFigma, FakeFrame } from "./fake-figma.ts";
import { built, corpusMarkup, corpusNames } from "./helpers.ts";

type Layer = {
  id?: string;
  type?: string;
  name?: string;
  children?: Layer[];
  shared?: Map<string, Map<string, string>>;
};

async function tree(figma: FakeFigma, frame: FakeFrame): Promise<string> {
  const ids = new Map<string, string>();
  for (const v of await figma.variables.getLocalVariablesAsync()) ids.set(v.id, `var(${v.name})`);
  const collect = (layer: Layer) => {
    if (layer.id !== undefined) ids.set(layer.id, `layer(${layer.name ?? ""})`);
    for (const child of layer.children ?? []) collect(child);
  };
  collect(frame as unknown as Layer);
  return layerTree(frame, {
    children: (layer) => (layer as Layer).children,
    heading: (layer) => `${(layer as Layer).type} ${JSON.stringify((layer as Layer).name)}`,
    skip: new Set(["id", "type", "name", "parent", "figma", "children", "shared", "x", "y"]),
    defaults: {
      visible: true,
      rotation: 0,
      width: 100,
      height: 100,
      layoutMode: "NONE",
      layoutWrap: "NO_WRAP",
      itemSpacing: 0,
      counterAxisSpacing: null,
      primaryAxisAlignItems: "MIN",
      counterAxisAlignItems: "MIN",
      primaryAxisSizingMode: "FIXED",
      counterAxisSizingMode: "FIXED",
      paddingLeft: 0,
      paddingRight: 0,
      paddingTop: 0,
      paddingBottom: 0,
      gridColumnCount: 1,
      gridRowGap: 0,
      gridColumnGap: 0,
      fills: [],
      strokes: [],
      effects: [],
      strokeWeight: 1,
      cornerRadius: 0,
      gridRowAnchorIndex: undefined,
      gridColumnAnchorIndex: undefined,
      layoutPositioning: undefined,
      bound: {},
      font: { family: "Inter", style: "Regular" },
      size: 12,
    },
    extra: (layer) =>
      [...((layer as Layer).shared?.get(NAMESPACE) ?? new Map<string, string>())]
        .sort(([a], [b]) => (a < b ? -1 : 1))
        .map(([k, v]) => `${k} = ${v}`),
    ids,
  });
}

for (const name of corpusNames) {
  test(`corpus ${name}: the Figma layer tree`, async () => {
    const { figma, frame } = await built(corpusMarkup(name));
    await expect(await tree(figma, frame)).toMatchFileSnapshot(`__snapshots__/layers/${name}.txt`);
  });
}
