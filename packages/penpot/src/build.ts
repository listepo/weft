// Weft to Penpot: the shared build of @weft/design-tool, drawn with Penpot boards (flex or grid
// layout) and component copies. Every element remembers its Weft source in shared plugin data, so
// that reading the shapes back returns the document as written.
import type { Token } from "@weft/catalog";
import type { Catalog, Document } from "@weft/core";
import {
  buildScreen as buildShared,
  findBelow,
  hexOf,
  type BuildHost,
  type Built as BuiltRoot,
  type Display,
  type Marks,
} from "@weft/design-tool";
import type { PBoard, PenpotApi, PGrid, PLibraryComponent, PShape, PText } from "./api.ts";
import { childrenOf, isBoard } from "./api.ts";
import { dataOf, EMPTY_TEXT, isInstance, styleKey } from "./layer.ts";
import { drawText, paintFill, setGap, setPadding, type Library } from "./library.ts";

export type BuildOptions = {
  catalog: Catalog;
  library: Library;
  tokens?: ReadonlyMap<string, Token> | undefined;
  /** The document's `text` and `label` values in attribute form, from `displayTexts`. */
  display: Display;
};

const SCREEN_WIDTH = 480;
const SCREEN_GAP = 120;

const ALL_PADDING = ["leftPadding", "rightPadding", "topPadding", "bottomPadding"] as const;

function marks(shape: PShape): Marks {
  return {
    id: shape.id,
    get name() {
      return shape.name;
    },
    set name(value) {
      shape.name = value;
    },
    get visible() {
      return !shape.hidden;
    },
    set visible(value) {
      shape.hidden = !value;
    },
    ...dataOf(shape),
  };
}

/** Gives a new grid exactly `columns` equal columns and no rows; rows are added as cells fill. */
function setColumns(grid: PGrid, columns: number): void {
  while (grid.rows.length > 0) grid.removeRow(grid.rows.length - 1);
  while (grid.columns.length > columns) grid.removeColumn(grid.columns.length - 1);
  while (grid.columns.length < columns) grid.addColumn("flex", 1);
}

function appendToGrid(board: PBoard, grid: PGrid, child: PShape): void {
  const index = board.children.length;
  const columns = Math.max(grid.columns.length, 1);
  const row = Math.floor(index / columns) + 1;
  while (grid.rows.length < row) grid.addRow("auto");
  grid.appendChild(child, row, (index % columns) + 1);
}

/** The first text layer with this name below a shape. */
export const findText = (shape: PShape, name: string): PText | undefined =>
  findBelow<PShape>(
    shape,
    (c) => c.type === "text" && c.name === name,
    (s) => (isBoard(s) ? childrenOf(s) : undefined),
  ) as PText | undefined;

function penpotHost(api: PenpotApi, library: Library): BuildHost<PShape, PLibraryComponent> {
  let right = 0;
  return {
    async prepare() {
      api.flags.naturalChildOrdering = true;
      const page = api.currentPage;
      right = (page === null ? [] : (childrenOf(page.root) ?? [])).reduce(
        (x, s) => Math.max(x, s.x + s.width + SCREEN_GAP),
        0,
      );
    },
    instance(main, values) {
      const copy = main.instance();
      // Axis order is the variant property order the library gave the component.
      if (values !== undefined)
        Object.values(values).forEach((value, i) => copy.switchVariant(i, value));
      return copy;
    },
    frame(view, fill) {
      const board = api.createBoard();
      paintFill(board, library, fill);
      if (view.material !== undefined) {
        // The tint is the fill and its opacity; the blur is Penpot's own background blur.
        board.fills = [
          { fillColor: hexOf(view.material.color), fillOpacity: view.material.opacity },
        ];
        board.backgroundBlur = { value: view.material.blur, hidden: false };
      }
      const token = view.gap.token === undefined ? undefined : library.tokens.get(view.gap.token);
      if (view.mode === "grid") {
        const grid = board.addGridLayout();
        setColumns(grid, view.columns);
        grid.alignItems = "start";
        grid.horizontalSizing = "auto";
        grid.verticalSizing = "auto";
        setGap(board, grid, view.gap.px, token);
        setPadding(grid, ALL_PADDING, view.padding);
      } else {
        const flex = board.addFlexLayout();
        flex.dir = view.mode;
        flex.wrap = view.wrap ? "wrap" : "nowrap";
        flex.alignItems = view.align;
        flex.horizontalSizing = "auto";
        flex.verticalSizing = "auto";
        setGap(board, flex, view.gap.px, token);
        setPadding(flex, ALL_PADDING, view.padding);
      }
      return board;
    },
    turn(layer, degrees) {
      layer.rotation = degrees;
    },
    text: (drawing) => drawText(api, library, drawing),
    async setText(layer, name, value) {
      const text = findText(layer, name);
      if (text !== undefined) text.characters = value === "" ? EMPTY_TEXT : value;
    },
    append(parent, child) {
      if (!isBoard(parent) || isInstance(parent))
        throw new Error("only a board that is not a copy can hold layers");
      if (parent.grid !== undefined) appendToGrid(parent, parent.grid, child);
      else parent.appendChild(child);
    },
    marks,
    style: (layer, withSpacing) => styleKey(layer, withSpacing),
    place(root, screen) {
      root.x = right;
      root.y = 0;
      if (screen && isBoard(root)) {
        root.resize(SCREEN_WIDTH, Math.max(root.height, 1));
        if (root.flex !== undefined) root.flex.horizontalSizing = "fix";
      }
    },
  };
}

/**
 * Builds the document on the current page, to the right of what is there, and returns its root.
 * The document must be canonical, as `parse` and `canonicalize` return it: canonicalizing here
 * would need the WebAssembly core, which Penpot's plugin sandbox cannot run.
 */
export async function buildScreen(
  api: PenpotApi,
  document: Document,
  options: BuildOptions,
): Promise<BuiltRoot<PBoard>> {
  const { library, ...rest } = options;
  const built = await buildShared(penpotHost(api, library), document, {
    ...rest,
    kinds: library.kinds,
  });
  if (!isBoard(built.root)) throw new Error("a document root must be a board");
  return built as BuiltRoot<PBoard>;
}
