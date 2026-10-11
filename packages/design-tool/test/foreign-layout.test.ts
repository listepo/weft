// Foreign frames carry cross-axis alignment, main-axis alignment, wrap, padding and max width,
// except the defaults SPEC §5.1 tells an importer not to write back: a row's center, a column's
// start, and `justify`'s start. A layer that already stores a Weft source is read from that
// source. Grid children with anchors are ordered by cell.
import assert from "node:assert/strict";
import { coreCatalog } from "@weft/catalog";
import type { Node } from "@weft/core";
import { describe, test } from "vitest";
import {
  KEY,
  readLayers,
  writeJson,
  type Layer,
  type LayerLayout,
  type Source,
} from "../src/index.ts";
import { tokens } from "./corpus.ts";

const layoutOf = (layout: Partial<LayerLayout> & Pick<LayerLayout, "mode">): LayerLayout => ({
  label: "NONE",
  align: "start",
  alignLabel: "MIN",
  wrap: false,
  columns: 1,
  justify: "start",
  justifyLabel: "MIN",
  padding: 0,
  maxWidth: undefined,
  minWidth: undefined,
  flexTracks: true,
  ...layout,
});

function makeLayer(init: {
  kind: Layer["kind"];
  name: string;
  layout?: LayerLayout;
  children?: readonly Layer[];
  characters?: string;
  source?: Source;
  sizing?: Layer["sizing"];
  gridRowAnchorIndex?: number;
  gridColumnAnchorIndex?: number;
  layoutPositioning?: "AUTO" | "ABSOLUTE";
}): Layer {
  const stored = new Map<string, string>();
  const layer: Layer = {
    id: init.name,
    kind: init.kind,
    type: init.kind,
    name: init.name,
    visible: true,
    x: 0,
    y: 0,
    children: init.kind === "text" ? undefined : (init.children ?? []),
    characters: init.characters ?? "",
    layout: init.layout ?? layoutOf({ mode: "none" }),
    image: false,
    painted: false,
    variant: {},
    sizing: init.sizing ?? { horizontal: "hug", vertical: "hug" },
    gap: () => 0,
    gapToken: async () => undefined,
    paddingToken: async () => undefined,
    maxWidthToken: async () => undefined,
    style: () => "",
    main: async () => undefined,
    getPluginData: (key) => stored.get(key) ?? "",
    setPluginData: (key, value) => {
      if (value === "") stored.delete(key);
      else stored.set(key, value);
    },
    ...(init.gridRowAnchorIndex === undefined
      ? {}
      : { gridRowAnchorIndex: init.gridRowAnchorIndex }),
    ...(init.gridColumnAnchorIndex === undefined
      ? {}
      : { gridColumnAnchorIndex: init.gridColumnAnchorIndex }),
    ...(init.layoutPositioning === undefined ? {} : { layoutPositioning: init.layoutPositioning }),
  };
  if (init.source !== undefined) {
    writeJson(layer, KEY.source, init.source);
    layer.setPluginData(KEY.origin, layer.id);
  }
  return layer;
}

const text = (
  characters: string,
  place?: { row?: number; column?: number; absolute?: boolean },
): Layer =>
  makeLayer({
    kind: "text",
    name: characters,
    characters,
    ...(place?.row === undefined ? {} : { gridRowAnchorIndex: place.row }),
    ...(place?.column === undefined ? {} : { gridColumnAnchorIndex: place.column }),
    ...(place?.absolute === true ? { layoutPositioning: "ABSOLUTE" as const } : {}),
  });

const frame = (
  layout: LayerLayout,
  children: readonly Layer[],
  source?: Source,
  sizing?: Layer["sizing"],
): Layer =>
  makeLayer({
    kind: "frame",
    name: "Frame",
    layout,
    children,
    ...(source ? { source } : {}),
    ...(sizing ? { sizing } : {}),
  });

async function readForeign(layer: Layer): Promise<{ node: Node; notes: string[] }> {
  const result = await readLayers(layer, { catalog: coreCatalog });
  const child = result.document.root.children?.[0];
  assert.ok(typeof child === "object");
  return {
    node: child,
    notes: result.losses
      .filter((loss) => loss.kind === "layout" || loss.kind === "tokens")
      .map((loss) => loss.note),
  };
}

/** The same read with the default tokens, so a px value can name a token. */
async function readLayersToken(layer: Layer): Promise<{ node: Node; notes: string[] }> {
  const result = await readLayers(layer, { catalog: coreCatalog, tokens });
  const child = result.document.root.children?.[0];
  assert.ok(typeof child === "object");
  return {
    node: child,
    notes: result.losses
      .filter((loss) => loss.kind === "layout" || loss.kind === "tokens")
      .map((loss) => loss.note),
  };
}

const shown = (node: Node): string[] =>
  (node.children ?? []).map((child) => {
    assert.ok(typeof child === "object");
    const text = child.children?.[0];
    assert.ok(typeof text === "string");
    return text;
  });

describe("a foreign horizontal frame aligned to the start", () => {
  test("reads back as a row with align start and no wrap", async () => {
    const { node } = await readForeign(
      frame(layoutOf({ mode: "row", align: "start", wrap: false }), [text("Hi")]),
    );
    assert.equal(node.kind, "stack");
    assert.deepEqual(node.props, { direction: "row", align: "start" });
  });
});

describe("a foreign horizontal frame centered and wrapping", () => {
  test("reads wrap and leaves the row's center unset", async () => {
    const { node } = await readForeign(
      frame(layoutOf({ mode: "row", align: "center", alignLabel: "CENTER", wrap: true }), [
        text("Hi"),
      ]),
    );
    assert.deepEqual(node.props, { direction: "row", wrap: true });
  });
});

describe("a foreign vertical frame aligned to the start", () => {
  test("leaves align and wrap unset", async () => {
    const { node } = await readForeign(
      frame(layoutOf({ mode: "column", align: "start", wrap: false }), [text("Hi")]),
    );
    assert.deepEqual(node.props, {});
  });
});

describe("a foreign row whose alignment is not the default", () => {
  test("keeps end and stretch, and drops a baseline", async () => {
    const end = await readForeign(
      frame(layoutOf({ mode: "row", align: "end", alignLabel: "MAX" }), []),
    );
    const stretch = await readForeign(
      frame(layoutOf({ mode: "row", align: "stretch", alignLabel: "STRETCH" }), []),
    );
    const baseline = await readForeign(
      frame(layoutOf({ mode: "row", align: undefined, alignLabel: "BASELINE" }), []),
    );
    assert.deepEqual(end.node.props, { direction: "row", align: "end" });
    assert.deepEqual(stretch.node.props, { direction: "row", align: "stretch" });
    assert.deepEqual(baseline.node.props, { direction: "row" });
  });
});

describe("a foreign column whose alignment is not the host default", () => {
  test("keeps center, end and stretch", async () => {
    for (const align of ["center", "end", "stretch"] as const) {
      const { node } = await readForeign(frame(layoutOf({ mode: "column", align }), []));
      assert.deepEqual(node.props, { align }, align);
    }
  });
});

describe("a stack that already has a Weft source", () => {
  test("keeps the stored align, including a row's center", async () => {
    const { node } = await readForeign(
      frame(layoutOf({ mode: "row", align: "center", alignLabel: "CENTER", wrap: true }), [], {
        kind: "stack",
        id: "row",
        props: { direction: "row", align: "center", wrap: true },
      }),
    );
    assert.equal(node.id, "row");
    assert.deepEqual(node.props, { direction: "row", align: "center", wrap: true });
  });

  test("keeps a column's stored start", async () => {
    const { node } = await readForeign(
      frame(layoutOf({ mode: "column", align: "start" }), [], {
        kind: "stack",
        id: "col",
        props: { align: "start" },
      }),
    );
    assert.equal(node.id, "col");
    assert.deepEqual(node.props, { align: "start" });
  });
});

describe("a foreign grid", () => {
  const grid = (children: readonly Layer[]): Layer =>
    frame(layoutOf({ mode: "grid", label: "GRID", columns: 2 }), children);

  test("orders children by row, then column, and notes the change", async () => {
    const { node, notes } = await readForeign(
      grid([
        text("B", { row: 0, column: 1 }),
        text("C", { row: 1, column: 0 }),
        text("A", { row: 0, column: 0 }),
      ]),
    );
    assert.equal(node.kind, "grid");
    assert.deepEqual(node.props, { columns: 2 });
    assert.deepEqual(shown(node), ["A", "B", "C"]);
    assert.deepEqual(notes, [
      "grid children are ordered by their row and column anchors; their z-order was not kept",
    ]);
  });

  test("keeps an absolutely positioned child at its index and notes the loss", async () => {
    const { node, notes } = await readForeign(
      grid([
        text("B", { row: 0, column: 1 }),
        text("Abs", { absolute: true, row: 1, column: 1 }),
        text("A", { row: 0, column: 0 }),
      ]),
    );
    assert.deepEqual(shown(node), ["A", "Abs", "B"]);
    assert.deepEqual(notes, [
      "grid children are ordered by their row and column anchors; their z-order was not kept",
      "absolute positioning has no Weft form and was kept in flow",
    ]);
  });

  test("keeps z-order when no child carries an anchor", async () => {
    const { node, notes } = await readForeign(grid([text("B"), text("A")]));
    assert.deepEqual(shown(node), ["B", "A"]);
    assert.deepEqual(notes, []);
  });

  test("adds no loss when the anchors already match z-order", async () => {
    const { node, notes } = await readForeign(
      grid([text("A", { row: 0, column: 0 }), text("B", { row: 0, column: 1 })]),
    );
    assert.deepEqual(shown(node), ["A", "B"]);
    assert.deepEqual(notes, []);
  });

  test("drops grid tracks that are not equal flex tracks", async () => {
    const { notes } = await readForeign(
      frame(layoutOf({ mode: "grid", label: "GRID", columns: 2, flexTracks: false }), [text("A")]),
    );
    assert.deepEqual(notes, [
      "grid tracks that are not equal flex tracks were drawn as equal columns",
    ]);
  });
});

describe("a foreign stack's main-axis alignment", () => {
  test("keeps center, end and space-between, and leaves start out", async () => {
    for (const [justify, expected] of [
      ["center", { direction: "row", justify: "center" }],
      ["end", { direction: "row", justify: "end" }],
      ["space-between", { direction: "row", justify: "space-between" }],
    ] as const) {
      const { node, notes } = await readForeign(
        frame(layoutOf({ mode: "row", align: "center", justify }), [text("Hi")]),
      );
      assert.deepEqual(node.props, expected, justify);
      assert.deepEqual(notes, []);
    }
    const start = await readForeign(
      frame(layoutOf({ mode: "row", align: "center", justify: "start" }), []),
    );
    assert.deepEqual(start.node.props, { direction: "row" });
  });

  test("drops space-around and space-evenly as layout losses", async () => {
    for (const [label, justifyLabel] of [
      ["space-around", "SPACE_AROUND"],
      ["space-evenly", "SPACE_EVENLY"],
    ] as const) {
      const { node, notes } = await readForeign(
        frame(layoutOf({ mode: "row", align: "center", justify: undefined, justifyLabel }), [
          text("Hi"),
        ]),
      );
      assert.deepEqual(node.props, { direction: "row" }, label);
      assert.deepEqual(notes, [`main-axis alignment ${justifyLabel} has no Weft value`]);
    }
  });
});

describe("a foreign child of a stack", () => {
  const parent = (child: Layer, mode: "row" | "column" = "row"): Layer =>
    frame(layoutOf({ mode }), [child]);
  const box = (init: { sizing?: Layer["sizing"]; minWidth?: number }): Layer =>
    makeLayer({
      kind: "frame",
      name: "Box",
      layout: layoutOf({
        mode: "column",
        ...(init.minWidth === undefined ? {} : { minWidth: init.minWidth }),
      }),
      ...(init.sizing === undefined ? {} : { sizing: init.sizing }),
    });

  test("reads fill on the parent's main axis as grow", async () => {
    const row = await readForeign(parent(box({ sizing: { horizontal: "fill", vertical: "hug" } })));
    const column = await readForeign(
      parent(box({ sizing: { horizontal: "hug", vertical: "fill" } }), "column"),
    );
    const childOf = (node: Node) => {
      const child = node.children?.[0];
      assert.ok(typeof child === "object");
      return child;
    };
    assert.equal(childOf(row.node).props?.["grow"], true);
    assert.equal(childOf(column.node).props?.["grow"], true);
  });

  test("drops a fixed size on the main axis as a layout loss", async () => {
    const { notes } = await readForeign(
      parent(box({ sizing: { horizontal: "fixed", vertical: "hug" } })),
    );
    assert.deepEqual(notes, ["a fixed size has no Weft form and was dropped"]);
  });

  test("drops minWidth as a layout loss", async () => {
    const { notes } = await readForeign(parent(box({ minWidth: 80 })));
    assert.deepEqual(notes, ["minWidth has no Weft form and was dropped"]);
  });
});

describe("a foreign stack's box", () => {
  test("keeps four equal paddings that name a token", async () => {
    const { node, notes } = await readLayersToken(
      frame(layoutOf({ mode: "column", padding: 16 }), [text("Hi")], undefined, undefined),
    );
    assert.deepEqual(node.props, { padding: { token: "space.md" } });
    assert.deepEqual(notes, []);
  });

  test("drops unequal paddings as a layout loss", async () => {
    const layer = frame(layoutOf({ mode: "column", padding: undefined }), [text("Hi")]);
    const { node, notes } = await readForeign(layer);
    assert.deepEqual(node.props, {});
    assert.deepEqual(notes, ["the paddings differ; Weft has one token on all four sides"]);
  });

  test("keeps a max width that names a token", async () => {
    const { node, notes } = await readLayersToken(
      frame(layoutOf({ mode: "column", maxWidth: 480 }), [text("Hi")]),
    );
    assert.deepEqual(node.props, { "max-width": { token: "size.md" } });
    assert.deepEqual(notes, []);
  });
});
