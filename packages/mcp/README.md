# @weft/mcp

An MCP server that lets an agent read, check and edit Weft documents (see `SPEC.md`). It speaks stdio and works only on markup passed in tool arguments: its tools read no files, and it opens no network connections. The only file it reads is the project file its host names at start.

## Run and register

Node 26.7 or newer runs the TypeScript source directly:

```sh
node packages/mcp/src/server.ts
```

Register it in an MCP client with an absolute path to the checkout:

```json
{
  "mcpServers": {
    "weft": {
      "command": "node",
      "args": ["/path/to/weft/packages/mcp/src/server.ts"]
    }
  }
}
```

For Claude Code: `claude mcp add weft -- node /path/to/weft/packages/mcp/src/server.ts`. The package also declares a `weft-mcp` bin.

### With a project

`--project /path/to/weft.json` starts the server with a project file (SPEC section 10). Its catalog, tokens, actions and data schema become the server's, and two of its settings (SPEC section 10.6) apply:

- `validate.mode`: the default of `weft_validate`'s `strict` (`"strict"` or `"lenient"`, default lenient). The argument still wins. `weft_patch` and `weft_render` always check strictly.
- `mcp.limits`: any of `markupChars`, `dataChars`, `patches`, `patchesChars`, `projectChars`, `diagnostics`, `inputElements`, each a whole number of at least 1. Unset limits keep the defaults below.

Project problems go to stderr. A project with errors, or a file that cannot be read, stops the server before it serves (exit 1 or 2). The server never looks for a project file itself: it has no screen to look from.

## Tools

| Tool | Input | Result |
| --- | --- | --- |
| `weft_primer` | none | A short primer: syntax rules, value forms, patch forms, how to use the other tools. Read first. |
| `weft_catalog` | `kind?`, `project?` | Without `kind`, one line per component (`kind \| role \| content \| description`). With `kind`, that component's full definition as JSON. |
| `weft_validate` | `markup`, `strict?`, `project?` | `{"valid":boolean,"diagnostics":[…]}`. `strict` rejects unknown elements and attributes. |
| `weft_format` | `markup`, `project?` | The canonical markup, plus a second block with warnings if there are any. Diagnostics and `isError` if the markup has errors. |
| `weft_patch` | `markup`, `patches`, `project?` | The new canonical markup (SPEC section 7), or diagnostics and `isError` with nothing applied. The result is validated strictly. |
| `weft_render` | `markup`, `data?`, `html?`, `project?` | The accessibility tree the reference renderer (`@weft/render-react`) produces, as YAML in the style of a Playwright aria snapshot: what a user of assistive technology or a browsing agent gets, without a browser. `data` is the sample data model the bindings read. With `html: true`, a second block holds the static HTML page. Diagnostics and `isError` if the markup is not strictly valid. |

Diagnostics are the objects of SPEC section 6.1. A patch problem has a `W5xx` code and a path into the patch list such as `#/patches/2/parent`.

`project` is the content of a project file (SPEC section 10): `weft.json` with every file name replaced by that file's JSON content. For that call it replaces the host's catalog, tokens, actions and data schema, and bindings are checked against the data schema. Its problems have `W7xx` codes and paths that start at `#/project`. `weft_validate` lists them first; the other tools fail with them when the project has errors.

## Limits

Tool inputs are untrusted. They are checked against their schema before a tool runs, and every failure comes back as a tool result with `isError: true`; no tool throws.

- `markup`: at most 200,000 characters.
- `patches`: at most 100 patches and 200,000 characters of JSON per call.
- `data` (`weft_render`): at most 200,000 characters of JSON.
- `project`: at most 500,000 characters of JSON.
- Arguments: at most 20,000 JSON values.
- Results list at most 40 diagnostics and report how many were left out.
- `kind`: at most 100 characters.

These are the defaults, in `LIMITS` in `src/context.ts`; the host changes them with `mcp.limits` (above) or `createServer`. A `project` tool argument never does: its settings are ignored, so a model cannot lift its own bounds.

## Host configuration

`createServer({ catalog, tokens, actions, data })` takes the catalog (default `weft-core`), the known design tokens (a map of token path to DTCG `$type`), the known action names and a data schema from `compileDataSchema` of `@weft/core`. Token references, action names and bindings are checked only when the host supplies them; a `project` argument replaces all four. The stdio entry point supplies them only with `--project`.

A second argument, `{ mode, limits }`, sets the server's own settings: the default of `weft_validate`'s `strict` and the limits to change. `hostOptions(project)` turns a loaded project into both arguments.

## Adding a tool

A tool is one function `(server, context, settings) => void` in `src/tools/`, listed in `src/tools/index.ts`. `weft_render` is such a file; it renders with the host's catalog from `Context` and bounds its input with `settings.limits`. A new limit or option gets a key in `weft.json` in the same change (SPEC section 10.6).
