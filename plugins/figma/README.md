# Weft Figma plugin

A Figma plugin with two actions:

- **Build:** paste a `.weft` screen or open one, and the plugin builds it as frames and library instances.
- **Export:** select a frame and export it back as `.weft`. The plugin lists any losses along with the markup. When the file has token modes, it also shows them as a DTCG resolver document and offers `tokens.resolver.json` for download.

Token modes are optional. Paste a resolver document, or pick its file, under "Token modes", press "Load resolver" and choose the modifier whose contexts become the `Weft tokens` variable modes ("Modes from"). The appearance modifier (`light` and `dark`) is chosen when the resolver has one; "No modes" builds a single mode. The resolver's default context replaces the plugin's default tokens, as `tokens` of a project does. The document must carry its token sets inline: it is read without files, so a `$ref` to a file is reported (`W704`) and skipped. A document larger than one million characters is refused, and everything else the loader finds wrong is listed as a note (`W705`) while the rest still loads. A context the Figma plan has no mode for is skipped and noted after the build.

All the conversion lives in `@weft/figma`. The plugin has two halves, because Figma's main thread has no WebAssembly and the Weft core is WebAssembly (see "Where the code runs" in `packages/figma/README.md`):

- `src/code.ts` → `dist/code.js`: the main thread. It builds and reads layers. The core's WebAssembly loader is replaced by a stub that fails with a reason if anything reaches it.
- `src/ui.ts` → `dist/ui.html`: the UI iframe. It parses and serializes with the core. The module's bytes are inlined into the page, because Figma loads the UI as one HTML string with nothing beside it.

The build, the stub, the inlining and the UI page are shared with the Penpot plugin and live in `@weft/design-plugin`. This plugin passes its entries and how Figma carries messages (`pluginMessage`).

## Build and run

```sh
mise exec -- moon run figma-plugin:build
```

This builds the WebAssembly core first, then writes `dist/code.js` and `dist/ui.html`. To load the plugin in the Figma desktop app, go to Plugins → Development → Import plugin from manifest and pick `plugins/figma/manifest.json`.

The `id` in `manifest.json` is a development placeholder. Figma assigns the real id when the plugin is published.

The plugin needs no network access. The default token set (`packages/catalog/tokens/default.tokens.json`) is inlined into the UI.

`test/bundle.test.ts` builds both halves and runs them through `test/harness.ts`, which runs the UI with `@weft/design-plugin`'s shared UI harness:

- `code.js` runs in a VM context with `WebAssembly` and `fetch` removed.
- The UI script runs without `process` or `fetch`.
- The two exchange structured-cloned messages over the fake file of `@weft/figma`'s tests.

This shows the split works. It does not replace a run in the real Figma app.
