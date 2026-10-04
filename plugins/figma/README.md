# Weft Figma plugin

A Figma plugin with two actions:

- **Build:** paste a `.weft` screen or open one, and the plugin builds it as frames and library instances.
- **Export:** select a frame and export it back as `.weft`. The plugin lists any losses along with the markup.

All the conversion lives in `@weft/figma`. `src/code.ts` only connects the `figma` global to it.

## Build and run

```sh
mise exec -- pnpm --filter @weft/figma-plugin build
```

This writes `dist/code.js`. To load the plugin in the Figma desktop app, go to Plugins → Development → Import plugin from manifest and pick `plugins/figma/manifest.json`.

The `id` in `manifest.json` is a development placeholder. Figma assigns the real id when the plugin is published.

The plugin needs no network access. The default token set (`packages/catalog/tokens/default.tokens.json`) is inlined into the bundle.
