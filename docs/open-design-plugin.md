# The Open Design plugin

The `weft` plugin for [Open Design](https://open-design.ai) gives its agent the same Weft features as the [Claude Code](claude-code-plugin.md) and [Cursor](cursor-plugin.md) plugins, from the same bundled scripts, and one more: it turns a design system into Weft design tokens.

An Open Design plugin is a folder with one `SKILL.md` and an `open-design.json` next to it. There are no typed commands, so you ask the agent in plain words ("import this page into Weft", "export this screen as React") and the skill tells it which script to run.

| You ask for | The skill runs | Result |
| --- | --- | --- |
| A screen in Weft | The agent writes `.weft` from [AGENT-SPEC.md](../AGENT-SPEC.md) and checks it with `render.js` (strict) or the MCP tools | A valid `.weft` file |
| An HTML page as Weft | `import.js` | A `.weft` file and a loss table |
| A screen as React | `export.js` | A `.jsx` component |
| A preview | `render.js` | An HTML page in the project, which Open Design previews |
| A design system as tokens | `design-md.js` | A DTCG token file and a loss table |

## Install

From a clone of this repository:

```bash
od plugin install ./plugins/open-design
```

Open Design copies the whole folder into its plugin registry. The scripts need the `bash` capability, which Open Design treats as elevated: if it asks, grant it, or do it ahead of time with `od plugin trust weft --capabilities fs:read,bash`. (Open Design's code trusts a plugin installed from a local folder, its specification says such a plugin starts restricted; which one applies has not been tried.) `node` 24.2 or later must be on the `PATH` the agent's shell uses.

The weft MCP server (`dist/server.js`) is shipped in the folder, but a plugin manifest cannot start it for you (the [plugin's README](../plugins/open-design/README.md) says why). Without it the skill still works through the scripts. To use it, register it once in Open Design's MCP settings as a stdio server running `node <the plugin's folder>/dist/server.js`.

## Design systems

`design-md.js` takes a design system and writes a DTCG 2025.10 token file next to it, or into the folder that `plugins.open-design.tokensDir` names in the project's [`weft.json`](projects.md):

```bash
node dist/design-md.js DESIGN.md                    # YAML frontmatter, Google Labs format
node dist/design-md.js design-systems/minimal       # an Open Design design system: its tokens.css
node dist/design-md.js tokens.css tokens/brand.tokens.json
```

Open Design's own `DESIGN.md` files are prose, and keep their tokens in `tokens.css`, so for a folder the script reads `tokens.css` first. Google Labs' `DESIGN.md` carries its tokens as YAML frontmatter.

What maps, and what does not:

| In the design system | In the token file |
| --- | --- |
| Colors (hex, `rgb()`, `hsl()`, `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()`) | `color` tokens in their own color space, with a hex fallback where one exists |
| Lengths in `px` and `rem` (spacing, radius, sizes) | `dimension` tokens |
| Typography entries | `typography` tokens; a pixel line height becomes a multiplier of the font size, an `em` letter spacing becomes a length at that font size; each rewrite is a loss-table row |
| Font stacks, durations, easings, box shadows, numbers | `fontFamily`, `duration`, `cubicBezier`, `shadow` and `number` tokens |
| `var(--x)` and `{group.token}` references | Aliases that Weft resolves |
| Components, Markdown prose, other themes, `color-mix()` and other computed values, `em` lengths, `none` shadows, unreadable values | Not carried; each is a row of the loss table, which the script prints |

List the new file under `tokens` in `weft.json`, or pass it to `render.js` with `--tokens`, and screens use it.

The same mapping is the `@weft/design-md` package (`packages/design-md`), which other tools can use: `fromDesignMd(text)` and `fromTokensCss(css)` return `{ tokens, losses, problems }` and never throw.

## One bundle, three plugins

The sources of the scripts exist once, in `plugins/shared`. `moon run shared:build` bundles them once and writes the same `dist/` into `plugins/claude-code`, `plugins/cursor` and `plugins/open-design`; the Open Design copy of `AGENT-SPEC.md` lands in `references/`. A test rebuilds into a temporary folder and fails while any committed copy differs, and the plugin's own tests run every script from a copy of the folder.

It has been checked by tests against Open Design's published manifest schema and from a copy of the folder, but not yet inside a running Open Design; the plugin's README lists what remains.
