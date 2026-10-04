# Weft plugin for Claude Code

Works on `.weft` files from Claude Code, including Claude Code Desktop.

| Skill | What it does |
| --- | --- |
| `/weft:import <page.html> [out.weft]` | HTML to a Weft screen through `fromDom` of `@weft/from-aria`; prints the loss table. |
| `/weft:export <screen.weft> [out.jsx] [--name Component]` | A Weft screen to a React component through `@weft/to-jsx`. |
| `/weft:render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]` | A Weft screen to a static HTML page through `renderPage` of `@weft/render-react`, then opened in the Desktop built-in browser. Without `--tokens` the catalog's default tokens apply. |
| `weft:spec` | Teaches `AGENT-SPEC.md` (linked, not copied) so Claude writes, patches and repairs valid markup. Claude loads it on its own. |

The plugin also registers the `weft` MCP server (`@weft/mcp`: `weft_primer`, `weft_catalog`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render`). The server never touches files; reading and writing them belongs to the three skills' scripts in `scripts/`. A script refuses to replace an existing file unless `--force` is given. Exit codes: 0 done, 1 the input has errors (printed as `file:line:col code message`), 2 usage or file problem.

## Install

The packages are private and run from this repository's sources, so install the plugin from a clone, with the dependencies installed and the WebAssembly core built (`mise install`, `pnpm install`, `moon run root:wasm`):

```
/plugin marketplace add /path/to/weft
/plugin install weft@weft
```

A marketplace added from a local path loads the plugin in place, so `${CLAUDE_PLUGIN_ROOT}` is `plugins/claude-code` of the clone and the workspace packages resolve through its `node_modules`. `node` 26.7 or later must be on the `PATH` that Claude Code starts the MCP server and the scripts with. Installing from a GitHub-hosted marketplace would copy only `plugins/claude-code` into the plugin cache, without the packages or their dependencies, so it does not work yet.

## Format notes

Checked against the official Claude Code documentation on 2026-10-05 (Claude Code 2.1.267):

- Manifest: `.claude-plugin/plugin.json`, only `name` is required; components are found in their default folders. <https://code.claude.com/docs/en/plugins-reference>
- Skills: `skills/<name>/SKILL.md`; the docs prefer them to `commands/` for new plugins, and a plugin skill is invoked as `/<plugin>:<skill>`. `${CLAUDE_PLUGIN_ROOT}` is substituted in the skill body. <https://code.claude.com/docs/en/skills>
- MCP servers: `.mcp.json` at the plugin root, with `${CLAUDE_PLUGIN_ROOT}` in `command` and `args`. <https://code.claude.com/docs/en/plugins-reference#mcpservers>
- Marketplace: `.claude-plugin/marketplace.json` at the repository root with `name`, `owner` and `plugins`; a plugin in the same repository uses a relative `source` such as `./plugins/claude-code`. <https://code.claude.com/docs/en/plugin-marketplaces>, <https://code.claude.com/docs/en/plugins/marketplace-reference>
- Loading: a marketplace added from a local path loads relative-path plugins in place; every other install copies only the plugin folder into the cache. <https://code.claude.com/docs/en/plugins/loading>
- `claude plugin validate plugins/claude-code --strict` and `claude plugin validate . --strict` both pass.
