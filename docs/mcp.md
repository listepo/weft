# The MCP server

`@weft/mcp` is a small server for the [Model Context Protocol](https://modelcontextprotocol.io) that gives an AI agent six tools to read, check, edit and preview Weft screens. It is the safest way to let a model work on a screen: every answer comes from the same validator you use yourself, and the agent never has to guess whether its markup is valid.

The server works only on text passed in tool calls. It reads no files, and opens no network connections. Reading and writing files is the agent's job, or the [plugin's scripts](claude-code-plugin.md).

## The tools

| Tool | Input | What you get back |
| --- | --- | --- |
| `weft_primer` | none | A short primer: the syntax rules, value forms, patch forms and how to use the other tools. An agent calls it first. |
| `weft_capabilities` | none | What the host accepts: `{"weft", "catalogs": [{name, version}]}`, plus the token paths and action names when the host checks them. |
| `weft_catalog` | `kind` (optional) | Without `kind`, one line per component. With `kind`, that component's full definition. |
| `weft_validate` | `markup`, `strict` (optional) | `{"valid": true/false, "diagnostics": […]}`. |
| `weft_format` | `markup` | The canonical markup, or the diagnostics when the markup has errors. |
| `weft_patch` | `markup`, `patches` | The patched screen in canonical form, or diagnostics and nothing applied. The result is checked strictly. |
| `weft_render` | `markup`, `data` (optional), `html` (optional) | The accessibility tree of the screen, as YAML, and with `html: true` also the static HTML page. |

A tool that cannot do its job (an unknown component, a patch that does not apply, markup that `weft_format` or `weft_render` cannot accept) sets `isError` in its result and never throws. `weft_validate` is different: finding problems is its job, so it answers `"valid": false` with the diagnostics as an ordinary result. Inputs are bounded (for example 200,000 characters of markup and 100 patches per call); the numbers are in `packages/mcp/README.md`.

## Try the tools without an agent

You can call the tools yourself with `docs/examples/mcp-call.mjs`, a 40-line client that starts the server, makes one call and prints the text it returns. A value written `@file` is read from the file. The examples use the scratch folder from the [tour](tour.md); this block makes it again if you skipped that page.

```console
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
$ node docs/examples/mcp-call.mjs weft_catalog | head -n 6
catalog weft-core 0.1.0 (weft 0.1); kind | role | content | description
screen | main | nodes | The root of every document: one full screen of UI, carrying the format version.
stack | none | nodes | Lays its children out in one line, as a column or a row; use it for most vertical or horizontal grouping.
grid | none | nodes | Lays its children out in a fixed number of equal columns; use it for card galleries and dashboards.
section | region | nodes | A named region of the screen that groups related content; use it to divide a screen into labelled parts.
heading | heading | text | A title for the content that follows it; the level orders headings from 1 (most important) to 6.
$ node docs/examples/mcp-call.mjs weft_catalog kind=alert
{"kind":"alert","description":"A message that tells the user about a result or problem, announced to assistive technology.","role":"alert","content":"mixed","props":{"tone":{"description":"Severity of the message.","type":"enum","values":["info","success","warning","danger"],"default":"info"},"text":{"description":"The text to show, given as a value instead of content; bind it when the text comes from data. Never give both.","type":"string"}}}
```

`weft_validate` returns its diagnostics as data. They carry everything a model needs to repair the markup: the code, the path to the element, what was expected, what was found and a hint.

```console
$ node docs/examples/mcp-call.mjs weft_validate markup='<screen id="s" weft="0.1"><button id="b" variant="primry">Go</button></screen>'; echo "exit $?"
{"valid":false,"diagnostics":[{"code":"W203","severity":"error","message":"\"primry\" is not an allowed value.","path":"/screen#s/button#b/@variant","line":1,"column":42,"expected":"one of: \"primary\", \"secondary\", \"danger\"","got":"\"primry\"","hint":"did you mean \"primary\"?"}]}
exit 0
```

`weft_format` accepts only valid markup. This call has a stray attribute order and an empty element written long-hand, so it changes:

```console
$ node docs/examples/mcp-call.mjs weft_format markup='<screen weft="0.1" id="s"><button variant="primary" id="b"></button></screen>'
<screen id="s" weft="0.1">
  <button id="b" variant="primary"/>
</screen>
```

`weft_render` shows what the screen looks like to a screen reader or a browsing agent, without needing a browser. With `data`, bindings resolve; without it, the Sign in button below is disabled because `$.email` is empty.

```console
$ node docs/examples/mcp-call.mjs weft_render markup=@weft-tour/login.weft
- main "Sign in":
  - heading "Sign in" [level=1]
  - textbox "Email"
  - textbox "Password"
  - button "Sign in" [disabled]
  - link "Forgot password?"
  - link "Create an account"
$ node docs/examples/mcp-call.mjs weft_render markup=@weft-tour/login.weft data=@corpus/login/data.json
- main "Sign in":
  - heading "Sign in" [level=1]
  - textbox "Email": ada@example.com
  - textbox "Password"
  - button "Sign in"
  - link "Forgot password?"
  - link "Create an account"
```

`weft_patch` takes the screen and a list of patches (see [Patches](patches.md)). When a patch is wrong, nothing is applied and the diagnostic points at the patch:

```console
$ echo '[{"op":"set","id":"submt","prop":"variant","value":"danger"}]' > weft-tour/bad-patch.json
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/bad-patch.json; echo "exit $?"
{"diagnostics":[{"code":"W502","severity":"error","message":"No element has the id \"submt\".","path":"#/patches/0/id","expected":"the id of an element in the document","got":"submt","hint":"did you mean \"submit\"?"}]}
exit 1
```

## Register the server

The server is a Node program that runs straight from the repository (Node 26.7 or later, no build step; the WebAssembly core must be built with `moon run root:wasm`). Register it with an absolute path to your clone.

**Claude Code.**

```bash
claude mcp add weft -- node /path/to/weft/packages/mcp/src/server.ts
claude mcp list
```

Add `--scope project` to store the registration in the project's `.mcp.json` so a team shares it. The [plugin](claude-code-plugin.md) registers the same server for you, so you only need this when you want the tools without the plugin.

**Claude Desktop.** Add the server to the `mcpServers` section of Claude Desktop's configuration file (Settings, Developer, Edit Config), then restart the app:

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

An app started from the Dock or a launcher does not read your shell setup, so it may not find a `node` that came from mise. If the server does not start, replace `"node"` with the absolute path that `mise which node` prints.

**Any other MCP client** starts `node /path/to/weft/packages/mcp/src/server.ts` and talks to it over standard input and output.

**With a project.** Add `--project /path/to/weft.json` to the arguments to serve one project ([Projects](projects.md)). Its catalog, tokens, actions and data schema become the server's, so token names, action names and bindings are checked too; its `validate.mode` becomes the default of `weft_validate`'s `strict`, and `mcp.limits` changes the bounds above. A project with errors stops the server before it starts, with the problems on standard error. A `project` argument in a tool call replaces the resources for that call only and never changes the limits.

## How an agent uses it

The server's own instructions tell the agent to call `weft_primer` first. The loop that works is:

1. `weft_capabilities` to see which catalogs, tokens and actions the host accepts, then `weft_catalog` to see the vocabulary that exists.
2. Write or change the markup, preferably as a patch.
3. `weft_validate` with `strict: true`, and fix every error using its hint.
4. `weft_render` to check the result reads right, or `weft_patch` to apply an edit.
5. Only then answer.

[AGENT-SPEC.md](../AGENT-SPEC.md) is the full guide an agent follows, including what to do for each diagnostic code. Without a project the server does not check token names, action names or bindings, because it does not know your app's: start it with `--project`, pass `project` in a call, or, in a host that embeds the server, supply them to `createServer({ tokens, actions, data })`.

## Raw protocol

If you want to see what an MCP client sends, this is the whole conversation for one `weft_validate` call, as three lines on standard input and the answer on standard output (the last line is the tool's reply):

```console
$ printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}' '{"jsonrpc":"2.0","method":"notifications/initialized"}' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"weft_validate","arguments":{"markup":"<screen id=\"s\" weft=\"0.1\"><button id=\"b\" variant=\"primry\">Go</button></screen>"}}}' | node packages/mcp/src/server.ts | tail -n 1
{"result":{"content":[{"type":"text","text":"{\"valid\":false,\"diagnostics\":[{\"code\":\"W203\",\"severity\":\"error\",\"message\":\"\\\"primry\\\" is not an allowed value.\",\"path\":\"/screen#s/button#b/@variant\",\"line\":1,\"column\":42,\"expected\":\"one of: \\\"primary\\\", \\\"secondary\\\", \\\"danger\\\"\",\"got\":\"\\\"primry\\\"\",\"hint\":\"did you mean \\\"primary\\\"?\"}]}"}]},"jsonrpc":"2.0","id":2}
```
