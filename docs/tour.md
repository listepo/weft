# A ten-minute tour

In this tour you take one real screen, the login form from the `corpus/` folder, and work with it from start to finish. You read it, check it, break it on purpose and fix it, look at it in a browser, change it with a patch, ask Weft what the change means, and turn the screen into React code and back.

No model is involved. Everything runs on your machine, and every command below was run exactly as printed. The output under each command is what it printed.

## Set up

Run these once in the repository root. They install the pinned tools, build the Rust core as WebAssembly (the TypeScript packages load it), and build the `weft` command-line program. They are not repeated in the other pages.

```bash
mise install
pnpm install
moon run root:wasm
cargo build -p weft-cli
```

If `node`, `cargo` or `moon` are not found, your shell does not use mise yet. Put `mise exec --` in front of each command.

The program is built to `target/debug/weft`. Put that folder on your `PATH` for this shell, then make a scratch folder for the tour with a copy of the screen:

```console
$ export PATH="$PWD/target/debug:$PATH"
$ weft --version
weft 0.1.0
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
```

You remove the folder at the end.

## 1. Read the screen

```console
$ cat weft-tour/login.weft
<screen id="login" label="Sign in" weft="0.1">
  <form id="form" state="idle" on-submit="auth.submit">
    <heading id="title" level="1">Sign in</heading>
    <stack id="fields" gap="{token.space.md}">
      <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
      <field id="password" label="Password" required="true" type="password" value="{$.password}"/>
    </stack>
    <button id="submit" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
    <slot name="footer">
      <link id="reset" on-press="nav.reset">Forgot password?</link>
      <link id="signup" on-press="nav.signup">Create an account</link>
    </slot>
  </form>
</screen>
```

That is the whole screen. Read it from the outside in:

| Element | What it is |
| --- | --- |
| `<screen id="login" label="Sign in" weft="0.1">` | The root of every Weft file. `weft="0.1"` is the format version, `label` is the accessible name of the page. |
| `<form id="form" state="idle" on-submit="auth.submit">` | A form. `state` is one of the states a form is allowed to have (`idle`, `submitting`, `invalid`). `on-submit` names the action your app runs when the form is submitted. Weft never says what the action does, only its name. |
| `<heading id="title" level="1">` | A heading. The text between the tags is its content. |
| `<stack id="fields" gap="{token.space.md}">` | A layout container that stacks its children. The gap is not a number: `{token.space.md}` points to a design token, so a designer can change the spacing in one place. |
| `<field id="email" … value="{$.email}"/>` | A text input. `{$.email}` is a binding: it reads, and when the user types it also writes, the path `email` in your app's data. `type="email"` and `required="true"` are plain values. |
| `<button id="submit" disabled="{!$.email}" submit="true" …>` | A button that submits the form. `{!$.email}` is a negated binding: the button is disabled while `email` is empty or missing. |
| `<slot name="footer">` | A named place in the form for the two links. The form decides where the footer goes, so where the slot stands in the file does not matter. |
| `<link id="reset" on-press="nav.reset">` | A link that runs the action `nav.reset` when pressed. |

Three things to notice. Every element has an `id` that stays the same through edits, so a tool can say "change `submit`" without counting lines. Every attribute value is exactly one of four things: a plain value, a binding like `{$.email}`, a negated binding like `{!$.email}`, or a token reference like `{token.space.md}`. And there is no code anywhere, only names and paths.

[SPEC §2](../SPEC.md#2-markup-syntax) has the exact rules.

## 2. Check it

`weft validate` reads the file and checks it against the catalog, the list of every element, attribute, state and event that exists (see [Catalog and tokens](catalog-and-tokens.md)). `--strict` also rejects anything unknown, which is what you want before a file is shared. Silence and exit code 0 mean the file is valid.

```console
$ weft validate weft-tour/login.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
exit 0
```

## 3. Break it, read the diagnostic, fix it

Change `primary` to the typo `primry`:

```console
$ sed 's/variant="primary"/variant="primry"/' weft-tour/login.weft > weft-tour/login-typo.weft
$ weft validate weft-tour/login-typo.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
weft-tour/login-typo.weft:8:61 W203 "primry" is not an allowed value. — did you mean "primary"?
exit 1
```

One line per problem: file, line and column, a stable code (`W203`), what is wrong, and a hint with the smallest fix. Codes never change meaning, so you can search for them; [SPEC §6.2](../SPEC.md#62-codes) lists all of them. Exit code 1 means the file has errors. Apply the hint:

```console
$ sed 's/primry/primary/' weft-tour/login-typo.weft > weft-tour/login-fixed.weft
$ weft validate weft-tour/login-fixed.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
exit 0
```

A misspelled element name gets the same treatment:

```console
$ sed 's/button/buton/g' weft-tour/login.weft > weft-tour/login-buton.weft
$ weft validate weft-tour/login-buton.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
weft-tour/login-buton.weft:8:5 W401 <buton> is not in catalog weft-core 0.1.0. — did you mean "button"?
exit 1
```

## 4. See it in a browser

The reference renderer turns a screen into a page. The Claude Code plugin ships a script that does it for a file; `--data` supplies the sample data the bindings read (here an email address, so the Sign in button is enabled).

```console
$ node plugins/shared/scripts/render.ts weft-tour/login.weft weft-tour/login.html --data corpus/login/data.json
Wrote weft-tour/login.html
file:///path/to/weft/weft-tour/login.html
```

Open the page with `open weft-tour/login.html` on macOS or `xdg-open weft-tour/login.html` on Linux. It is plain, browser-default HTML: the renderer's job is a correct structure and accessibility tree, not a visual design (see [Rendering](rendering.md)). Its structure is what a screen reader gets:

```console
$ head -c 330 weft-tour/login.html
<!doctype html>
<html lang="en"><head><meta charSet="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><title>Weft: login</title></head><body><main data-weft-id="login" aria-label="Sign in"><form data-weft-id="form" data-state="idle"><h1 data-weft-id="title">Sign in</h1><div data-weft-id="fields" style
```

## 5. Change it with a patch

An agent does not rewrite a screen to change it. It sends a patch: a short list of operations addressed by `id`. This one adds a "Remember me" checkbox below the password field and disables the Sign in button while the app is busy.

```console
$ cat > weft-tour/patches.json <<'EOF'
[
  {
    "op": "insert",
    "parent": "fields",
    "markup": "<checkbox id=\"remember\" label=\"Remember me\" checked=\"{$.remember}\"/>"
  },
  { "op": "set", "id": "submit", "prop": "disabled", "value": { "bind": "$.busy" } }
]
EOF
```

The patch tool is part of the MCP server (see [MCP server](mcp.md)). `docs/examples/mcp-call.mjs` calls it without needing an MCP client:

```console
$ node docs/examples/mcp-call.mjs weft_patch markup=@weft-tour/login.weft patches=@weft-tour/patches.json > weft-tour/login-patched.weft; echo "exit $?"
exit 0
$ diff weft-tour/login.weft weft-tour/login-patched.weft
6a7
>       <checkbox id="remember" checked="{$.remember}" label="Remember me"/>
8c9
<     <button id="submit" disabled="{!$.email}" submit="true" variant="primary">Sign in</button>
---
>     <button id="submit" disabled="{$.busy}" submit="true" variant="primary">Sign in</button>
```

A patch is all or nothing: if one operation fails, nothing changes and you get a diagnostic. The result is validated strictly and written in canonical form (attributes in a fixed order), so the diff shows only the real change. More in [Patches](patches.md).

## 6. Ask what the change means

A valid file can still say the wrong thing. The classic mistake is an inverted condition: you ask for "disabled while busy" and the file says "disabled while not busy". Validation cannot see that. `weft explain` reads every binding back as a plain sentence, and `--against` shows only what changed:

```console
$ weft explain weft-tour/login-patched.weft --against weft-tour/login.weft --catalog packages/catalog/catalog.json
checkbox#remember checked added: true while $.remember is truthy, and user input writes $.remember
checkbox#remember label added: always "Remember me"
button#submit disabled changed: was true while $.email is falsy (NOT $.email); now true while $.busy is truthy
```

Read the last line and compare it with what you asked for: the button is now disabled while `$.busy` is truthy. That is right. If the file said `{!$.busy}`, the sentence would end with `is falsy (NOT $.busy)`, and the mistake would be visible.

## 7. Turn it into React and back

Export the screen to a React component:

```console
$ node plugins/shared/scripts/export.ts weft-tour/login.weft weft-tour/LoginScreen.jsx --name LoginScreen
Wrote weft-tour/LoginScreen.jsx
$ grep -n "export default" weft-tour/LoginScreen.jsx
28:export default function LoginScreen({ data, actions, onChange }) {
```

The component takes `data`, `actions` and `onChange` and needs no other Weft code at run time (see [Exporting to React](exporting-jsx.md)).

Import goes the other way. Feed the page the renderer wrote back into the importer. It prints what it could not carry over, because a web page has no bindings, actions or design tokens:

```console
$ node plugins/shared/scripts/import.ts weft-tour/login.html weft-tour/imported.weft
Wrote weft-tour/imported.weft

Import losses:

| Kind | Path | Note |
| --- | --- | --- |
| bindings | /screen#login | values are the resolved values the page shows, not bindings |
| actions | /screen#login | event handlers and their action names are not in the HTML |
| tokens | /screen#login | design token references are rendered as CSS and cannot be mapped back |
| slots | /screen#login | slot membership is not in the HTML; slot content is imported as default content |
| hidden | /screen#login | elements a renderer leaves out (hidden, closed dialogs) are not in the HTML |
| layout | /screen#login/form#form/stack#fields | gap 16px cannot be mapped back to a design token |
$ cat weft-tour/imported.weft
<screen id="login" label="Sign in" weft="0.1">
  <form id="form" state="idle">
    <heading id="title" level="1">Sign in</heading>
    <stack id="fields">
      <field id="email" label="Email" required="true" type="email" value="ada@example.com"/>
      <field id="password" label="Password" required="true" type="password" value=""/>
    </stack>
    <button id="submit" submit="true" variant="primary">Sign in</button>
    <link id="reset">Forgot password?</link>
    <link id="signup">Create an account</link>
  </form>
</screen>
```

The ids survived because the renderer writes them into the page as `data-weft-id`. A page from anywhere else gets generated ids. See [Importing HTML](importing.md).

## Clean up

The other pages reuse this folder. When you are done with the guide, remove it:

```bash
rm -r weft-tour
```

## Where next

- [How it works](how-it-works.md) shows how the pieces you just used fit together.
- [The `weft` command](cli.md), [the MCP server](mcp.md) and [the Claude Code plugin](claude-code-plugin.md) are the three ways to use Weft day to day.
