# Weft plugin for cox

A [cox](https://github.com/pyrlyn/cox) plugin that gives the cox agent the Weft agent tools: the ones the MCP server (`packages/mcp`) gives Claude Code, Cursor and Open Design. The tools come from the same Rust code (`crates/weft-binding`, module `tools`), so their descriptions, limits, checks and results are the same in every host. The plugin is a WebAssembly module (`wasm32-unknown-unknown`, an extism guest) built with cox's Rust SDK, `cox-plugin-sdk`.

All nine tools are served: `weft_primer`, `weft_capabilities`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render` and `weft_context`, named `wasm__weft__<tool>` in cox.

A cox tool result is one text, so the MCP server's content blocks are joined a blank line apart. `weft_render`'s accessibility tree comes from the same Rust code in both hosts. With `html: true` the page differs: the MCP server draws it with the React reference renderer (`@weft/render-react`), and the plugin, which has no JavaScript, with the static generator (`crates/weft-web`), and its description says so. The static page uses the project's tokens, or the default tokens without them. Markup that names a token those tokens do not define gets the tree and the page's diagnostics, as an error. A token resolver's light and dark contexts are not applied to the page yet.

cox finds plugin tools through `tool_search`, so the agent loads them when a task needs them. Every tool only reads its arguments (`read_only`); the plugin asks cox for no other capability: no files, network, store or model.

## Build and install

```sh
mise exec -- moon run cox-plugin:build   # writes plugins/cox/weft.wasm
cox plugin install plugins/cox
```

The first build fetches pyrlyn/cox for the SDK (a git dependency pinned to a commit: about 560 MB of history in `~/.cargo/git`). The module is about 1.4 MB.

## Project

Without configuration the tools check against the core catalog only. To give them a project, add a `[plugins.weft]` table to cox's config. It is a `weft.json` (SPEC §10.6) with every file replaced by its content, as in the tools' `project` argument, because the plugin has no file access:

```toml
[plugins.weft]
actions = ["save", "cancel"]
validate = { mode = "strict" }
mcp = { context = "read-only", limits = { markupChars = 100000 } }

[plugins.weft.data]
type = "object"
properties = { user = { type = "object", properties = { name = { type = "string" } } } }
```

`catalog`, `tokens` and `fragments` take the same inline content. The tool settings are the `weft.json` keys the MCP server reads: `validate.mode`, `mcp.limits` and `mcp.context`. A table with errors stops the plugin at session start with the diagnostics; warnings show as cox notices.

## Tests

`moon run cox-plugin:test` builds the module, then runs clippy and rustfmt for both targets and three test sets:

- `src/lib.rs`: the tool specs, calls and the config table, on the host.
- `tests/manifest.rs`: `plugin.toml` read into cox's own manifest type (the type cox's `docs/plugin.schema.json` is generated from) and checked by its `validate`, at the pinned cox revision.
- `tests/e2e.rs`: `weft.wasm` loaded with `wasm-plugin-host`, the extism engine of cox's plugin host, through `cox_init` and `cox_tool_call`, with results equal to the native build's.

The plugin is a Cargo workspace of its own, so the root workspace's Rust checks never fetch pyrlyn/cox or build guest code.
