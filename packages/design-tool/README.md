# @weft/design-tool

The part of the Weft ↔ design-tool round trip that does not depend on the tool. `@weft/figma` and `@weft/penpot` use it, so both tools share one implementation of the mapping:

- which kind becomes a library instance and which a frame;
- what the library draws for each kind and variant;
- what a layer stores in plugin data;
- how a layer is read back, edits and foreign layers included;
- the plugin messages and their validation.

Each tool package supplies its side through three narrow interfaces. None of them names a tool.

## The interfaces a tool implements

### `Layer`: the read-back (`src/layer.ts`)

A tool wraps its nodes in `Layer`, and `readLayers(layer, options)` reads them back into Weft.

| Member | Meaning |
| --- | --- |
| `id`, `name`, `visible`, `x`, `y` | As the tool reports them. |
| `kind` | `frame` (a container the reader descends into), `instance` (a library copy), `text` or `other` (groups, shapes). |
| `type` | The tool's layer type in lower case, for loss notes. |
| `children` | In the order the layout shows them; undefined for layers that hold none. |
| `characters` | The text of a text layer; `""` otherwise. |
| `layout` | `{ mode, label, align, alignLabel, wrap, columns }`. `mode` is `row`, `column`, `grid` or `none`. `label` and `alignLabel` are the tool's own names, used in loss notes. |
| `image`, `painted` | Whether a fill is an image; whether the layer has fills or strokes. |
| `variant` | An instance's variant values. |
| `gap(grid)`, `gapToken(grid)` | The spacing between children and the token path bound to it. |
| `style(withSpacing)` | A fingerprint of the visual properties Weft has no prop for. The build stores it; a different value on read is reported as a `tokens` loss. |
| `main()` | An instance's library component: its catalog kind mark and its style. |
| `getPluginData`, `setPluginData` | Plugin data, with `""` for a key that is not set. |

### `BuildHost<L, C>`: the build (`src/build.ts`)

`buildScreen(host, document, options)` decides what to draw. The host draws it: `L` is the tool's layer, `C` its library component.

| Member | Meaning |
| --- | --- |
| `prepare()` | Runs before anything is drawn (fonts, where the screen goes). |
| `instance(main, values)` | A copy of a component; `values` when the exact variant is missing and the copy must be switched. |
| `frame(view, fill)` | A frame with a `LayoutView` (`mode`, `align`, `wrap`, `columns`, `gap`, `padding`). |
| `text(drawing)`, `setText(layer, name, value)` | A text layer; the text of a named layer inside a copy. |
| `append(parent, child)` | Adds a child at the end of the parent's layout order. |
| `marks(layer)` | The layer's name, visibility and plugin data. |
| `style(layer, withSpacing)` | The same fingerprint `Layer.style` returns. |
| `place(root, screen)` | Puts the finished root on the page. |

### `Drawing`: the library (`src/library.ts`)

`drawing(kind, def, values)` returns what a library component draws, as plain data: boxes with a row or column layout, rectangles and text, with colors and lengths that name a token and give a fallback. Each tool turns a drawing into its own layers and keeps its own components and token store (Figma variables, Penpot design tokens).

### `PluginTool<L>`: the plugin (`src/plugin.ts`)

`handleRequest(tool, selection, message)` validates a message from the plugin UI with zod, then calls `tool.build` or `tool.read`. `buildRequest`, `exportRequest` and `finishExport` (`src/plugin-ui.ts`) are the UI side.

A build request may carry a resolver `modifier` (SPEC §10.3), at most 64 contexts, whose contexts the tool turns into its modes; `tool.build` gets it and may return `notes` for what it skipped without failing. A tool that keeps modes implements `tool.modes(tokens)`, and an export reply then carries the modes as a resolver document in `resolver`.

### Token modes (`src/modes.ts`)

`resolverDocument(modifier)` writes a DTCG Resolver Module 2025.10 document: one set `base` with the default context's tokens, the modifier with each context's differing tokens (the default context's list is empty), and a `resolutionOrder` of the two, the modifier's name escaped as a JSON Pointer. A path that is not a DTCG name, or clashes with another token, is left out; every object is made without a prototype. `modeToken(original, value)` turns a value read from a file into a token: the original token when the value is what it already gives, else a `px` dimension, a number or an sRGB colour, and nothing for a value out of range.

## Where the code runs

Both tools run plugin code in a sandbox without WebAssembly, and the UI in a browser iframe. The Weft core is WebAssembly. So:

- `buildScreen`, `readLayers` and `handleRequest` never call the core. They take a canonical document and its display texts (`displayTexts`), and they return an unfinished document.
- `displayTexts`, `finishRead` and the UI helpers call the core. Text a designer typed comes back as a `RawText`, and `finishRead` reads it with the core's `readValue`.

## Tests

`test/shared.test.ts` covers message validation, the neutral layout view, the library drawings and the resolver document. `test/modes.ts` gives the tool packages the example project's light and dark modifier and the check that a resolver read back from a file has the same values per context. The round trips run in the tool packages, over their fakes. `test/corpus.ts` gives those packages the corpus screens and the default tokens, so both tools are tested on the same screens.
