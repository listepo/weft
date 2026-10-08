# @weft/figma

Converts Weft screens (see `SPEC.md`) to Figma and back. The conversion is pure TypeScript over a narrow subset of the Figma Plugin API (`src/api.ts`). The plugin in `plugins/figma` runs it inside Figma. The tests run it against an in-memory fake of the same subset.

The mapping itself is not here. It lives in `@weft/design-tool`, which `@weft/penpot` uses too. This package is the Figma side of it:

- `src/layer.ts` shows Figma nodes to the shared read-back as `Layer`s;
- `src/build.ts` draws the shared build with frames, auto layout and instances;
- `src/library.ts` draws the shared library drawings with components and variables.

## What it does

| Step | Function | Result |
| --- | --- | --- |
| Library | `ensureLibrary(api, catalog, tokens, modifier?)` | A `Weft library` page holding one component set per catalog kind, plus a `Weft tokens` variable collection. Enum props and `state` are the variant axes. Each token becomes a variable: dimensions and numbers as `FLOAT` in px, colors as `COLOR`. Calling it again reuses what is already there. With a resolver `modifier`, its contexts become the collection's modes (see "Token modes" below). |
| Weft → Figma | `buildScreen(api, document, options)` | Text-only kinds (`content` `none` or `text`) become instances of their variant. Every other kind becomes an auto-layout frame. Each layer carries its Weft source in shared plugin data (namespace `weft`, see below): kind, id, props, `on` handlers, the text content of leaves, and the text and label it showed. `options.display` gives the `text` and `label` values in attribute form; `displayTexts(document)` computes it with the core. A `gap` token is bound to its variable. |
| Modes → resolver | `readModes(api, tokens)` | The collection's modes as a DTCG resolver document, when it has more than one. |
| Figma → Weft | `readScreen(api, layer, options)` | Returns `{ document, losses, diagnostics }` (see "Reading a frame back" below). It is `readLayers`, which only reads layers, followed by `finishRead`, which reads typed text as values, canonicalizes and validates with the core. |
| Plugin, main thread | `handleRequest(api, selection, message, options)` | Validates a message from the plugin UI, then builds a parsed screen or reads the selected frame. |
| Plugin, UI | `buildRequest`, `exportRequest`, `finishExport` | Parse pasted markup into a build request, and turn a read into `.weft` markup. |

## Glass

A `stack` or `grid` with a `material` token (`material="{token.material.glass}"`, SPEC §10.3) is drawn as a solid fill of the tint colour at the tint's opacity plus a `BACKGROUND_BLUR` effect of the token's radius (`effects` of `FLayout`; the radius is Figma's own unit, taken as px). The `material` prop itself comes back from the plugin data, like every prop; the style fingerprint includes the effects, so a blur or tint a designer changed by hand is reported as a `tokens` loss, not silently dropped. The library has no variable for a material token, since one variable holds one value and a material is a colour and a blur.

## Token modes

A project whose `tokens` is a DTCG resolver (SPEC §10.3) has modifiers, such as `theme` with the contexts `light` and `dark`. When the build request carries one (`UiOptions.modifier`), `ensureLibrary` makes each context a mode of the `Weft tokens` collection:

- The default context takes the collection's default mode, renamed after it; the other contexts get a mode each, found by name on later builds. The modifier's name is stored on the collection (`weft.modifier`).
- Every variable gets its value in every mode. A context without the token, or with a value of another type, takes the default context's value.
- Modes need a paid Figma plan: `addMode` throws `in addMode: Limited to N modes only` on a plan without them (Figma's plugin typings 1.140.0). The build then keeps the default mode, skips the context and says so in the reply's `notes`; it never fails over a mode.

On export, `readModes` turns a collection with two or more modes back into a resolver document, returned in the reply's `resolver` field (`finishExport` gives it as `tokens.resolver.json` text). The document has one set `base` with the default mode's values and the modifier with, per context, only the tokens whose value differs. A value equal to what the request's token gives keeps that token as it was (`rem`, hex spelling); a changed one becomes a `px` dimension, a number or an sRGB colour. An alias or a malformed value in a mode is left out, as is a second mode with the same name.

## Where the code runs

The Weft core (`@weft/core`, `@weft/catalog`) is the Rust core compiled to WebAssembly. A Figma plugin runs in two places:

- **The main thread** reaches the file but has no WebAssembly. The official list of what the sandbox provides does not include `WebAssembly`, and it says the sandbox has no `fetch` (https://developers.figma.com/docs/plugins/how-plugins-run/, checked 2026-10-05).
- **The UI iframe** is a browser page. The same page says that inside it you can "access any browser APIs", which includes WebAssembly.

So the work is split:

- **Main thread:** `buildScreen`, `readLayers`, `ensureLibrary` and `handleRequest` never call the core and never interpret a value. They need a canonical document and its display texts as input, and they return an unfinished document.
- **UI:** parsing, canonicalizing, validating, serializing and token loading run here, and so do the value forms. `displayTexts` writes `text` and `label` values in attribute form (`{$.name}`, `{token.…}`) for the build. Text a designer typed is left by `readLayers` as a `RawText` (`{ raw, path }`) in the prop, and `finishRead` reads it with the core's `readValue`. There is one implementation of the value rules: the core's.

**Unverified:** the main thread runs in QuickJS compiled to WebAssembly, which would explain why WebAssembly is missing there. This comes from a Figma co-founder's post of 2019-10-02 (https://madebyevan.com/figma/an-update-on-plugin-security/), not from Figma's docs. A community forum answer from 2023-11-26 also says WebAssembly is not supported in the main thread (https://forum.figma.com/ask-the-community-7/does-the-plugin-environment-support-webassembly-31158). The plugin does not depend on either: it works whether or not the main thread has WebAssembly.

### Reading a frame back

`readLayers` recomputes what each layer looked like when it was built (`view.ts` of `@weft/design-tool`; the text and label it showed are stored with the source) and compares that with what the layer shows now:

- **Unedited layers:** the stored source is kept as written, so an unedited screen comes back byte-identical after `serialize`.
- **Designer edits come back as Weft changes:**
  - text and labels, including bindings typed as `{$.path}`;
  - order;
  - removed layers and added library instances;
  - variants and `state`;
  - visibility, as `hidden`;
  - stack direction, alignment and wrap;
  - turning a layer, as a visual edit (below);
  - grid columns;
  - a gap bound to another variable, or typed as a number that equals a token.
- **Models:** a `model` is drawn as a grey 160 by 120 rectangle named `model`; the paths stay in the plugin data, and Figma shows no 3D scene or still.
- **Tilts:** Figma has no 3D transform. `rotate-z` is drawn as the layer's `rotation` (Figma counts counterclockwise, so the sign flips); `rotate-x`, `rotate-y` and `perspective` are not drawn and stay in the layer's Weft source, which is what reads back.
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
| `tokens` | A visual edit with no Weft prop (fill, stroke, radius, padding, rotation), or a gap that matches no token. |
| `layout` | A frame without auto layout; its children were ordered by position. |
| `kinds` | A layer Weft has no kind for (a vector or an empty shape) was dropped. |
| `names` | A kind that needs a label had none; `""` stands in. |
| `values` | A required prop had no value (from-aria's stand-ins), or an image had no source. |
| `slots` | Two layers claimed the same slot; their content was joined. |
| `structure` | The selection was not a screen, or the layers were too many or too deep. |

## Pulling a frame through the REST API

`pullScreen` (`@weft/figma/pull`) reads a frame without opening Figma: the plugins' `figma-pull` script runs it (`docs/claude-code-plugin.md`). It asks `GET /v1/files/:key/nodes` for the frame with `plugin_data=shared`, so each layer's Weft source comes along in its shared plugin data, then once more (`depth=1`, at most 200 ids per request) for the main components of its instances, where the catalog kind and the library style live. `rest.ts` checks every field of the response and turns the nodes into the same read-only node shape the Plugin API's nodes have (`ReadNode` in `layer.ts`), so the read-back above runs unchanged and an unedited frame comes back byte-identical.

- **The token** is a personal access token or OAuth token with the `file_content:read` scope. It goes only into the `X-Figma-Token` header; the script reads it from `FIGMA_TOKEN` and never from a flag, and every error has it replaced by `[token]`.
- **Responses** larger than 50 MB are refused before they are parsed. 400, 403, 404 and 429 become messages that say why (with the `Retry-After` wait); the server's `err` text is shown without control characters.
- **Variables:** the REST API names no variables without the Variables API, which needs a Full seat in an Enterprise org. A gap bound to another variable is therefore read by its value, like a typed number.
- **Cost:** a pull is two requests (one when the frame has no instances). File-node requests are rate-limit tier 1, which allows View and Collab seats only a few requests a month.

### REST API facts the pull relies on

Checked against the official documentation on 2026-10-08.

| Fact | Source |
| --- | --- |
| Every endpoint is under `https://api.figma.com` (Figma for Government uses `https://api.figma-gov.com`, which the script does not call). | https://developers.figma.com/docs/rest-api/ |
| A personal access token is sent in the `X-Figma-Token` header. | https://developers.figma.com/docs/rest-api/personal-access-tokens/ |
| Reading file content needs the `file_content:read` scope. | https://developers.figma.com/docs/rest-api/authentication/ |
| `GET /v1/files/:key/nodes` takes `ids` (comma-separated, required), `depth`, `geometry`, `version` and `plugin_data` (plugin ids and/or `shared`). `:key` may be a branch key. The `nodes` map holds `{ document, components, componentSets, styles, schemaVersion }` per id, and `null` for an id that is not in the file. Errors: 400 (with `err` naming the parameter), 403 (invalid or expired token), 404 (file not found). Rate limit tier 1, scope `file_content:read`. | https://developers.figma.com/docs/rest-api/file-endpoints/ |
| `pluginData` holds the data of the plugins named in `plugin_data`, visible only to the plugin that wrote it; `sharedPluginData`, data visible to all plugins, needs `shared`. Both are typed `Any`. `visible` defaults to `true`. | https://developers.figma.com/docs/rest-api/files/ (`sharedPluginData` checked 2026-10-08) |
| Frame fields and their defaults: `fills`, `strokes`, `effects` `[]`; `layoutMode` `NONE` (`HORIZONTAL`, `VERTICAL`, `GRID`); `layoutWrap` `NO_WRAP` or `WRAP`; `counterAxisAlignItems` `MIN`; `itemSpacing`, the paddings, `gridRowGap` `0`; `gridColumnCount` with `GRID`; `cornerRadius`, and `rectangleCornerRadii` for four corners; `componentId` and `componentProperties` on instances; `characters` on text; `absoluteBoundingBox`. | https://developers.figma.com/docs/rest-api/file-node-types/ |
| A solid paint is `{ type, color: { r, g, b, a }, opacity?, boundVariables?: { color } }`; a background blur is `{ type: "BACKGROUND_BLUR", radius, visible }`; a component property is `{ type, value }`. | https://github.com/figma/rest-api-spec (`dist/api_types.ts`, main branch) |
| 429 carries `Retry-After` in seconds. Tier 1 allows View and Collab seats up to 20 requests a month; Dev and Full seats 10 to 20 a minute. | https://developers.figma.com/docs/rest-api/rate-limits/ |
| The Variables API needs a Full seat in an Enterprise org. | https://developers.figma.com/docs/rest-api/variables/ |
| A link's `node-id` writes `1:2` as `1-2`; a branch link is `figma.com/design/:fileKey/branch/:branchKey/:fileName`. | The tool descriptions of Figma's own MCP server (`get_metadata`, `use_figma`), read 2026-10-08 |

A request to the real API with a made-up token got 403 and `{"err":"Invalid token"}` on 2026-10-08, as the docs say.

**Unverified:** the pull does not depend on these for correctness, but a wrong guess shows up as a spurious `tokens` loss on an unedited layer:

- **`rotation` is in radians.** The docs say only "the rotation of the node, if not 0"; a forum report shows a radian-sized value (https://forum.figma.com/archive-21/rotate-value-not-exported-34766). The pull converts it to the Plugin API's degrees.
- **Numbers match the Plugin API's.** The style fingerprint the plugin stored is compared with one computed from REST values (colours, radii, padding); the docs do not say both APIs print the same digits.

Both are to be checked on a real file, together with the plugin itself.

## Where the Weft source is stored

Every mark (the layer's source, the library page and board, component kinds, token variables and their collection) is shared plugin data under the namespace `weft` (`NAMESPACE` in `src/data.ts`, the namespace Penpot's plugin uses too). Shared plugin data can be read by any plugin, by the Figma MCP server's `use_figma`, and through the REST API with `plugin_data=shared`; private plugin data (`setPluginData`) can be read only by the plugin id that wrote it.

Only shared plugin data is supported. `dataOf(node)` is the one way the package reaches plugin data, and it reads and writes shared data only; the narrow API (`FPluginData` in `src/api.ts`) has no private calls. A file whose Weft source sits in private plugin data, as the plugin's first versions stored it, reads as foreign layers; rebuild it with the current plugin.

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
| `setSharedPluginData(namespace, key, value)` writes data any plugin can read; the namespace must be at least 3 alphanumeric characters; the empty string removes the entry; one entry holds at most 100 kB. `getSharedPluginData` returns `""` for a missing key. Nodes, variables and variable collections have both (`PluginDataMixin` in `@figma/plugin-typings` 1.140.0). | https://developers.figma.com/docs/plugins/api/properties/nodes-setsharedplugindata/ (checked 2026-10-08) |
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
- whether plugin data is copied on duplicate or on detach;
- the shape of the REST API's `sharedPluginData`: the docs type it `Any`, and the pull reads it as `pluginData` is read, an object of entries per namespace (`{ "weft": { "weft.source": "…" } }`). A pull from a real file confirms it.

## Tests

`vitest run` in this package covers:

- every corpus screen, built into the fake and read back byte-identical;
- the designer edits in `test/edits.test.ts`, each one compared with the original after Weft patches are applied;
- foreign layers and their losses;
- the plugin message handler;
- the pull (`test/pull.test.ts`): every corpus screen built into the fake, written as REST JSON with the documented defaults left out (`test/rest-server.ts`), served on a local port and pulled back byte-identical; a designer's edit; the requests' header and parameters; the error messages and the token's redaction; link parsing.
- shared plugin data (`test/shared-data.test.ts`): a build writes the source only as shared data under `weft`, and the frame reads back byte-identical. The layer snapshots list the shared data. The pull asks for shared data only, and a frame without it reads as foreign layers.

`test/api-types.test.ts` assigns the official `PluginAPI` and node types to the narrow API. If the subset ever drifts from the real API, `tsc` fails.
