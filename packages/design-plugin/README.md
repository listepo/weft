# @weft/design-plugin

The shell shared by Weft's design-tool plugins (`plugins/figma`, `plugins/penpot`). Both tools run a plugin's code in a sandbox without WebAssembly and show its UI in an iframe, so both plugins are built and wired the same way. A plugin gives only its entries and how its tool carries messages.

| Export | What it does |
| --- | --- |
| `@weft/design-plugin/build`: `buildDesignPlugin(options)` | Builds the sandbox entry into one classic script, with the core's WebAssembly loader replaced by a stub (`src/no-wasm.ts`) that fails with a reason if anything reaches it. Builds the UI entry into one HTML page (`src/ui.html`) with the core and its module bytes inlined (`src/wasm-inline.ts`), so the page needs no files beside it and no network. `ui.tool` and `ui.layer` name the tool and the layer it builds in the page. |
| `startUi(transport)` | Runs the plugin UI: build, export, copy and download. It parses and finishes with the core and shows replies as text, never HTML. `Transport` is `{ send(request), listen(onReply) }`; replies are checked before use. |

Message validation and the request and reply types live in `@weft/design-tool`.

## Tests

`test/ui-harness.ts` runs a built `ui.html` in Node with a fake DOM and without `process` or `fetch`, so the core must load from the inlined bytes. The Figma and Penpot plugin harnesses use it for their UI half. `test/ui.test.ts` covers the UI against a scripted sandbox.
