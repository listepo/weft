# Projects: `weft.json`

A real app has more than one screen, and its screens share things: design tokens, components of its own, the actions the app handles and the shape of its data. A project file, `weft.json`, declares them once. Every tool finds it by walking up from the screen, the way TypeScript finds `tsconfig.json`, so a screen never names what it uses. The same file holds the tools' settings, so every screen of the project is checked, rendered and exported the same way.

The specification is [SPEC §10](../SPEC.md#10-projects); this page shows it in use.

## An example

`examples/project/` is a small shop with two screens:

```text
examples/project/
├── weft.json
├── catalog.json          a component of its own, <rating>, and a ghost button
├── data.schema.json      the shape of the app's data
├── sample.data.json      data to preview the screens with
├── tokens/
│   ├── theme.resolver.json the layers below, then a light or dark theme
│   ├── base.tokens.json    colours, spacing and the body text
│   ├── brand.tokens.json   the brand colour, layered over the base
│   └── dark.tokens.json    the brand colour of the dark theme
└── screens/
    ├── cart.weft
    └── review.weft
```

```json
{
  "$schema": "../../schemas/weft.schema.json",
  "tokens": "tokens/theme.resolver.json",
  "catalog": "catalog.json",
  "actions": ["cart.checkout", "cart.remove", "nav.back"],
  "data": "data.schema.json",
  "validate": { "mode": "strict" },
  "render": { "data": "sample.data.json" }
}
```

The first five members are the shared resources:

| Member | What it is |
| --- | --- |
| `tokens` | DTCG token files, in layer order: a later file overrides an earlier one, and aliases are resolved after the merge. Or one DTCG resolver file, for tokens with modes such as light and dark ([below](#light-and-dark-a-resolver)). |
| `catalog` | An extension of the core catalog: new components, and new values or props on core ones. It may widen the core catalog, never narrow it. |
| `actions` | The action names the app handles. An `on-*` value outside the list is an error. |
| `data` | A JSON Schema (a 2020-12 subset) of the app's data. Bindings are checked against it, names and types. |
| `$schema` | Ignored by the tools. Editors use it to complete and check the file. |

The rest are tool settings, described [below](#settings).

File names are relative to `weft.json` and must stay inside its folder: `../shared.json` or an absolute path is refused.

## What the project changes

With the project, the cart screen is valid. Without it, `<rating>` and the ghost button do not exist:

```console
$ weft validate examples/project/screens/cart.weft; echo "exit $?"
exit 0
$ weft validate examples/project/screens/cart.weft --no-project --catalog packages/catalog/catalog.json --strict; echo "exit $?"
examples/project/screens/cart.weft:8:11 W401 <rating> is not in catalog weft-core 0.1.0. — use a catalog component, or an extension named x-<vendor>-rating
examples/project/screens/cart.weft:10:31 W203 "ghost" is not an allowed value.
exit 1
```

The project also catches what the core catalog cannot know: an action the app does not handle and a binding to data that does not exist.

```console
$ sed 's/\$.cart.total/$.cart.totl/; s/nav.back/nav.home/' examples/project/screens/cart.weft > examples/project/screens/typo.weft
$ weft validate examples/project/screens/typo.weft; echo "exit $?"
examples/project/screens/typo.weft:20:23 W308 Action "nav.home" is not provided by the host.
examples/project/screens/typo.weft:19:22 W315 The data schema does not declare "totl" in $.cart. — did you mean "total"?
exit 1
$ rm examples/project/screens/typo.weft
```

## Light and dark: a resolver

Tokens that differ by mode (light and dark, compact and roomy, two brands) live in a [DTCG resolver](https://www.designtokens.org/TR/2025.10/resolver/) file, which `tokens` names instead of a list. The example project's `tokens/theme.resolver.json` layers the base and brand files as before, then adds the dark theme's file when the theme is dark:

```json
{
  "version": "2025.10",
  "sets": {
    "foundation": {
      "sources": [{ "$ref": "base.tokens.json" }, { "$ref": "brand.tokens.json" }]
    }
  },
  "modifiers": {
    "theme": {
      "contexts": { "light": [], "dark": [{ "$ref": "dark.tokens.json" }] },
      "default": "light"
    }
  },
  "resolutionOrder": [{ "$ref": "#/sets/foundation" }, { "$ref": "#/modifiers/theme" }]
}
```

- Validation uses the default of each modifier (its first context when it names none).
- A modifier with `light` and `dark` contexts is the appearance: generated SwiftUI colours, pages and stylesheets follow the system's light or dark mode.
- File references are relative to the resolver and stay inside the project. Every problem is `W705` (or `W703` and `W704` for files) with a pointer into the resolver, and the rest still loads.
- A plain list of token files works as before.
- In Figma the contexts of a modifier become variable modes, and in Penpot token themes, when the plugin is given that modifier; exporting from the file gives the modes back as a resolver document (`@weft/figma`, `@weft/penpot`).
- React and SolidJS components read their tokens from CSS custom properties: `weft css-tokens` writes them as `weft-tokens.css`, light values first and the dark ones under `prefers-color-scheme: dark`. Link it once in the app, with `weft css-base`'s `weft-base.css` after it ([the base stylesheet](cli.md#the-base-stylesheet)).

## Choosing the project

Every tool that reads a screen looks for the first `weft.json` in the screen's folder or above it.

- `--project <file>` uses another project file.
- `--no-project` uses none.
- `--catalog <file>` (the `weft` command) replaces the project's catalog and keeps the rest.

The MCP server has no screen to look from, so it never looks: its host names the file with `--project` (see [The MCP server](mcp.md)), and a tool call can pass a whole project as its `project` argument.

## Settings

Anything a tool lets you choose can also be set in `weft.json`, in one section per tool. Every section and every key is optional. An argument you give a tool always wins over the file, and the file wins over the tool's default.

| Key | Default | What it sets |
| --- | --- | --- |
| `validate.mode` | `"lenient"` | `"strict"` makes unknown elements and attributes errors in `weft validate` (`--lenient` overrides it), and is the default of `strict` in the MCP server started with the project. |
| `format.write` | `false` | `weft fmt` rewrites the file instead of printing it (`--print` overrides it). |
| `render.data` | none | Sample data for rendered pages (`--data` overrides it). |
| `render.tokens` | the project's `tokens` | Token files for rendered pages, layered the same way, or one resolver file (`--tokens` overrides it). |
| `render.outDir` | next to the screen | Where rendered pages go (an output path overrides it). |
| `render.appearance` | the default context | `"light"` or `"dark"`: which theme of a resolver's light and dark modifier rendered pages use (`--appearance` overrides it). |
| `export.html.outDir` | standard output | Where `weft html` writes `<screen>.html` (`--out-dir` overrides it). |
| `export.html.source` | `false` | The page keeps the screen in a leading comment, so `weft import-html` gives it back exactly (`--no-source` overrides it). Off by default: a deployed page would publish it. |
| `export.html.data` | none | Sample data `weft html` shows in the page instead of keeping bindings as a template (`--data` overrides it). |
| `export.react.outDir` | next to the screen | Where exported React components go; `weft react` prints when it is absent (`--out-dir` overrides it). |
| `export.react.typescript`, `export.solid.typescript` | `false` | Write TSX with typed props (`--javascript` overrides it). |
| `export.react.source`, `export.solid.source` | `false` | The component keeps the screen in a leading comment, so `weft import-react` and `weft import-solid` give it back exactly (`--no-source` overrides it). |
| `export.solid.outDir` | standard output | Where `weft solid` writes `<screen>.jsx` or `.tsx` (`--out-dir` overrides it). |
| `export.lit.outDir` | standard output | Where `weft lit` writes `<screen>.js` (`--out-dir` overrides it). |
| `import.html.outDir` | next to the page | Where screens imported from HTML go; `weft import-html` prints when it is absent (`--out-dir` overrides it). |
| `import.react.outDir`, `import.solid.outDir` | standard output | Where `weft import-react` and `weft import-solid` write `<file>.weft` (`--out-dir` overrides it). |
| `export.swiftui.outDir` | standard output | Where `weft swiftui` writes `<screen>.swift` and `weft swiftui-tokens` writes `WeftTokens.swift`, and the Xcode command plugin's `export` (next to the screen when absent; `--out-dir` overrides it). The build tool plugin ignores it and writes into the build folder. |
| `export.css.outDir` | standard output | Where `weft css-tokens` writes `weft-tokens.css`, the stylesheet with the `--weft-…` properties that React, SolidJS and Lit components read, and `weft css-base` writes `weft-base.css`, their base rules (`--out-dir` overrides it). |
| `export.swiftui.sharedTokens` | `true` | Screens read the tokens from one shared `WeftTokens.swift` instead of each carrying a theme with the tokens it uses (`--shared-tokens` and `--no-shared-tokens` override it). The Xcode build tool plugin writes `WeftTokens.swift` once per target. |
| `export.swiftui.data` | none | Sample data `weft swiftui` builds the model's `sample` from, for `#Preview` (`--data` overrides it). |
| `import.swiftui.outDir` | standard output | Where `weft import-swiftui` writes `<file>.weft`, and the Xcode command plugin's `import` (next to the view when absent; `--out-dir` overrides it). |
| `export.a2ui.outDir`, `import.a2ui.outDir` | standard output | Where `weft a2ui` writes `<screen>.a2ui.json` and `weft import-a2ui` writes `<file>.weft` (`--out-dir` overrides them). |
| `export.slint.outDir`, `import.slint.outDir` | standard output | Where `weft slint` writes `<screen>.slint` and `weft import-slint` writes `<file>.weft` (`--out-dir` overrides them). |
| `import.cem.outDir` | standard output | Where `weft import-cem` writes `<file>.catalog.json`, the catalog imported from a Custom Elements Manifest (`--out-dir` overrides it). |
| `import.cem.name`, `import.cem.version` | the manifest's file stem; `0.0.0` | The name and version of that catalog (`--name` and `--version` override them). |
| `import.figma.outDir` | the working directory | Where the plugins' `figma-pull` script writes `<screen id>.weft`, a Figma frame read through the REST API (an output path argument overrides it). |
| `import.figma.pluginId` | `weft-development` | The Figma plugin whose plugin data holds the Weft source of each layer (`--plugin-id` overrides it). |
| `mcp.limits.*` | see `packages/mcp/README.md` | The MCP server's bounds on one call: `markupChars`, `dataChars`, `patches`, `patchesChars`, `projectChars`, `diagnostics`, `inputElements`. |
| `plugins.<name>` | none | Settings of a plugin or tool Weft does not know. Weft only checks that each is an object. |
| `plugins.open-design.tokensDir` | next to the design system | Where the Open Design plugin's `design-md` script writes the tokens it maps from a `DESIGN.md` or `tokens.css`. A file name like any other (`W703` when it is absolute or leaves the project), and an unknown key in `plugins.open-design` is `W702`. |

Directories are file names too: relative to `weft.json`, no trailing `/`. A missing directory is created.

Here a copy of the example sends pages and components to `out/`:

```console
$ cp -r examples/project weft-tour/shop
$ cd weft-tour/shop
$ cat > weft.json <<'EOF'
{
  "tokens": ["tokens/base.tokens.json", "tokens/brand.tokens.json"],
  "catalog": "catalog.json",
  "actions": ["cart.checkout", "cart.remove", "nav.back"],
  "data": "data.schema.json",
  "validate": { "mode": "strict" },
  "render": { "data": "sample.data.json", "outDir": "out" },
  "export": { "react": { "outDir": "out" } }
}
EOF
$ node ../../plugins/shared/scripts/render.ts screens/cart.weft
Wrote /path/to/weft/weft-tour/shop/out/cart.html
file:///path/to/weft/weft-tour/shop/out/cart.html
$ node ../../plugins/shared/scripts/export.ts screens/cart.weft
Wrote /path/to/weft/weft-tour/shop/out/cart.jsx
$ cd ../..
```

The page shows the sample cart from `render.data`, with the brand layer's tokens. In Claude Code and Cursor, `/weft:render`, `/weft:export` and `/weft:import` run these same scripts.

## When the file is wrong

A project file never crashes a tool. Each problem is a diagnostic that points into the file with a JSON Pointer:

```console
$ echo '{"tokens": ["tokens/base.tokens.json", "../outside.json"], "validate": {"mode": "loose"}}' > weft-tour/broken.json
$ weft validate weft-tour/shop/screens/cart.weft --project weft-tour/broken.json; echo "exit $?"
weft-tour/broken.json:#/tokens/0 W704 The file "tokens/base.tokens.json" cannot be read.
weft-tour/broken.json:#/tokens/1 W703 The file name "../outside.json" is absolute, leaves the project directory or is malformed.
weft-tour/broken.json:#/validate/mode W701 "validate.mode" must be "strict" or "lenient".
weft-tour/shop/screens/cart.weft:6:46 W306 Token "space.sm" does not exist.
weft-tour/shop/screens/cart.weft:8:11 W401 <rating> is not in catalog weft-core 0.1.0. — use a catalog component, or an extension named x-<vendor>-rating
weft-tour/shop/screens/cart.weft:8:80 W306 Token "color.star" does not exist.
weft-tour/shop/screens/cart.weft:10:31 W203 "ghost" is not an allowed value.
weft-tour/shop/screens/cart.weft:18:38 W306 Token "space.md" does not exist.
exit 1
```

- A file that cannot be read (`W704`) or a name that leaves the folder (`W703`) is left out; the rest of the project still applies, which is why the screen's own problems follow.
- A setting of the wrong type (`W701`) is an error. The tool would otherwise run with a default you did not choose.
- An unknown key (`W702`) is only a warning, so a file written for a newer Weft still works. A misspelled key shows up the same way: `"colour"` in `render` prints `W702 Unknown key "colour" in "render".`
- The render, export and import scripts and the MCP server refuse to work with a project that has errors, and write nothing.

`schemas/weft.schema.json` is the JSON Schema of the file. Point `$schema` at it and your editor completes every key and flags these mistakes as you type.

## Limits

- One project per screen. A project cannot include another one, and there are no shared fragments (a header or a card kept in its own file) yet; their design waits for approval.
- `render.data` and `render.tokens` apply to the render script and `write-page`; the MCP server's `weft_render` takes data as an argument.
- The Node front end of the `weft` command (`packages/core/src/cli.ts`) does not read projects. Use the Rust program.
