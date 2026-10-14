# Weft plugin for cox

A [cox](https://github.com/pyrlyn/cox) plugin that gives the cox agent the Weft agent tools: the ones the MCP server (`packages/mcp`) gives Claude Code, Cursor and Open Design. The tools come from the same Rust code (`crates/weft-binding`, module `tools`), so their descriptions, limits, checks and results are the same in every host. The plugin is a WebAssembly module (`wasm32-unknown-unknown`, an extism guest) built with cox's Rust SDK, `cox-plugin-sdk`.

All nine tools are served: `weft_primer`, `weft_capabilities`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render` and `weft_context`, named `wasm__weft__<tool>` in cox.

A cox tool result is one text, so the MCP server's content blocks are joined a blank line apart. `weft_render`'s accessibility tree comes from the same Rust code in both hosts. With `html: true` the page differs: the MCP server draws it with the React reference renderer (`@weft/render-react`), and the plugin, which has no JavaScript, with the static generator (`crates/weft-web`), and its description says so. The static page uses the project's tokens, or the default tokens without them. Markup that names a token those tokens do not define gets the tree and the page's diagnostics, as an error. When the project's token resolver has a light and a dark context, the page follows the system appearance.

cox finds plugin tools through `tool_search`, so the agent loads them when a task needs them. Every tool only reads its arguments (`read_only`); besides the canvas below, the plugin asks cox for no other capability: no files, network, store or model.

The plugin also ships the `weft-spec` skill (`skills/weft-spec`): the Claude Code and Cursor plugins' authoring guide, `AGENT-SPEC.md`, with a `SKILL.md` that names cox's tools. cox lists it in its skill index and loads it with the `skill` tool; a user or project skill named `weft-spec` wins. `plugins/shared/build.ts` writes the `AGENT-SPEC.md` copy (`moon run shared:build`), and `shared:test` fails when it is stale. The Claude Code and Cursor plugins' other skills (`render`, `import`, `export`, `figma-pull`) run Node scripts, which a cox package cannot carry, so they are left out.

## Design tab

In the cox desktop app the plugin fills the canvas pane (⌘⇧D, or "Show Weft" in the palette): one frame per screen its tools saw, 390 pt wide, drawn by the static generator as `weft_render` with `html` draws it, with each element's diagnostic path in `data-weft-path`. A frame shows the last markup `weft_validate`, `weft_format` or `weft_render` was given for that screen id, or the markup `weft_patch` returned, with the `data` and `project` of the last call that gave them; cox redraws after each of the plugin's tools. Markup that does not draw leaves the last page that did. At most 12 screens show; a new one replaces the one left alone longest.

A click on an element adds an attachment to the composer: the screen id, the element's path, and its markup line when the path names an element of the markup as written. It reaches the agent only when you send it with your next message.

The pane needs its own grant line, `desktop.canvas`, which cox asks for when the plugin is installed or updated. cox shows the pages in an isolated web view: no page script, no network, no navigation. The terminal has no canvas; the tools' results are the same there.

## Build and install

```sh
mise exec -- moon run cox-plugin:build   # writes plugins/cox/weft.wasm
cox plugin install plugins/cox
```

The first build fetches pyrlyn/cox for the SDK (a git dependency pinned to a commit on cox's unmerged `t33.47-plugin-canvas` branch until it reaches cox main: about 560 MB of history in `~/.cargo/git`). The module is about 1.4 MB.

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

- `src/lib.rs` and `src/canvas.rs`: the tool specs, calls, the config table and the canvas, on the host.
- `tests/manifest.rs`: `plugin.toml` read into cox's own manifest type (the type cox's `docs/plugin.schema.json` is generated from) and checked by its `validate`, at the pinned cox revision, and each declared skill folder holding a `SKILL.md` named for it.
- `tests/e2e.rs`: `weft.wasm` loaded with `wasm-plugin-host`, the extism engine of cox's plugin host, through `cox_init`, `cox_tool_call`, `cox_canvas` and `cox_canvas_select`, with results equal to the native build's.

The plugin is a Cargo workspace of its own, so the root workspace's Rust checks never fetch pyrlyn/cox or build guest code.
