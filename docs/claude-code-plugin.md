# The Claude Code plugin

The `weft` plugin makes Claude Code, including Claude Code Desktop, good at Weft files. It adds three commands you can type, one skill Claude loads on its own, and the [MCP server](mcp.md).

| Command | What it does |
| --- | --- |
| `/weft:import <page.html> [out.weft]` | Turns an HTML file into a Weft screen and prints what the import lost. |
| `/weft:export <screen.weft> [out.jsx] [--name Component]` | Turns a Weft screen into a React component. |
| `/weft:render <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]` | Turns a screen into an HTML page and opens it in the browser pane of Claude Code Desktop. |
| `weft:spec` (a skill, not a command) | Teaches Claude the format from [AGENT-SPEC.md](../AGENT-SPEC.md), so it writes, patches and repairs valid markup. Claude loads it whenever you ask for work on a `.weft` file. |

The plugin's MCP server gives Claude `weft_primer`, `weft_catalog`, `weft_schema`, `weft_validate`, `weft_format`, `weft_patch` and `weft_render`. The three commands read and write files through small scripts; the server never touches files.

## Install

The packages run from this repository's sources, so install the plugin from a clone. First make the clone ready, as in the [tour](tour.md): `mise install`, `pnpm install`, `moon run root:wasm`. `node` 26.7 or later must be on the `PATH` that Claude Code starts programs with.

Then, inside Claude Code:

```text
/plugin marketplace add /path/to/weft
/plugin install weft@weft
```

A marketplace added from a local folder loads the plugin in place, so it keeps using your clone. To try the plugin for one session without installing it, start Claude Code with the folder instead:

```bash
claude --plugin-dir /path/to/weft/plugins/claude-code
```

To check the plugin and the marketplace files without starting a session:

```console
$ claude plugin validate plugins/claude-code --strict
Validating plugin manifest: /path/to/weft/plugins/claude-code/.claude-plugin/plugin.json

✔ Validation passed
$ claude plugin validate . --strict
Validating marketplace manifest: /path/to/weft/.claude-plugin/marketplace.json

✔ Validation passed
```

Installing from a GitHub address is not possible yet: Claude Code copies only the plugin folder into its cache, and the Weft packages are not inside it.

## What the commands run

Each command is a script in `plugins/shared/scripts/`, so you can run the same thing from a terminal. The tour already used two of them. The scripts print what they did, never replace a file unless you pass `--force`, and use these exit codes:

| Code | Meaning |
| --- | --- |
| 0 | Done. |
| 1 | The input has errors; they are printed as `file:line:column code message`. Nothing was written. |
| 2 | A usage or file problem: a missing file, an input that is too large, an output that already exists. |

Each script works in the project of its input: the first `weft.json` in the input's folder or above it (`--project <file>` names another, `--no-project` ignores it). The project brings its own components, tokens, actions and data schema, and its settings stand in for options you leave out: `render.data`, `render.tokens` and `render.outDir` for render, `export.react.outDir` for export, `import.html.outDir` for import. A project with errors stops the script with exit code 1. See [Projects](projects.md).

The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
$ cp corpus/login/screen.html weft-tour/page.html
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/plugin.html --data corpus/login/data.json; echo "exit $?"
Wrote weft-tour/plugin.html
file:///path/to/weft/weft-tour/plugin.html
exit 0
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/plugin.html; echo "exit $?"
weft: weft-tour/plugin.html already exists; pass --force to replace it
exit 2
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/plugin.html --force; echo "exit $?"
Wrote weft-tour/plugin.html
file:///path/to/weft/weft-tour/plugin.html
exit 0
```

`/weft:render` runs the same script, then opens the `file://` address it prints in the Desktop browser pane. Where there is no browser pane (the terminal), Claude gives you the path to open yourself. A screen that is not valid is not rendered:

```console
$ sed 's/variant="primary"/variant="primry"/' weft-tour/login.weft > weft-tour/login-typo.weft
$ node plugins/shared/scripts/render.ts weft-tour/login-typo.weft; echo "exit $?"
weft-tour/login-typo.weft:8:61 W203 "primry" is not an allowed value. — did you mean "primary"?
exit 1
```

`/weft:export` and `/weft:import` are covered on their own pages: [Exporting to React](exporting-jsx.md) and [Importing HTML](importing.md).

## Working with Claude

You mostly do not type the commands. You say what you want ("add a Remember me checkbox below the password field of `login.weft`", "bring `pricing.html` into Weft and show me what was lost") and Claude uses the skill and tools. What to expect:

- Claude edits with `weft_patch` rather than rewriting the file, and validates strictly before it answers.
- After `/weft:render` in the Desktop app, Claude looks at the page before saying it is done.
- After an edit, ask it to read the changed bindings back (`weft explain --against` in the terminal does the same). the agent guide tells Claude to compare each changed binding with your instruction.

## Limits

- Installing from a local clone only; the plugin is not on a marketplace you can add by name.
- `/weft:render` previews in the Desktop browser pane. In a terminal-only session you open the file yourself.
- The importer takes an HTML file, not a running page. For a live page, save its HTML first.

## Coming

Installing the plugin straight from GitHub is in progress.
