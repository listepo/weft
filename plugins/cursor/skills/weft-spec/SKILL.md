---
name: weft-spec
description: How to write, validate, patch and repair Weft UI markup. Use whenever you create or edit a .weft file, or answer a diagnostic from the weft MCP tools.
---

Weft describes UI as strict markup that you read, write and patch. The full agent guide is [AGENT-SPEC.md](AGENT-SPEC.md), next to this file: read it before you write or change any Weft markup, and follow it for patches and for every diagnostic code.

The weft MCP server of this plugin gives you the tools; they never touch files, so read the `.weft` file yourself, pass its text as `markup`, and write the result back.

| Tool            | Use it for                                                                |
| --------------- | ------------------------------------------------------------------------- |
| `weft_primer`   | The syntax rules in a page; call it first                                 |
| `weft_catalog`  | The components, their props and variants; the only vocabulary that exists |
| `weft_validate` | Check markup (strict before you answer)                                   |
| `weft_format`   | The canonical form of markup                                              |
| `weft_patch`    | Change a screen with patch operations instead of rewriting it             |
| `weft_render`   | The accessibility tree of a screen, and with `html: true` the page        |

Workflow: write or edit, validate in strict mode, fix every error using its hint, and only then answer. To see a screen in a browser use the `weft-render` skill; to bring in an HTML page use `weft-import`; to get React code use `weft-export`.
