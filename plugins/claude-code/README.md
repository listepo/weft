# Weft plugin for Claude Code

Works on `.weft` files from Claude Code, including Claude Code Desktop.

| Skill | What it does |
| --- | --- |
| `/weft:import <page.html> [out.weft]` | HTML to a Weft screen through `fromDom` of `@weft/from-aria`; prints the loss table. |
| `/weft:export <screen.weft> [out.jsx] [--name Component]` | A Weft screen to a React component through `@weft/to-jsx`. |
| `/weft:render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]` | A Weft screen to a static HTML page through `renderPage` of `@weft/render-react`, then opened in the Desktop built-in browser. Without `--tokens` the catalog's default tokens apply. |
| `/weft:figma-pull <figma link> [out.weft]` | A frame of a Figma file back to a Weft screen through `pullScreen` of `@weft/figma/pull` (the Figma REST API); prints the loss table. The token comes from `FIGMA_TOKEN` only. |
| `weft:spec` | Teaches `AGENT-SPEC.md` (a generated copy) so Claude writes, patches and repairs valid markup. Claude loads it on its own. |

The plugin also registers the `weft` MCP server (`@weft/mcp`: `weft_primer`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render`, `weft_context`). The server never touches files; reading and writing them belongs to the four skills' scripts (sources in `plugins/shared/scripts/`, shared with the Cursor plugin and bundled into `dist/`). A script refuses to replace an existing file unless `--force` is given. Exit codes: 0 done, 1 the input or its project has errors (printed as `file:line:col code message`, or `weft.json:#/pointer code message`), 2 usage or file problem.

Each script works in the project of its input (SPEC section 10; figma-pull, which has no input file, in the project of the working directory): the first `weft.json` in the input's folder or above it, another one with `--project <file>`, or none with `--no-project`. The project brings the catalog, tokens, actions and data schema, and its settings (SPEC section 10.6) stand in for arguments that are not given: `render.data`, `render.tokens` and `render.outDir` for render, `export.react.outDir`, `typescript` and `source` for export, `import.html.outDir` for import, `import.figma.outDir` for figma-pull. Arguments always win.

## Install

```
/plugin marketplace add /path/to/weft
/plugin install weft@weft
```

Claude Code copies only `plugins/claude-code` into its plugin cache, without the workspace packages or `node_modules`, so the folder carries everything it runs: the four scripts and the MCP server are bundled into `dist/` (`import.js`, `export.js`, `render.js`, `figma-pull.js`, `server.js`, and shared files in `dist/chunks/`) together with the WebAssembly core they load from `dist/wasm-web/weft_bg.wasm`, with `AGENT-SPEC.md` copied into `skills/spec/`. Only Node built-ins are imported at run time. Installing from a GitHub-hosted marketplace (`/plugin marketplace add <owner>/weft`) therefore needs nothing but the repository. It has not been tried yet, because the repository has no remote; `repository` goes into `plugin.json` once it has one.

### Node

Every script and the MCP server start with `node` from the `PATH`, so `node` 24.2 or later must be on the `PATH` that Claude Code starts them with (the bundles use `import.meta.main`, new in Node 24.2; on older Node they print that requirement and exit 2; checked on Node 24.18 and 26.7). Claude Code resolves a stdio MCP server's `command` through the `PATH`, and the docs name no setting that pins a runtime for a plugin. In Claude Code Desktop on macOS, started from the Dock or Finder, the app reads the shell profile (such as `~/.zshrc`) to extract `PATH`, so put `node` on the `PATH` there. A version manager that sets `PATH` only from an interactive prompt hook (`mise activate`, a lazy `nvm`) may not be seen: add its shims directory (`~/.local/share/mise/shims` for mise) to the profile's `PATH`, or install Node system-wide. If `weft` shows as failed in `/mcp`, `node` is not found or too old. Sources, checked 2026-10-05: <https://code.claude.com/docs/en/desktop#local-sessions>, <https://code.claude.com/docs/en/desktop#session-not-finding-installed-tools>, <https://code.claude.com/docs/en/mcp>.

### Rebuild

`dist/` and `skills/spec/AGENT-SPEC.md` are generated and committed, because Claude Code installs from git. After a change to `plugins/shared/scripts/`, `@weft/*` or `AGENT-SPEC.md`, run `moon run shared:build` (one build writes `dist/` and the spec copy of this plugin, of `plugins/cursor` and of `plugins/open-design`; it builds the WebAssembly core first with `root:wasm`, then `node build.ts`, Vite's rolldown bundler); a test rebuilds into a temporary folder and fails while the committed bundles differ. The `.wasm` is compared byte for byte too: `root:wasm` remaps the cargo, rustup and workspace paths (`/cargo`, `/rustup`, `/weft`), so the same sources build to the same bytes on any machine. Other tests copy only the plugin folder outside the repository and run the scripts and the MCP server from there.

## Format notes

Checked against the official Claude Code documentation on 2026-10-05 (Claude Code 2.1.267):

- Manifest: `.claude-plugin/plugin.json`, only `name` is required; components are found in their default folders. <https://code.claude.com/docs/en/plugins-reference>
- Skills: `skills/<name>/SKILL.md`; the docs prefer them to `commands/` for new plugins, and a plugin skill is invoked as `/<plugin>:<skill>`. `${CLAUDE_PLUGIN_ROOT}` is substituted in the skill body. <https://code.claude.com/docs/en/skills>
- MCP servers: `.mcp.json` at the plugin root, with `${CLAUDE_PLUGIN_ROOT}` in `command` and `args`. <https://code.claude.com/docs/en/plugins-reference#mcpservers>
- Marketplace: `.claude-plugin/marketplace.json` at the repository root with `name`, `owner` and `plugins`; a plugin in the same repository uses a relative `source` such as `./plugins/claude-code`. <https://code.claude.com/docs/en/plugin-marketplaces>, <https://code.claude.com/docs/en/plugins/marketplace-reference>
- Loading: a marketplace added from a local path loads relative-path plugins in place; every other install copies only the plugin folder into the cache, and installs Node dependencies only from an npm or Bun lockfile (this plugin has none: nothing is needed at run time). <https://code.claude.com/docs/en/plugins/loading>
- `claude plugin validate plugins/claude-code --strict` and `claude plugin validate . --strict` both pass.
