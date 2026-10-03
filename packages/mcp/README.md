# @weft/mcp

An MCP server that lets an agent read, check and edit Weft documents (see `SPEC.md`). It speaks stdio and works only on markup passed in tool arguments: it reads no files and opens no network connections.

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

## Tools

| Tool | Input | Result |
| --- | --- | --- |
| `weft_primer` | none | A short primer: syntax rules, value forms, patch forms, how to use the other tools. Read first. |
| `weft_catalog` | `kind?` | Without `kind`, one line per component (`kind \| role \| content \| description`). With `kind`, that component's full definition as JSON. |
| `weft_validate` | `markup`, `strict?` | `{"valid":boolean,"diagnostics":[…]}`. `strict` rejects unknown elements and attributes. |
| `weft_format` | `markup` | The canonical markup, plus a second block with warnings if there are any. Diagnostics and `isError` if the markup has errors. |
| `weft_patch` | `markup`, `patches` | The new canonical markup (SPEC section 7), or diagnostics and `isError` with nothing applied. The result is validated strictly. |
| `weft_render` | `markup`, `data?`, `html?` | The accessibility tree the reference renderer (`@weft/render-react`) produces, as YAML in the style of a Playwright aria snapshot: what a user of assistive technology or a browsing agent gets, without a browser. `data` is the sample data model the bindings read. With `html: true`, a second block holds the static HTML page. Diagnostics and `isError` if the markup is not strictly valid. |

Diagnostics are the objects of SPEC section 6.1. A patch problem has a `W5xx` code and a path into the patch list such as `#/patches/2/parent`.

## Limits

Tool inputs are untrusted. They are checked against their schema before a tool runs, and every failure comes back as a tool result with `isError: true`; no tool throws.

- `markup`: at most 200,000 characters.
- `patches`: at most 100 patches and 200,000 characters of JSON per call.
- `data` (`weft_render`): at most 200,000 characters of JSON.
- Arguments: at most 20,000 JSON values.
- Results list at most 40 diagnostics and report how many were left out.
- `kind`: at most 100 characters.

The numbers live in `LIMITS` in `src/context.ts`.

## Host configuration

`createServer({ catalog, tokens, actions })` takes the catalog (default `weft-core`), the known design tokens (a map of token path to DTCG `$type`) and the known action names. Token references and action names are checked only when the host supplies them. The stdio entry point supplies neither.

## Adding a tool

A tool is one function `(server, context) => void` in `src/tools/`, listed in `src/tools/index.ts`. `weft_render` is such a file; it renders with the host's catalog from `Context`.
