# Weft guide

This guide is for people who use Weft: developers who wire it into an app or an agent, and designers who care about what ends up on the screen. It explains what Weft is, shows how to use every tool that ships with it, and tells you the truth about what is not finished. It is not the specification ([SPEC.md](../SPEC.md)) and it is not the guide for models ([AGENT-SPEC.md](../AGENT-SPEC.md)); it links to both where they have the details.

## Reading order

1. **[What is Weft](what-is-weft.md).** The problem, the idea, how it compares with HTML, JSX and A2UI, and the honest limits. Five minutes.
2. **[A ten-minute tour](tour.md).** One screen, hands on: validate, break and fix, preview, patch, read the change back, export and import.
3. **[How it works](how-it-works.md).** How the Rust core, WebAssembly, the TypeScript packages and the tools fit together, with a diagram.

## One page per tool

| Page | Use it to |
| --- | --- |
| [The `weft` command](cli.md) | Validate, format and explain screens from a terminal. |
| [The MCP server](mcp.md) | Give an AI agent tools to read, check, edit and preview screens. |
| [The Claude Code plugin](claude-code-plugin.md) | Import, export and render from Claude Code, with the agent guide built in. |
| [The Cursor plugin](cursor-plugin.md) | The same from Cursor: skills, a rule and the MCP server. |
| [The Open Design plugin](open-design-plugin.md) | The same from Open Design, and design systems (`DESIGN.md`, `tokens.css`) as Weft tokens. |
| [Weft in Xcode](xcode-plugin.md) | SwiftUI generated from `.weft` files at build time, convert commands, an Editor menu extension, and the MCP server for Xcode's agents. |
| [Rendering](rendering.md) | Turn a screen into an HTML page, an accessibility tree or a React tree. |
| [Importing HTML](importing.md) | Start a screen from an existing page and see what the import lost. |
| [Exporting to React](exporting-jsx.md) | Turn a screen into a React component. |
| [Catalog and tokens](catalog-and-tokens.md) | Understand and extend the vocabulary and the design values. |
| [Patches](patches.md) | Change a screen by element id, atomically. |
| [Projects](projects.md) | Share tokens, components, actions, a data schema and tool settings across screens with `weft.json`. |
| [Contributing](contributing.md) | Build, test and change Weft. |

## How to read the commands

Commands are written to be run from the repository root, in the order of the tour. A line that starts with `$` is a command, and the lines under it are its real output, copied from a run. Where a command ends with `; echo "exit $?"`, the guide is showing the exit code too: 0 means success, 1 means the input has errors, 2 means the command was used wrongly or a file could not be read.

- The tour builds the tools and the scratch folder `weft-tour/` that the other pages reuse. Each page repeats the two lines that recreate the folder, so you can start anywhere after the setup.
- Paths of your own clone appear as `/path/to/weft` in the output shown.
- `weft` means the program built from `crates/weft-cli`; the tour shows how to put it on your `PATH`.
- Some examples call the libraries (`@weft/core`, `@weft/catalog`, …). They are workspace packages with no build step, and Node finds them only from a file inside a package folder that depends on them. The guide puts such scripts in `plugins/shared/scratch/`, which is safe to delete.
- Slash commands (`/weft:import` and the others) and `/plugin` run inside Claude Code and cannot be shown here. The scripts they run are shown instead.

## Status

Weft is a prototype at format version 0.1. Every page says what it cannot do yet.
