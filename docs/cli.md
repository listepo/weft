# The `weft` command

`weft` checks, formats and explains Weft files from a terminal. It is the Rust core as a program: it has no other dependencies, starts instantly and works the same on a laptop and in CI.

## Get it

There is no installer yet. Build it from the repository and put the folder on your `PATH`:

```bash
cargo build -p weft-cli
export PATH="$PWD/target/debug:$PATH"
```

The examples assume you have also made the scratch folder from the [tour](tour.md). This block makes it again if you skipped that page:

```console
$ mkdir -p weft-tour
$ cp corpus/login/screen.weft weft-tour/login.weft
$ weft --help
Validate, format and explain Weft documents, and convert them to and from SwiftUI, HTML, React, SolidJS, Slint and A2UI

Usage: weft <COMMAND>

Commands:
  validate        Check a document: markup, or canonical JSON when the file name ends in `.json`
  fmt             Print the canonical markup of a document
  explain         Read back what each binding, token, event and loop of a markup document means, one per line, so the meaning can be compared with the instruction behind an edit
  swiftui         Generate a SwiftUI view (iOS 17, macOS 14) from a markup document
  swiftui-tokens  Generate `WeftTokens.swift`, the design tokens every SwiftUI screen of a project shares
  import-swiftui  Read a SwiftUI view back into markup; what Weft cannot hold is listed on stderr as losses
  slint           Generate a Slint component (Slint 1.x, `std-widgets.slint`) from a markup document
  import-slint    Read a generated Slint component back into markup
  html            Generate a static HTML page with CSS and no script from a markup document
  react           Generate a React component (JSX or TSX) from a markup document
  solid           Generate a SolidJS component (JSX or TSX) from a markup document
  lit             Generate a Lit web component (JavaScript) from a markup document
  css-tokens      Generate `weft-tokens.css`, the design tokens React, SolidJS and Lit components read as CSS custom properties
  css-base        Generate `weft-base.css`, the base stylesheet React, SolidJS and Lit components share with the static page
  import-html     Read an HTML page back into markup; what Weft cannot hold is listed on stderr as losses
  import-react    Read a React component (.jsx, or .tsx as TypeScript) back into markup; losses go to stderr
  import-solid    Read a SolidJS component (.jsx, or .tsx as TypeScript) back into markup; losses go to stderr
  a2ui            Write a markup document as A2UI v0.9 messages (basic catalog); what A2UI cannot hold is listed on stderr as losses
  import-a2ui     Read A2UI v0.9 messages (a JSON array, one object or JSON Lines) back into markup; losses go to stderr
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

## `weft validate`

```console
$ weft validate weft-tour/login.weft; echo "exit $?"
weft: no --catalog and no project; only the syntax layer was checked
exit 0
$ weft validate weft-tour/login.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
exit 0
```

Without `--catalog` or a project only the syntax layer runs (is it well-formed Weft markup?) and the program says so. The core catalog is at `packages/catalog/catalog.json`; with it, `validate` also checks every element, attribute, value, slot, state, event and parent-child rule. Pass your own catalog file to check against your own vocabulary ([Catalog and tokens](catalog-and-tokens.md)).

**`--strict`** turns unknown elements and attributes from warnings into errors. Use it for anything you write and in CI. Without it, a file from a newer minor version of the format passes with warnings, which is what a reader wants. The lines look the same in both modes; the exit code is what changes:

```console
$ weft validate compat/unknown-element.weft --catalog packages/catalog/catalog.json; echo "exit $?"
compat/unknown-element.weft:2:31 W403 Version 0.2 is newer than 0.1; unknown content is read as extensions.
compat/unknown-element.weft:5:5 W401 <hologram> is not in catalog weft-core 0.1.0. — use a catalog component, or an extension named x-<vendor>-hologram
exit 0
$ weft validate compat/unknown-element.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
compat/unknown-element.weft:2:31 W403 Version 0.2 is newer than 0.1; unknown content is read as extensions.
compat/unknown-element.weft:5:5 W401 <hologram> is not in catalog weft-core 0.1.0. — use a catalog component, or an extension named x-<vendor>-hologram
exit 1
```

**Projects.** Every command first looks for a `weft.json` in the file's folder or above it and, when it finds one, checks against the project's catalog, tokens, actions and data schema, so token names, action names and bindings are checked too. `--project <file>` names another project file, `--no-project` ignores it, and `--catalog` replaces only the project's catalog. The project's `validate.mode` sets the default of `--strict` (`--lenient` overrides it), and its `format.write` the default of `fmt --write` (`--print` overrides it). Project problems are printed first, as `weft.json:#/pointer code message`. See [Projects](projects.md).

**Output.** One line per diagnostic: `file:line:column code message`, then ` — hint` when there is one. [SPEC §6.2](../SPEC.md#62-codes) lists the codes; a code never changes meaning.

**Exit codes.**

| Code | Meaning |
| --- | --- |
| 0 | No errors. Warnings may have been printed. |
| 1 | The file has errors. |
| 2 | The command was used wrongly, or a file could not be read. |

**JSON files.** A file whose name ends in `.json` is read as canonical JSON instead of markup (see [How it works](how-it-works.md#two-forms-of-one-document)). Diagnostics then show the location as a path in the document, since JSON has no lines of its own.

**Check every reference screen** in one loop (the shell prints nothing when all are valid):

```console
$ for f in corpus/*/screen.weft; do weft validate "$f" --catalog packages/catalog/catalog.json --strict || echo "FAILED $f"; done
```

## `weft fmt`

`fmt` prints the canonical form of a file: attributes in a fixed order, two-space indentation, one way to write empty elements, comments dropped. Two files that mean the same have the same canonical text, so formatting makes diffs show only real changes.

```console
$ cat > weft-tour/messy.weft <<'EOF'
<screen weft="0.1"   label="Hello"   id="hello">
  <!-- a comment -->
  <button variant="primary" id="go"></button>
</screen>
EOF
$ weft fmt weft-tour/messy.weft
<screen id="hello" label="Hello" weft="0.1">
  <button id="go" variant="primary"/>
</screen>
```

`--write` rewrites the file instead of printing it, and leaves a file that is already canonical untouched:

```console
$ weft fmt weft-tour/messy.weft --write
$ cat weft-tour/messy.weft
<screen id="hello" label="Hello" weft="0.1">
  <button id="go" variant="primary"/>
</screen>
```

`fmt` is strict about syntax. A file that is not well-formed is not "fixed"; you get a diagnostic and exit code 1:

```console
$ printf '<screen id="s" weft="0.1"><button id="b" variant=primary/></screen>' > weft-tour/broken.weft
$ weft fmt weft-tour/broken.weft; echo "exit $?"
weft-tour/broken.weft:1:50 W106 Value of attribute "variant" must be in double quotes. — write variant="…"
exit 1
```

## `weft explain`

`explain` reads every binding, token reference, event and loop of a screen back as a plain sentence. Use it to compare what a file says with what you meant, which validation cannot do. Pass the catalog so that boolean and writable props read correctly:

```console
$ weft explain weft-tour/login.weft --catalog packages/catalog/catalog.json
form#form on-submit: runs action auth.submit
stack#fields gap: design token space.md
field#email value: reads and writes $.email
field#password value: reads and writes $.password
button#submit disabled: true while $.email is falsy (NOT $.email)
link#reset on-press: runs action nav.reset
link#signup on-press: runs action nav.signup
```

With `--against <old-file>` it prints only what was added, removed or changed. This is how you check an edit, yours or a model's. Here a copy of the login screen has its condition turned around:

```console
$ sed 's/disabled="{!$.email}"/disabled="{$.email}"/' weft-tour/login.weft > weft-tour/login-flipped.weft
$ weft explain weft-tour/login-flipped.weft --against weft-tour/login.weft --catalog packages/catalog/catalog.json
button#submit disabled changed: was true while $.email is falsy (NOT $.email); now true while $.email is truthy
```

Sentences never hide a negation: a negated binding always contains `NOT`. If neither file changed anything, you get a message on stderr and no output. If either file has errors, `explain` prints the diagnostics as `validate` does and exits 1. It reads markup only, not `.json` files.

## `weft swiftui`, `weft swiftui-tokens` and `weft import-swiftui`

`weft swiftui` prints a SwiftUI file for a screen. It contains an `@Observable` model, an action enum, and the view. In a project the view reads the tokens from `WeftTokens`, which `weft swiftui-tokens` writes once for the whole token set; without a project, or with `--no-shared-tokens`, the file carries a theme with the screen's tokens instead and builds on its own. `weft import-swiftui` reads Swift source back into markup. What the generator printed comes back unchanged:

```console
$ weft swiftui weft-tour/login.weft --out-dir weft-tour/ios
$ weft import-swiftui weft-tour/ios/login.swift | diff - <(weft fmt weft-tour/login.weft) && echo same
same
```

Hand-written SwiftUI imports too. Each part that Weft cannot hold is reported on stderr as a loss. The output is still printed, and the command exits 0:

```console
$ printf 'import SwiftUI\nstruct Hello: View {\n    var body: some View { Text("Hi").padding() }\n}\n' > weft-tour/Hello.swift
$ weft import-swiftui weft-tour/Hello.swift
weft-tour/Hello.swift:/screen#screen-1 loss ids: the view has no accessibilityIdentifier; the id is generated
weft-tour/Hello.swift:/screen#screen-1 loss structure: the view's body is not one stack; a screen root is added around it
weft-tour/Hello.swift:/screen#screen-1/text#text-hi loss ids: the view has no accessibilityIdentifier; the id is generated
weft-tour/Hello.swift:/screen#screen-1/text#text-hi loss layout: `.padding` not kept
<screen id="screen-1" weft="0.1">
  <text id="text-hi">Hi</text>
</screen>
```

The commands use the project's catalog and tokens, or `--catalog` and `--tokens` when you give them; `weft swiftui-tokens` finds the project from the working directory. `--out-dir` writes `<name>.swift`, `WeftTokens.swift` or `<name>.weft` instead of printing. Without it, the project's `export.swiftui.outDir` or `import.swiftui.outDir` decides ([Projects](projects.md)). `--shared-tokens` and `--no-shared-tokens` override the project's `export.swiftui.sharedTokens`. A kind of the project's own catalog becomes a call of a view the app writes (`rating` → `RatingView`); the header comment of the screen file names them. The mapping table and every loss are listed in `crates/weft-swiftui/README.md`.

With `--data <file>` (or the project's `export.swiftui.data`), the model also gets an initializer and a `sample` built from that JSON, and `#Preview` shows it. The importer ignores both.

## `weft slint` and `weft import-slint`

`weft slint` prints a Slint component for a screen (Slint 1.x, `std-widgets.slint`): one exported `Window`, one property per data path, and a `perform` callback for events. `weft import-slint` reads that file back. What the generator printed comes back unchanged:

```console
$ weft slint weft-tour/login.weft --out-dir weft-tour/ui
$ weft import-slint weft-tour/ui/login.slint | diff - <(weft fmt weft-tour/login.weft) && echo same
same
```

`--name` sets the component name (`SignInScreen` for `--name sign-in`); without it the name comes from the screen id. The commands use the project's catalog and tokens, or `--catalog` and `--tokens` when you give them to `weft slint`. `--out-dir` writes `<stem>.slint` or `<stem>.weft` (the input file's stem, not the component name) instead of printing. Without it, the project's `export.slint.outDir` or `import.slint.outDir` decides ([Projects](projects.md)). An existing file is left in place unless `--force` is passed. A screen the generator cannot express, and a Slint file it did not print, are reported on stderr and the command exits 1. The mapping is in SPEC §9 and `crates/weft-slint/README.md`.

## `weft html`, `weft react`, `weft solid`, `weft lit`, `weft css-tokens`, `weft css-base` and the importers

`weft html` prints a static page: semantic HTML, the tokens as CSS custom properties, and no script. Bindings, events and repetition are kept as inert `data-` attributes and `<template>` elements. With `--data <file>` (or the project's `export.html.data`), the page shows that data instead: bound values filled in, one copy per list item, empty slots shown. Such a page is a picture of the screen, not a template. `weft react` and `weft solid` print one self-contained component, JSX by default or TSX with `--typescript`. With `--source`, the output keeps the screen in a leading comment, and the matching importer gives it back unchanged:

```console
$ weft react weft-tour/login.weft --source --out-dir weft-tour/web
$ weft import-react weft-tour/web/login.jsx | diff - <(weft fmt weft-tour/login.weft) && echo same
same
```

`weft lit` prints one Lit element as JavaScript: a custom element that takes `data`, `actions` and `onChange` as properties and renders into the light DOM, so it needs the same `weft-tokens.css` and `weft-base.css` as the other components. It has no TSX flavour, no `--source` and no importer; asking for the first two is an error.

Without the comment, generated code still reads back by its conventions; only the ids of `<each>` are generated again. Hand-written pages and components import too, with their losses on stderr:

```console
$ printf 'export default function Hello({ data }) {\n  const [open, setOpen] = useState(false);\n  return <main aria-label="Hello"><p style={{ padding: 8 }}>Hi {data.name}</p><button onClick={() => setOpen(!open)}>Toggle</button></main>;\n}\n' > weft-tour/Hello.jsx
$ weft import-react weft-tour/Hello.jsx
weft-tour/Hello.jsx:/screen#screen-hello loss ids: 3 elements carry no data-weft-id; their ids are generated
weft-tour/Hello.jsx:/screen#screen-hello/text#text-hi loss text: the text written beside the bound value is left out
weft-tour/Hello.jsx:/screen#screen-hello/text#text-hi loss layout: style padding: 8px is left out
weft-tour/Hello.jsx:/screen#screen-hello/button#button-toggle loss actions: onClick={() => setOpen(!open)} calls no action Weft can read; it is left out
<screen id="screen-hello" label="Hello" weft="0.1">
  <text id="text-hi" text="{$.name}"/>
  <button id="button-toggle">Toggle</button>
</screen>
```

The source is parsed, never run. `import-react` and `import-solid` read `.tsx` (and `.ts`) files as TypeScript. `--source`/`--no-source`, `--typescript`/`--javascript` and `--out-dir` override the project's `export.<target>` and `import.<target>` settings ([Projects](projects.md)); `--catalog` and `--tokens` replace the project's catalog and tokens. The parsers and the mapping are described in `crates/weft-web/README.md` and SPEC §9.

A component reads its tokens as `var(--weft-…)`. `weft css-tokens` writes them once for the app as `weft-tokens.css`, from the project found in the working directory (or `--project`, `--tokens`). It prints unless `--out-dir` or the project's `export.css.outDir` names a folder. When the project's tokens are a resolver with light and dark themes ([Projects](projects.md#light-and-dark-a-resolver)), the stylesheet and the page from `weft html` follow the system appearance: the light values on `:root`, the ones that differ in the dark under `@media (prefers-color-scheme: dark)`. A typography token is the `font` shorthand plus `--weft-<path>-letter-spacing`.

### The base stylesheet

`weft css-base` writes the rules that every web target shares as `weft-base.css`, to the same folder as `weft-tokens.css` and by the same rule (`--out-dir`, else `export.css.outDir`, else standard output). It does not depend on the project: its values are the `var(--weft-…)` properties of `weft-tokens.css`, each with the default token's value as its fallback, so link it after `weft-tokens.css` (or alone). `weft html` puts the same rules in the page's `<style>`, so a page and the components look the same.

It is a base, not a theme. It sets no page colour and no page font; it puts a field's caption above its control, a checkbox, switch or radio and its caption in a row, spaces the fields of a form, makes links (including `role="link"` elements) look like links, styles buttons (`primary` and `danger` are filled) and gives tabs, alerts, tones, footers, tables and lists the few rules they need. It uses `space.xs`, `space.sm`, `space.md`, `radius.sm`, `radius.pill`, `font-size.sm`, `color.white`, `color.action.primary`, `color.action.danger` and, if your tokens have them, `color.success` and `color.warning`; borders and muted text mix `currentColor`, so light and dark follow the page. It uses `:has()` (Chromium 105, Safari 15.4, Firefox 121 or later). [SPEC §9](../SPEC.md) lists every rule's target.


## `weft a2ui` and `weft import-a2ui`

[A2UI](https://a2ui.org/specification/v0.9-a2ui/) is the format agents send to render a UI. `weft a2ui` writes the messages of one surface (`createSurface` and `updateComponents`, A2UI v0.9, basic catalog) as a JSON array, and `weft import-a2ui` reads them back: a JSON array, one object, or JSON Lines. What A2UI has no form for (design tokens, the `state` prop, `on-change` events, a `dialog`'s own opening) is listed on stderr, one loss per line, as for the other importers; the table is in SPEC section 9.

```console
$ weft a2ui weft-tour/login.weft --out-dir weft-tour/a2ui
$ weft import-a2ui weft-tour/a2ui/login.a2ui.json
```

The commands use the project's catalog, or `--catalog`. `--out-dir` writes `<name>.a2ui.json` or `<name>.weft` instead of printing, and falls back to the project's `export.a2ui.outDir` or `import.a2ui.outDir` ([Projects](projects.md)). The data of a screen is not part of it, so no `updateDataModel` message is written, and one in the input is ignored.

## What `weft` does not do

- It does not check token names, action names or bindings without a project: those checks need your app's tokens, actions and data schema, which a `weft.json` declares ([Projects](projects.md)).
- It does not render pages with sample data, and it does not convert to or from Figma, Penpot or json-render. Those are in the [plugin scripts](claude-code-plugin.md) and the packages.
- It does not apply patches. [Patches](patches.md) go through the MCP server or the library.

## The Node version

`packages/core/src/cli.ts` is an older front end with the same name that runs on Node through WebAssembly. It has `validate` and `fmt` with the same output, but no `explain` and no project support:

```console
$ node packages/core/src/cli.ts validate weft-tour/login.weft --catalog packages/catalog/catalog.json --strict; echo "exit $?"
exit 0
```

Prefer the Rust program.
