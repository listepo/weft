# Weft Penpot plugin

A Penpot plugin with two actions:

- **Build:** paste a `.weft` screen or open one, and the plugin builds it as boards and component copies. The first build also creates the Weft component library and design tokens in the file.
- **Export:** select a board and export it back as `.weft`. The plugin lists any losses along with the markup.

All the conversion lives in `@weft/penpot`. The plugin has two halves, because Penpot runs plugin code in an SES compartment without WebAssembly and the Weft core is WebAssembly (see "Where the code runs" in `packages/penpot/README.md`):

- `src/plugin.ts` → `dist/plugin.js`: the sandbox. It builds and reads shapes, selects and zooms to a built board, and opens the UI.
- `src/ui.ts` → `dist/ui.html`: the UI iframe. It parses and serializes with the core, which is inlined into the page with its module bytes. It accepts messages only from its parent window, which is Penpot.

The build and the UI page are shared with the Figma plugin and live in `@weft/design-plugin`.

## Build and run

```sh
mise exec -- moon run penpot-plugin:build
```

This builds the WebAssembly core first, then writes `dist/manifest.json`, `dist/plugin.js` and `dist/ui.html`. Penpot loads a plugin from a URL and plugins are hosted outside Penpot, so `dist/` is not committed: it is served. The manifest uses version 2, so `plugin.js` and `ui.html` are loaded from the manifest's folder.

```sh
mise exec -- moon run penpot-plugin:serve
```

This builds, then serves `dist/` on `http://localhost:4400` with `Access-Control-Allow-Origin: *` (`serve.config.ts`), because Penpot fetches the files from its own origin and Vite's default allows only localhost origins. Install the plugin in Penpot's plugin manager with `http://localhost:4400/manifest.json`. Whether a real Penpot needs the CORS header has not been checked; the server sends it either way. A hosted copy for users is T41 in `roadmap.md`.

The plugin asks for `content:write` and `library:write`, which include the read permissions. It needs no network access: the default token set (`packages/catalog/tokens/default.tokens.json`) is inlined into the UI.

## Tests

`test/bundle.test.ts` builds both halves and runs them through `test/harness.ts`:

- `plugin.js` runs the way Penpot runs it: SES (`ses`, the library Penpot's runtime uses) is loaded into a fresh VM realm, `lockdown()` hardens it, and the script is evaluated in a `Compartment` whose globals are Penpot's endowments over the fake Penpot of `@weft/penpot`'s tests. The test checks that `WebAssembly` is not there.
- The UI script runs through `@weft/design-plugin`'s shared UI harness, without `process` or `fetch`.
- The two exchange structured-cloned messages the way Penpot carries them: `parent.postMessage` to `penpot.ui.onMessage`, and `penpot.ui.sendMessage` to a `message` event.

This shows the split works. It does not replace a run in a real Penpot.
