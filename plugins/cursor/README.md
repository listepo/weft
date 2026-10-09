# Weft plugin for Cursor

Works on `.weft` files from Cursor: the same features as the [Claude Code plugin](../claude-code/README.md), from the same bundled scripts and MCP server.

| Skill (type `/` in Agent chat) | What it does |
| --- | --- |
| `/weft-import <page.html> [out.weft]` | HTML to a Weft screen through `fromDom` of `@weft/from-aria`; prints the loss table. |
| `/weft-export <screen.weft> [out.jsx] [--name Component]` | A Weft screen to a React component through `@weft/to-jsx`. |
| `/weft-render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]` | A Weft screen to a static HTML page through `renderPage` of `@weft/render-react`, then opened in Cursor's built-in browser. Without `--tokens` the catalog's default tokens apply. |
| `/weft-figma-pull <figma link> [out.weft]` | A frame of a Figma file back to a Weft screen through `pullScreen` of `@weft/figma/pull` (the Figma REST API); prints the loss table. The token comes from `FIGMA_TOKEN` only. |
| `weft-spec` | Teaches `AGENT-SPEC.md` (a generated copy) so the agent writes, patches and repairs valid markup. The agent loads it on its own. |

| Rule | What it does |
| --- | --- |
| `rules/weft-files.mdc` | Attached when a `.weft` file is in context: read the guide, use the `weft_*` tools, validate in strict mode before answering. |

The plugin also registers the `weft` MCP server (`@weft/mcp`: `weft_primer`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch`, `weft_render`, `weft_context`). The server never touches files; reading and writing them belongs to the four skills' scripts. A script refuses to replace an existing file unless `--force` is given. Exit codes: 0 done, 1 the input or its project has errors (printed as `file:line:col code message`, or `weft.json:#/pointer code message`), 2 usage or file problem.

Each script works in the project of its input (SPEC section 10; figma-pull, which has no input file, in the project of the working directory): the first `weft.json` in the input's folder or above it, another one with `--project <file>`, or none with `--no-project`. The project brings the catalog, tokens, actions and data schema, and its settings (SPEC section 10.6) stand in for arguments that are not given: `render.data`, `render.tokens` and `render.outDir` for render, `export.react.outDir`, `typescript` and `source` for export, `import.html.outDir` for import, `import.figma.outDir` for figma-pull. Arguments always win.

## Install

From a clone of this repository, as a local plugin:

```bash
ln -s "$PWD/plugins/cursor" ~/.cursor/plugins/local/weft
```

Then restart Cursor or run **Developer: Reload Window**. A real folder copied to `~/.cursor/plugins/local/weft` works the same. `node` 24.2 or later must be on the `PATH` that Cursor starts MCP servers and shell commands with (the bundles use `import.meta.main`, new in Node 24.2; on older Node they print that requirement and exit 2).

Once the repository has a GitHub remote, Cursor's **Import from Repo** (Dashboard, Plugins & MCPs) reads the root `.cursor-plugin/marketplace.json`, whose single entry has `source: plugins/cursor`. The repository has no remote yet, so this path is untested.

## One bundle, two plugins

Cursor extracts only the marketplace entry's `source` folder into `~/.cursor/plugins/cache/<marketplace>/<plugin>/<version>/` (checked 2026-10-05 against the cache of Cursor 3.23.12: a plugin from `third_party/outlook` of `cursor/plugins` holds only that folder's files). Nothing outside `plugins/cursor` is there at run time, so a link or a path into `plugins/claude-code` cannot work, and the folder must carry its own `dist/` and `AGENT-SPEC.md` copy, exactly as the Claude Code plugin does.

The sources exist once, in `plugins/shared` (the scripts, the build, the shared tests). `moon run shared:build` bundles them once and writes the identical `dist/` and the `AGENT-SPEC.md` copy into `plugins/claude-code`, `plugins/cursor` and `plugins/open-design`. `plugins/shared/test/bundle.test.ts` rebuilds into a temporary folder and fails while either plugin's `dist/` or spec copy differs, and runs every script and the MCP server from a copy of each plugin folder placed outside the repository. Never edit `dist/` by hand.

## Format notes

Checked on 2026-10-05 against Cursor's official documentation and the official plugin repository:

- A plugin is a folder with `.cursor-plugin/plugin.json` (only `name` is required; lowercase kebab-case). Components are found in default folders: `skills/<name>/SKILL.md`, `rules/` (`.mdc`), `commands/`, `agents/`, `hooks/hooks.json`, and `mcp.json` at the root; a path in the manifest replaces the folder's discovery. <https://cursor.com/docs/plugins>, <https://cursor.com/docs/reference/plugins>
- `mcp.json` has `mcpServers`; Cursor expands `${CURSOR_PLUGIN_ROOT}` (and `${CLAUDE_PLUGIN_ROOT}`) in `command`, `args`, `env` and `cwd`. The docs name no variable for skills or rules, so the skills reach the scripts by a path from their own folder (`../../dist/…`, the plugin root is two levels up). <https://cursor.com/docs/reference/plugins>
- Skills: frontmatter `name` (lowercase letters, digits and hyphens) and `description`; scripts are referenced by a path relative to the skill. Cursor has no plugin namespace for skills, hence the `weft-` prefix. <https://cursor.com/docs/skills>
- Rules: `.mdc` with `description`, `globs` and `alwaysApply`; a rule with a glob is attached when a matching file is in context; keep rules under 500 lines. <https://cursor.com/docs/context/rules>
- Marketplace: `.cursor-plugin/marketplace.json` at the repository root with `name`, `owner` and `plugins` (entries `name` and `source`, a path relative to the root). <https://cursor.com/docs/reference/plugins>
- Local plugins: `~/.cursor/plugins/local/<name>/`. <https://cursor.com/docs/plugins>
- Built-in browser: the agent drives it with a Navigate tool; the docs do not say whether `file://` URLs open, so `weft-render` falls back to a localhost server. <https://cursor.com/docs/agent/tools/browser>
- Validator: Cursor publishes no CLI. Its plugin repository ships the JSON schemas and a script (`scripts/validate-plugins.mjs`) that checks a marketplace and each plugin against them; `test/schemas/` holds those two schemas (MIT, <https://github.com/cursor/plugins> at commit `e43c7ee26e00`, 2026-10-04, reformatted with oxfmt, content unchanged; its `NOTICE` keeps the source, the copyright notice and the license text) and `test/plugin.test.ts` runs the same checks with Ajv: both manifests against the schemas, the entry's name against the manifest's name, the source folder and manifest exist.

## Not checked yet

These need a signed-in Cursor and have not been done:

- the plugin loads from `~/.cursor/plugins/local/weft` and lists the skills, the rule and the `weft` MCP server;
- an agent resolves `../../dist/…` from a skill's folder (it must know the skill's path);
- the `weft` server starts under Cursor's `PATH`;
- `/weft-render` opens its page in the built-in browser, by `file://` or through the localhost fallback;
- installing through **Import from Repo** from GitHub.
