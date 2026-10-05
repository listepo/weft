# @weft/design-plugin

The shell shared by Weft's design-tool plugins (`plugins/figma`, `plugins/penpot`). Both tools run a plugin's code in a sandbox without WebAssembly and show its UI in an iframe, so both plugins are built and wired the same way. A plugin gives only its entries and how its tool carries messages.

| Export | What it does |
| --- | --- |
| `@weft/design-plugin/build`: `buildDesignPlugin(options)` | Builds the sandbox entry into one classic script, with the core's WebAssembly loader replaced by a stub (`src/no-wasm.ts`) that fails with a reason if anything reaches it. Builds the UI entry into one HTML page (`src/ui.html`) with the core and its module bytes inlined (`src/wasm-inline.ts`), so the page needs no files beside it and no network. `ui.tool` and `ui.layer` name the tool and the layer it builds in the page. |
| `startUi(transport)` | Runs the plugin UI: build, export, copy and download, and token modes. It parses and finishes with the core and shows replies as text, never HTML. `Transport` is `{ send(request), listen(onReply) }`; replies are checked before use. |
| `readResolver(text)` (`src/resolver.ts`) | Reads a pasted or picked DTCG resolver document for the UI: at most `MAX_RESOLVER` (one million) characters, JSON only, then `loadProject({ tokens })` of `@weft/catalog`, which is the Rust resolver loader, so there is no TypeScript resolver parser. It returns the default context's tokens, the modifiers, the appearance modifier's name and the loader's diagnostics (`W705`, and `W704` for a file `$ref`, which a document without files cannot follow). |

### Token modes in the page

Under "Token modes (optional)" the page has a file picker and a paste box for a resolver document, a "Load resolver" button and a "Modes from" list of the modifiers plus "No modes". Loading lists the loader's diagnostics as notes; the appearance modifier is preselected, else "No modes". The loaded default context is the token set of build and export, and the chosen modifier is sent as `UiOptions.modifier` with each build. An empty box clears the resolver and goes back to the default tokens and one mode. A resolver file over the bound is refused before it is read.

After an export whose reply has a `resolver`, "Token modes of the file" shows it and "Download tokens.resolver.json" saves it; a later export without modes clears both. The exported document has its sets inline, so it loads back through the same box.

Message validation and the request and reply types live in `@weft/design-tool`.

## Tests

`test/ui-harness.ts` runs a built `ui.html` in Node with a fake DOM and without `process` or `fetch`, so the core must load from the inlined bytes. The Figma and Penpot plugin harnesses use it for their UI half. `test/ui.test.ts` covers the UI against a scripted sandbox.
