# @weft/figma

Converts Weft screens (see `SPEC.md`) to Figma and back. The conversion is pure TypeScript over a narrow subset of the Figma Plugin API (`src/api.ts`). The plugin in `plugins/figma` runs it inside Figma. The tests run it against an in-memory fake of the same subset.

## What it does

| Step | Function | Result |
| --- | --- | --- |
| Library | `ensureLibrary(api, catalog, tokens)` | A `Weft library` page holding one component set per catalog kind, plus a `Weft tokens` variable collection. Enum props and `state` are the variant axes. Each token becomes a variable: dimensions and numbers as `FLOAT` in px, colors as `COLOR`. Calling it again reuses what is already there. |
| Weft → Figma | `buildScreen(api, document, options)` | Text-only kinds (`content` `none` or `text`) become instances of their variant. Every other kind becomes an auto-layout frame. Each layer carries its Weft source in plugin data: kind, id, props, `on` handlers, and the text content of leaves. A `gap` token is bound to its variable. |
| Figma → Weft | `readScreen(api, layer, options)` | Returns `{ document, losses, diagnostics }` (see "Reading a frame back" below). It is `readLayers`, which only reads layers, followed by `finishRead`, which canonicalizes and validates with the core. |
| Plugin, main thread | `handleRequest(api, selection, message, options)` | Validates a message from the plugin UI, then builds a parsed screen or reads the selected frame. |
| Plugin, UI | `buildRequest`, `exportRequest`, `finishExport` | Parse pasted markup into a build request, and turn a read into `.weft` markup. |

## Where the code runs

The Weft core (`@weft/core`, `@weft/catalog`) is the Rust core compiled to WebAssembly. A Figma plugin runs in two places:

- **The main thread** reaches the file but has no WebAssembly. The official list of what the sandbox provides does not include `WebAssembly`, and it says the sandbox has no `fetch` (https://developers.figma.com/docs/plugins/how-plugins-run/, checked 2026-10-05).
- **The UI iframe** is a browser page. The same page says that inside it you can "access any browser APIs", which includes WebAssembly.

So the work is split:

- **Main thread:** `buildScreen`, `readLayers`, `ensureLibrary` and `handleRequest` never call the core. They need a canonical document as input, and they return an unfinished one.
- **UI:** parsing, canonicalizing, validating, serializing and token loading run here.
- **Value forms:** reading `{$…}` and `{token.…}` in text a designer types (`values.ts`) is a small TypeScript copy of the core's rule. `test/values.test.ts` runs every case through both, so the copy cannot drift.

**Unverified:** the main thread runs in QuickJS compiled to WebAssembly, which would explain why WebAssembly is missing there. This comes from a Figma co-founder's post of 2019-10-02 (https://madebyevan.com/figma/an-update-on-plugin-security/), not from Figma's docs. A community forum answer from 2023-11-26 also says WebAssembly is not supported in the main thread (https://forum.figma.com/ask-the-community-7/does-the-plugin-environment-support-webassembly-31158). The plugin does not depend on either: it works whether or not the main thread has WebAssembly.

### Reading a frame back

`readLayers` recomputes what each layer looked like when it was built (`src/view.ts`) and compares that with what the layer shows now:

- **Unedited layers:** the stored source is kept as written, so an unedited screen comes back byte-identical after `serialize`.
- **Designer edits come back as Weft changes:**
  - text and labels, including bindings typed as `{$.path}`;
  - order;
  - removed layers and added library instances;
  - variants and `state`;
  - visibility, as `hidden`;
  - stack direction, alignment and wrap;
  - grid columns;
  - a gap bound to another variable, or typed as a number that equals a token.
- **Duplicated layers** get a fresh id.
- **Foreign layers** (anything the library did not make) convert lossily:
  - text becomes a string or a `text` element;
  - auto-layout frames become `stack` or `grid`;
  - a frame named after a kind (`button`, or `button#id`) becomes that kind;
  - image fills become `image`;
  - vectors and other childless shapes are dropped.

`losses` has the same shape as the `@weft/from-aria` loss table: `{ kind, path, note }`, with the same `kind` names. These are the kinds this package reports:

| Kind | When |
| --- | --- |
| `ids` | The layer has no Weft source, or it is a copy, so its id was generated. |
| `hidden` | A hidden layer without a Weft source was dropped. |
| `props` | A frame named after a kind has no source; only its text and content were read. |
| `tokens` | A visual edit with no Weft prop (fill, stroke, radius, padding), or a gap that matches no token. |
| `layout` | A frame without auto layout; its children were ordered by position. |
| `kinds` | A layer Weft has no kind for (a vector or an empty shape) was dropped. |
| `names` | A kind that needs a label had none; `""` stands in. |
| `values` | A required prop had no value (from-aria's stand-ins), or an image had no source. |
| `slots` | Two layers claimed the same slot; their content was joined. |
| `structure` | The selection was not a screen, or the layers were too many or too deep. |

## Limits of this stage

- The only token-typed props in the `weft-core` catalog are `stack.gap` and `grid.gap`. Every other visual edit is reported as a `tokens` loss rather than carried into the markup. `docs/figma-style-overrides-design.md` proposes how Weft could keep such edits.
- Containers are frames, not instances. Figma instances cannot take arbitrary children, so a library container instance has to be detached before a designer can fill it. A detached instance is read as a frame named after its kind.
- `rem` tokens are converted at 16 px (`REM_PX`).
- Figma's documentation says nothing about whether plugin data survives duplicating a layer or detaching an instance. The read-back handles both cases:
  - a copy that kept its source gets a fresh id;
  - a layer without a source is read as a foreign layer.

## Figma API facts this package relies on

All facts were checked against the official documentation on 2026-10-05.

| Fact | Source |
| --- | --- |
| Plugin data values are strings and can be read back only by the same plugin. One entry holds at most 100 kB. | https://developers.figma.com/docs/plugins/api/properties/nodes-setplugindata/ |
| `layoutMode` includes `GRID`, with `gridColumnCount`, `gridRowGap` and `gridColumnGap`. `layoutWrap` applies only to horizontal and vertical layouts. | https://developers.figma.com/docs/plugins/api/properties/nodes-layoutmode/ |
| `figma.variables.createVariable(name, collection, resolvedType)` takes the collection object. The form that takes a collection id is deprecated and throws under `documentAccess: "dynamic-page"`. | https://developers.figma.com/docs/plugins/api/properties/figma-variables-createvariable/ |
| `setBoundVariable(field, variable)` binds a numeric field such as `itemSpacing` or a padding, and `null` unbinds it. | https://developers.figma.com/docs/plugins/api/properties/nodes-setboundvariable/ |
| `boundVariables` reports bindings as `VariableAlias` objects. A `cornerRadius` binding appears as the four corner fields. | https://developers.figma.com/docs/plugins/api/properties/nodes-boundvariables/ |
| `setBoundVariableForPaint` returns a new paint, which has to be written back to `fills`. | https://developers.figma.com/docs/plugins/api/figma-variables/ |
| `figma.combineAsVariants(nodes, parent)` needs at least one component and a parent. | https://developers.figma.com/docs/plugins/api/properties/figma-combineasvariants/ |
| On an instance, `setProperties` changes variants. `getMainComponentAsync` is required under dynamic page access, and `componentProperties` replaces the deprecated `variantProperties`. | https://developers.figma.com/docs/plugins/api/InstanceNode/ |
| A text layer's font must be loaded with `loadFontAsync` before `characters`, `fontSize` or `fontName` can change. | https://developers.figma.com/docs/plugins/api/properties/figma-loadfontasync/ |
| Manifest fields:<br>- `name`, `id`, `api`, `main`, `ui`, `editorType`;<br>- `documentAccess: "dynamic-page"`, which new plugins require;<br>- `networkAccess.allowedDomains: ["none"]`.<br>The `ui` file is exposed to the main code as `__html__`. | https://developers.figma.com/docs/plugins/manifest/ |
| The main code runs in a sandbox without the DOM, `fetch` or timers, so it is bundled into one file. | https://developers.figma.com/docs/plugins/how-plugins-run/ and https://developers.figma.com/docs/plugins/libraries-and-bundling/ |
| The UI talks to the main code through `parent.postMessage({ pluginMessage }, "*")` and `figma.ui.postMessage`. | https://developers.figma.com/docs/plugins/creating-ui/ |
| Under dynamic page access, pages are loaded with `page.loadAsync()` and nodes are looked up with `getNodeByIdAsync`. | https://developers.figma.com/docs/plugins/accessing-document/ |
| `@figma/plugin-typings` 1.140.0 is the official type package (MIT). Its `plugin-api-standalone.d.ts` exports the types without declaring globals that clash with `@types/node`. | https://registry.npmjs.org/@figma/plugin-typings (checked 2026-10-05) |

**Unverified:** the official docs do not state these, and the package does not depend on them:

- whether `/` in a variable name groups variables in Figma's UI (variables are named that way here);
- whether plugin data is copied on duplicate or on detach.

## Tests

`vitest run` in this package covers:

- every corpus screen, built into the fake and read back byte-identical;
- the designer edits in `test/edits.test.ts`, each one compared with the original after Weft patches are applied;
- foreign layers and their losses;
- the plugin message handler.

`test/api-types.test.ts` assigns the official `PluginAPI` and node types to the narrow API. If the subset ever drifts from the real API, `tsc` fails.
