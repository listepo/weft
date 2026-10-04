# What is Weft

Weft is an open format for describing a screen of a user interface so that an AI agent can read it, write it, check it and change it safely, and so that ordinary tools can check, preview and convert it too.

## The problem

When you ask a language model for a UI today, it usually writes HTML or React. That works for a demo, and it is hard to trust for anything else:

- **There is nothing to check against.** HTML accepts any tag and any attribute. A model can invent `<fancy-button>` or misspell `aria-label`, and the browser shrugs.
- **It is executable.** A React component can run arbitrary code. You cannot show a model's output to a user without reviewing it, and you cannot let a model edit it freely.
- **Edits are rewrites.** To change one label the model returns the whole file, and a tool cannot tell what changed or whether it meant to.
- **Meaning is hidden.** Which part is the main action, which field is required, what happens on submit: a person infers it from the code, and so does the model, sometimes wrongly.

Weft is a smaller thing on purpose: a screen is data, the vocabulary is closed, and every mistake produces an error message a model can act on.

## The idea

**Strict markup for models.** A Weft file looks like HTML and follows stricter rules: a restricted subset of XML, double-quoted attributes, one way to say each thing, no shortcuts.

```xml
<screen id="login" label="Sign in" weft="0.1">
  <form id="f1" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" submit="true" variant="primary">Sign in</button>
  </form>
</screen>
```

**Canonical JSON for tools.** The same screen exists as typed JSON (`.weft.json`). The two forms convert into each other without loss (comments and extra whitespace aside), and a given screen has exactly one canonical text, so two files that mean the same are byte-for-byte equal. Tools diff, validate and render the JSON; models read and write the markup.

**A catalog.** A closed list of components (`button`, `field`, `table`, `dialog` and 25 more), each with its props, slots, states and events. Anything not in the catalog is an error, with a "did you mean" hint. The catalog is data you can read, extend and version. See [Catalog and tokens](catalog-and-tokens.md).

**Design tokens.** Colors, sizes and spacing are references such as `{token.space.md}`, never raw values. The tokens themselves live in a standard file format (W3C Design Tokens, DTCG 2025.10), so a designer owns them.

**Bindings.** Where a value comes from your app's data, you write `{$.user.email}`. A binding reads a path in your data, and for input fields it writes back. `{!$.email}` is the negation. There are no other expressions: no `+`, no `if`, no function calls.

**Actions.** Where something should happen, you write a name: `on-press="nav.reset"`. Your app decides what `nav.reset` does. The document never carries behavior.

**Stable ids and patches.** Every element has an id. A change is a short patch such as "set `disabled` of `submit`" or "insert this checkbox into `fields`", applied atomically. Nothing is half-edited, and the diff is exactly what was asked for. See [Patches](patches.md).

**Roles from the web platform.** Every component has an ARIA role, so a screen carries its accessibility structure. The reference renderer produces an accessibility tree that equals what the document declares.

**No code.** Documents are untrusted input. The parser and validator never run anything, never throw on bad input and never touch the network. They return diagnostics.

[SPEC.md](../SPEC.md) is the exact definition. [AGENT-SPEC.md](../AGENT-SPEC.md) is the same format written for the model that uses it.

## How it compares

| | Good at | Weak at |
| --- | --- | --- |
| **HTML** | Familiar to every model. | No closed vocabulary, nothing to validate against, executable when mixed with scripts. |
| **React JSX** | Complete control. | Arbitrary code, so it must be reviewed; edits are rewrites; no structure to patch. |
| **A2UI-style catalog JSON** | Strict validation, patches by id, safe to render. | Verbose, and the tree is spread over flat references, so it reads poorly. |
| **Weft** | Reads like HTML, validates like JSON, patches by id, carries accessibility roles and tokens. | New syntax that has to earn its place; a deliberately small vocabulary. |

The reasoning behind these choices, with sources, is in [research.md](../research.md). On the 12 reference screens, Weft is about as large as the HTML version and 67% smaller than the A2UI version, counted in tokens with a stand-in tokenizer ([bench/REPORT.md](../bench/REPORT.md)).

## What you can do with it today

- Validate and format Weft files from a terminal: [the `weft` command](cli.md).
- Let an AI agent read, write and patch screens with checking built in: [the MCP server](mcp.md) and [the Claude Code plugin](claude-code-plugin.md).
- Preview a screen as an HTML page or a React tree: [Rendering](rendering.md).
- Bring an existing web page in, losses reported: [Importing HTML](importing.md).
- Get a React component out: [Exporting to React](exporting-jsx.md).

## Honest limits

Weft is a prototype. Know these before you rely on it:

- **The vocabulary is small.** 29 components. Layout is only `stack` and `grid`; there is no free positioning, no animation and no theming beyond tokens. Extensions (`x-<vendor>-…` elements with a fallback role) exist, but there is no registry for sharing them.
- **One screen per file.** Screens do not share anything yet: each one stands alone, and the data model and action names are agreed with your app by convention.
- **Import is lossy.** A web page has no ids, bindings, actions or tokens, so an import gets generated ids and a list of losses. It is a starting point, not a conversion.
- **One renderer.** The reference renderer targets React and produces plain, unstyled HTML. It exists to prove the structure and accessibility tree, not to look finished.
- **The evidence is small.** The benchmark in [test.md](../test.md) is 12 screens and 60 tasks written by the authors, and the final evaluation is not finished. Treat "models write it reliably" as promising, not proven.
- **Some checks need the library.** The `weft` command checks syntax and the catalog; it does not check token names or action names. Those checks exist in the TypeScript API.

## Coming

A Figma round trip, a project file that screens share, and installing the Claude Code plugin straight from GitHub are in progress.
