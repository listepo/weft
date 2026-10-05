---
name: weft
description: Design screens as Weft (.weft) files, the strict UI markup an agent can write, validate and patch. Import an HTML page into Weft, export a screen as a React component, preview it as a page, and map a DESIGN.md or tokens.css design system to Weft design tokens. Use when the user wants a screen in Weft, or to convert between Weft, HTML and React.
license: Apache-2.0
metadata:
  author: Ivan Tugay
  version: "0.1.0"
---

# Weft

Weft describes UI as strict markup with a closed vocabulary, so a screen can be checked before anyone sees it. This skill authors `.weft` screens, converts HTML to Weft and Weft to React, renders a screen as a page, and turns a design system into Weft tokens.

## How to run the scripts

The scripts are bundled in `dist/` of this skill's folder. The folder is the **Skill root** named at the top of your instructions (`.od-skills/<folder>/` in the project, read-only); there is no environment variable for it, so run them by that path from the project directory, never from inside the skill folder:

```bash
node "<Skill root>/dist/<script>.js" <arguments>
```

Node 24.2 or later must be on the `PATH`; an older Node prints that requirement and exits with 2. Every script refuses to replace an existing file unless the user asked for it and you pass `--force`, and ends with an exit code: 0 done, 1 the input has errors (printed as `file:line:col code message`, and nothing was written), 2 a usage or file problem. Report diagnostics as they are; do not edit generated output to hide them.

Each script works in the project of its input (the first `weft.json` in the input's folder or above, `--project <file>` for another, `--no-project` for none). That project supplies the catalog, tokens, actions and data schema, and its settings stand in for arguments that are not given.

## Write a screen

Read [references/AGENT-SPEC.md](references/AGENT-SPEC.md) before you write or change any Weft markup, and follow it for patches and for every diagnostic code. Nothing outside the catalog exists, so look components up instead of guessing.

If the `weft` MCP server is registered (see the README of this plugin), use its tools: `weft_primer` (the syntax, first), `weft_catalog`, `weft_validate` (strict before you answer), `weft_format`, `weft_patch` (change a screen without rewriting it) and `weft_render`. They never touch files: read the `.weft` file yourself, pass its text as `markup`, write the result back.

Without the server, write the file and run `render.js` on it: it validates in strict mode, prints every diagnostic with its hint and exits with 1 when the screen is not valid. Fix each error from its hint and run it again.

Write screens into the project folder, not into the skill folder.

## Preview a screen

```bash
node "<Skill root>/dist/render.js" <screen.weft> [out.html] [--data data.json] [--tokens tokens.json]
```

`--data` is the sample data the bindings read; `--tokens` is a DTCG tokens file or resolver (default: the project's tokens, else the catalog's defaults); `--appearance light` or `dark` picks a side of a resolver's light and dark themes (default: `render.appearance`, else its default context). Without an output path the page is written next to the screen as `<name>.html`, so keep it in the project folder where Open Design lists and previews HTML files. The script prints the path and a `file://` URL. After the page is shown, look at it: if something is clearly off, say what and fix the screen instead of declaring success.

## Import an HTML page

```bash
node "<Skill root>/dist/import.js" <page.html> [out.weft]
```

Writes the page as a `.weft` screen mapped onto the project's catalog and prints a loss table. Show the table to the user in full: it lists what an HTML page cannot carry (bindings, actions, tokens, slots, hidden elements), which is what to add back. Then validate and refine the screen as above.

## Export to React

```bash
node "<Skill root>/dist/export.js" <screen.weft> [out.jsx] [--name Component]
```

Writes `export default function Name({ data, actions, onChange })`: values are read from `data`, two-way fields call `onChange(path, value)`, host actions go through `actions`. `--name` sets the component name (default `WeftScreen`).

## Use a design system

```bash
node "<Skill root>/dist/design-md.js" <DESIGN.md | tokens.css | design-system folder> [out.tokens.json]
```

Maps a design system to a DTCG 2025.10 token file and prints a loss table. The input can be a Google Labs `DESIGN.md` (its YAML frontmatter), a `tokens.css` (its `:root` custom properties), or an Open Design design system folder (its `tokens.css`, else its `DESIGN.md`). Open Design's own `DESIGN.md` files are prose with no token block; the script says so and points at the `tokens.css` beside them.

The loss table is part of the result: components, prose, computed colors, other themes and values DTCG cannot hold are listed, not dropped silently. Show it in full. Then tell the user to list the new file under `tokens` in `weft.json` (or pass it as `--tokens` to `render.js`) so screens use it. The file goes next to its input, or into `plugins.open-design.tokensDir` of the project's `weft.json` when that is set.
