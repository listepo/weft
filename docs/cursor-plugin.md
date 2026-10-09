# The Cursor plugin

The `weft` plugin for Cursor does what the [Claude Code plugin](claude-code-plugin.md) does, with the same scripts and the same [MCP server](mcp.md): four skills you can type, one skill the agent loads on its own, and a rule that is attached whenever a `.weft` file is in context.

| Skill | What it does |
| --- | --- |
| `/weft-import <page.html> [out.weft]` | Turns an HTML file into a Weft screen and prints what the import lost. |
| `/weft-export <screen.weft> [out.jsx] [--name Component]` | Turns a Weft screen into a React component. |
| `/weft-render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]` | Turns a screen into an HTML page and opens it in Cursor's built-in browser. |
| `/weft-figma-pull <figma link> [out.weft]` | Reads a frame of a Figma file through the Figma REST API back into a Weft screen and prints what was lost. The access token comes from the `FIGMA_TOKEN` environment variable. |
| `weft-spec` (not typed) | Teaches the agent the format from [AGENT-SPEC.md](../AGENT-SPEC.md), so it writes, patches and repairs valid markup. |

The MCP server gives the agent `weft_primer`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render` and `weft_context`. The skills read and write files through small scripts; the server never touches files. The scripts are the ones described in [the Claude Code page](claude-code-plugin.md#what-the-commands-run): they never replace a file unless you pass `--force`, and exit with 0 (done), 1 (the input has errors) or 2 (usage or file problem).

## Install

The packages run from this repository's sources, so install the plugin from a clone. First make the clone ready, as in the [tour](tour.md): `mise install`, `pnpm install`, `moon run root:wasm`. `node` 26.7 or later must be on the `PATH` that Cursor starts programs with. Then link the plugin folder into Cursor's local plugins and restart Cursor, or run **Developer: Reload Window**:

```bash
ln -s /path/to/weft/plugins/cursor ~/.cursor/plugins/local/weft
```

Cursor copies only the plugin folder into its cache, so the folder carries what it runs: the bundled scripts and server in `dist/`, with the WebAssembly core. They are built once, from `plugins/shared`, for this plugin and the Claude Code plugin together (`moon run shared:build`).

## Limits

- Installing from a local clone only; the plugin is not listed in a marketplace you can add by name, and the repository has no GitHub address to import from yet.
- It has been checked by tests against Cursor's published plugin format and from a copy of the folder, but not yet inside a signed-in Cursor; the [plugin's README](../plugins/cursor/README.md) lists what remains.
- `/weft-render` opens the page in the built-in browser. If that browser refuses a `file://` address, the agent serves the folder on localhost instead.
