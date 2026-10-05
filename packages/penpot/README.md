# @weft/penpot

Converts Weft screens (see `SPEC.md`) to Penpot and back. The conversion is pure TypeScript over a narrow subset of Penpot's plugin API (`src/api.ts`). The plugin in `plugins/penpot` runs it inside Penpot. The tests run it against an in-memory fake of the same subset.

The mapping itself is not here. It lives in `@weft/design-tool`, which `@weft/figma` uses too. This package is the Penpot side of it:

- `src/layer.ts` shows Penpot shapes to the shared read-back as `Layer`s;
- `src/build.ts` draws the shared build with boards (flex or grid layout) and component copies;
- `src/library.ts` draws the shared library drawings with components, variants and design tokens.

## What it does

| Step | Function | Result |
| --- | --- | --- |
| Library | `ensureLibrary(api, catalog, tokens, modifier?)` | A `Weft library` page with a `Weft components` board. Each catalog kind becomes a component; a kind with enum props or `state` gets one component per combination, combined into variants with one property per axis. Each token becomes a design token in a `Weft tokens` set, named by its Weft path: dimensions as `spacing` (px), numbers as `number`, colors as `color`. Calling it again reuses what is there and updates token values. With a resolver `modifier`, its contexts become token themes (see "Token themes" below). |
| Themes → resolver | `readThemes(catalog, tokens, data)` | The library's theme group as a DTCG resolver document. |
| Weft → Penpot | `buildScreen(api, document, options)` | Text-only kinds become copies of their variant. Every other kind becomes a board with a flex layout, or a grid layout for `grid`. Each shape carries its Weft source in shared plugin data (namespace `weft`). A `gap` token is applied to the board's row and column gap. |
| Penpot → Weft | `readScreen(shape, options)` | `{ document, losses, diagnostics }`, read as described in `@weft/design-tool` and `@weft/figma`: an unedited screen comes back byte-identical; edits come back as Weft changes; foreign shapes convert with a loss table. `readLayers` is the half that runs in the sandbox. |
| Plugin, sandbox | `handleRequest(api, selection, message, options)` | Validates a message from the plugin UI, then builds a parsed screen or reads the selected board. |
| Plugin, UI | `buildRequest`, `exportRequest`, `finishExport` | The same UI helpers as Figma's, from `@weft/design-tool`. |

`PluginOptions` is the extension point for `weft.json`: Penpot has no option of its own, so the reserved `export.penpot` and `import.penpot` sections of the settings table stay empty until one is added.

## Where the code runs

Penpot evaluates a plugin's code as a script in an SES compartment whose globals are the `penpot` API and a fixed list of endowments: timers, `fetch`, `console`, `structuredClone`, `atob`, `btoa` and the standard JavaScript built-ins. `WebAssembly` is not among them. The UI is a page in an iframe, loaded by URL, where browser APIs are available. So the split is the one `@weft/figma` uses: `buildScreen`, `readLayers`, `ensureLibrary` and `handleRequest` never call the core; parsing, value reading, canonicalizing and serializing run in the UI.

## Token themes

A Penpot theme turns a list of token sets on, and only one theme of a group is on at a time (`TokenTheme` in `@penpot/plugin-types` 1.5.0). So a resolver modifier (SPEC §10.3) is a theme group named after it, and each context a theme:

- Every theme turns on the `Weft tokens` set. A context other than the default also turns on its own set, `Weft modes/<modifier>/<context>`, which holds the tokens whose value differs from the default. Among active sets the later one wins, and the context's set is made after the base set.
- The default context's theme is made first and turned on. Turning a set on by hand turns every theme off, so the build only does that for the base set when it is off.
- A token the context's set already has follows the context on later builds, so an override that no longer differs stops differing instead of staying stale.

On export, `readThemes` reads the first theme group that turns the base set on. Each theme is the request's tokens with the values of its sets, later sets winning, and the default context is the theme the build recorded as shared plugin data on the library (`weft.default-context/<group>`, under the namespace `weft`), else the first theme of the group. Token text is parsed as a number, a px length, a `#rrggbb[aa]` or an `rgb()`/`rgba()` colour; an alias (`{color.brand}`) or a formula is left out. The document goes back in the reply's `resolver` field, built as `@weft/figma` describes.

## Penpot specifics

- **Plugin data.** Penpot keys private plugin data by the plugin's id, and the id is a random one made when the plugin is installed; it is kept only when the same browser's plugin list already has a plugin with the same name and host. A file built by one designer's install could not be read by another's, so Weft uses shared plugin data under the namespace `weft`. A missing key reads as nothing; the adapter reads it as `""`.
- **Child order.** A board stores its children bottom to top, and a flex layout runs from the top child down. The library and the build turn on `flags.naturalChildOrdering`, so a flex board's `children` are in flow order; `appendChild` adds at the end of the flow either way. Children of a grid are read in cell order (row, then column).
- **Gaps.** Both the row and the column gap are set to the Weft gap, so turning a column into a row keeps the spacing. The gap read back is the one along the layout's axis (`rowGap` for a column or a grid, `columnGap` for a row). A reversed flex direction has no Weft form and is reported as a `layout` loss.
- **Empty text.** Penpot refuses a text shape without characters, so an empty Weft text is drawn as a zero-width space (`EMPTY_TEXT`) and read back as empty.
- **Tokens.** Literal values are set first, then the token is applied, so a shape shows the right value whatever the timing of the token. The style fingerprint leaves token bindings out, because Penpot applies a token after the call returns. Weft's radius tokens are dimensions, kept as `spacing` tokens, which Penpot cannot apply to a radius, so radii are the token's value in px.
- **Pages.** Penpot refuses changes to a page that is not active, and `createPage` does not open the new page. `ensureLibrary` opens the library page, then opens the designer's page again.
- **Copies.** A component copy's structure cannot change, so containers are boards, not copies, as in Figma.

## Limits of this stage

- Tilts: `rotate-z` is drawn as the shape's `rotation` (taken as clockwise, which the plugin types do not state, so this direction is unverified against a real file); `rotate-x`, `rotate-y` and `perspective` are not drawn and stay in the Weft source, which is what reads back. Turning a shape by hand is a visual edit.
- Visual edits with no Weft prop (fills, strokes, radius, padding, rotation) are reported as `tokens` losses. Style overrides are excluded until `docs/figma-style-overrides-design.md` is approved.
- `rem` tokens are converted at 16 px (`REM_PX`).
- Penpot's default font family is kept; only size and weight are set.

## Penpot API facts this package relies on

All facts were checked on 2026-10-05. "Source" links point at `penpot/penpot` at commit `7c039231f79022f26b252776817c41fa61476aa3` (branch `develop`); the plugin API types are `@penpot/plugin-types` 1.5.0.

| Fact | Source |
| --- | --- |
| A manifest has `name`, `description`, `code`, `icon`, `permissions` and `version`; with `version: 2` paths resolve from the manifest's location. Write permissions include the matching read permission. | https://help.penpot.app/plugins/create-a-plugin/ |
| Penpot fills in `host` from the manifest URL (its folder for version 2) and gives an install a random plugin id unless a registered plugin has the same name and host. `description` is at most 200 characters. | https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/register.cljs and https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/plugins/libs/plugins-runtime/src/lib/models/manifest.schema.ts |
| The UI page is opened with `penpot.ui.open(name, url, options)`; the plugin sends with `penpot.ui.sendMessage` and the UI receives a `message` event; the UI sends with `parent.postMessage` and the plugin receives through `penpot.ui.onMessage`. | https://help.penpot.app/plugins/create-a-plugin/ |
| Plugins are hosted outside Penpot. | https://help.penpot.app/plugins/faq/ |
| The plugin code is evaluated in an SES compartment; the endowments are listed in the sandbox, and `WebAssembly` is not one of them. | https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/plugins/libs/plugins-runtime/src/lib/create-sandbox.ts and https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/plugins/libs/plugins-runtime/src/lib/ses.ts |
| `getSharedPluginData`, `appendChild`, `insertChild`, `children`, `applyToken` (it emits `toggle-token`), `combineAsVariants`, and the checks on permissions, the active page and component-copy structure. With `naturalChildOrdering`, a flex board's `children` are reversed into flow order; `appendChild` puts a child at index 0 of a flex board's shapes. | https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/shape.cljs |
| Grid `appendChild(child, row, column)` refuses negative cells. Penpot's grid layout data counts cells from 1. | https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/grid.cljs and https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/common/src/app/common/types/shape/layout.cljc |
| `createPage`, `openPage`, `createBoard`, `createText` (null for an empty text), `library.local.createComponent`, `tokens.addSet`, `TokenSet.addToken`, `Variants.addProperty/removeProperty/renameProperty`, `LibraryVariantComponent.setVariantProperty`, `shape.switchVariant`, `shape.tokens` (property → token name), the token property names (`rowGap`, `columnGap`, `paddingLeft`…, `fill`, `strokeColor`) and token types (`spacing`, `number`, `color`). | https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/api.cljs, https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/library.cljs, https://github.com/penpot/penpot/blob/7c039231f79022f26b252776817c41fa61476aa3/frontend/src/app/plugins/tokens.cljs and `@penpot/plugin-types` 1.5.0 `index.d.ts` |
| `@penpot/plugin-types` is Penpot's own type package (MPL-2.0, its `LICENSE` file). 1.5.0, published on 2026-07-08 under the `next` tag, is the first with the design tokens API (1.4.2's `index.d.ts` has no `applyToken` or `TokenSet`); `latest` is 1.4.2. | https://registry.npmjs.org/@penpot/plugin-types |

**Unverified:** neither the docs nor the sources settle these. The package does not depend on them, and each is an open item for a check in a real Penpot:

- whether `combineAsVariants` and `applyToken` finish before the call returns (the library polls for the variants);
- whether the variant container stays in the library board (the library moves it there);
- whether a cell given to grid `appendChild` counts from 1 (the build uses 1, as the layout data does);
- whether shared plugin data is copied by duplicate, `instance()`, detach and `switchVariant`, and whether `switchVariant` keeps the shape's id (the read-back handles a copy or a shape without a source either way);
- whether typing over a value that has a token detaches the token in the plugin API, as it does in the UI (the read-back treats a token whose value differs as detached);
- whether a color token accepts `rgba(…)` for a color with alpha;
- whether `(unset)` is accepted as a variant value.

## Tests

`vitest run` in this package covers:

- every corpus screen, built into the fake and read back byte-identical;
- the designer edits in `test/edits.test.ts`, the Figma cases redone with Penpot's operations, plus a reversed stack and a grid;
- that the canvas shows children in Weft order, which fails without natural child ordering;
- a library built twice, and token values updated;
- the plugin message handler.

`test/fake-penpot.ts` models the rules above that the sources state. `test/api-types.test.ts` assigns the official `@penpot/plugin-types` types to the narrow API, so `tsc` fails if the subset drifts.
