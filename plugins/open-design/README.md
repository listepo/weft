# Weft plugin for Open Design

Works on `.weft` files from [Open Design](https://open-design.ai): the same features as the [Claude Code plugin](../claude-code/README.md) and the [Cursor plugin](../cursor/README.md), from the same bundled scripts and MCP server, plus a design-system mapper.

| What the skill does | Script (in `dist/`) |
| --- | --- |
| Authors and checks `.weft` screens from the guide `references/AGENT-SPEC.md` (a generated copy) | the weft MCP tools when registered, else `render.js`, which validates strictly |
| HTML to a Weft screen; prints the loss table | `import.js` |
| A Weft screen to a React component | `export.js` |
| A Weft screen to an HTML page, written into the project where Open Design previews HTML | `render.js` |
| A design system (`DESIGN.md` or `tokens.css`) to DTCG tokens; prints the loss table | `design-md.js` |

The folder is the skill: `SKILL.md` and `open-design.json` at the root, `dist/` for the bundle, `references/` for the guide. See [the human guide](../../docs/open-design-plugin.md) for how to use it; this file is for people who work on the plugin.

## Install

```bash
od plugin install ./plugins/open-design
```

`node` 24.2 or later must be on the `PATH` of the agent's shell (the bundles use `import.meta.main`; on older Node they print that requirement and exit 2). Do not use a symlink: the installer rejects them.

## One bundle, three plugins

The sources exist once, in `plugins/shared`. `moon run shared:build` bundles them once and writes the identical `dist/` into `plugins/claude-code`, `plugins/cursor` and `plugins/open-design`, and the copy of `AGENT-SPEC.md` into each plugin's guide folder (here `references/`, because the skill is the folder root, not a subfolder). `plugins/shared/test/bundle.test.ts` rebuilds into a temporary folder and fails while any committed copy differs, and runs every script and the MCP server from a copy of each folder. Never edit `dist/` or the spec copy by hand.

## Why the MCP server is not in the manifest

`dist/server.js` is in the folder, but `open-design.json` does not declare it. `od.context.mcp[]` takes `name`, `command`, `args`, `env` and `url`, with no variable for the folder the plugin was installed into and no relative-path rule, and the daemon moves the installed folder (the registry names it after the manifest `name`, and the host stages a read-only copy of the skill folder as `.od-skills/<folder>-<digest>/` in the project), so no fixed `command` can reach it. In the code read, a declared server is only recorded in the apply snapshot; whether the daemon then starts it is **unverified** (the specification, section 5.3, says more than the code showed). Declaring an MCP server also implies the elevated `subprocess` capability. So the skill works through the scripts, and a user who wants the tools registers `node <folder>/dist/server.js` once as a stdio server in Open Design's MCP settings.

## Format notes

Checked on 2026-10-05 against the `nexu-io/open-design` repository, `main` at commit `53231d40b778d88eba23f35547bf99485d3ae9fc` (2026-09-30, `apps/daemon` 0.23.1), <https://github.com/nexu-io/open-design>. (`github.com/attentiondotnet/open-design`, which the task named, is a stale copy; the canonical repository is `nexu-io`. The 0.8.0 release post, <https://open-design.ai/blog/open-design-0-8-0-everything-is-a-plugin/>, could not be read for any manifest detail, so nothing below rests on it.) Every paragraph names the file it comes from; a statement from a document that the code did not confirm is marked.

- **Plugin format.** A plugin is a folder anchored to one portable `SKILL.md`, with an optional `open-design.json` sidecar that "never duplicates skill body content". `docs/plugins-spec.md` section 5; schema `docs/schemas/open-design.plugin.v1.json` (`specVersion`, `name` and `version` required; `name` matches `^[a-z0-9][a-z0-9._-]*$`; `compat.agentSkills[].path`; the `od` block: `kind`, `taskKind`, `mode`, `scenario`, `useCase`, `context`, `pipeline`, `inputs`, `capabilities`). Open Design's doctor adds cross-field rules in `packages/plugin-runtime/src/validate.ts` (a pipeline stage with `repeat` needs `until`; capabilities outside the v1 vocabulary `prompt:inject`, `fs:read`, `fs:write`, `mcp`, `subprocess`, `bash`, `network`, `connector[:id]` are warnings), which `test/plugin.test.ts` repeats where they apply.
- **SKILL.md.** Agent Skills frontmatter (`name`, `description`, `license`, `metadata`). Open Design reads it with a hand-written parser, not a YAML library (`packages/plugin-runtime/src/parsers/frontmatter.ts`): flat keys, one level of nesting, inline arrays, no anchors or folded scalars. This plugin's `SKILL.md` was run through that file once, ad hoc, and parsed to the expected fields; the committed test checks the frontmatter stays in that subset.
- **Install from a local folder.** `od plugin install ./folder` (`docs/plugins-spec.md` section 7.2) copies the whole folder into the daemon's registry, rejecting symlinks and path traversal, with a 50 MiB cap (`apps/daemon/src/plugins/installer.ts`, `DEFAULT_MAX_BYTES`); the destination folder is chosen by the manifest `name`, not the directory name (same file). The test checks no symlink, no `..` and the size against that cap.
- **Where the scripts run from.** The system prompt advertises the staged skill folder `.od-skills/<folder>/` as the **Skill root** (`apps/daemon/src/prompts/system.ts`, `apps/daemon/src/skills.ts`: "Skill root (relative to project)"). There is no host variable, so `SKILL.md` tells the agent to run `node "<Skill root>/dist/<script>.js"` from the project, and the tests run each script from a copy placed at that kind of path.
- **No commands.** Open Design plugins have no typed commands; a `.claude-plugin` folder is read through a compatibility adapter (`packages/plugin-runtime/src/adapters/claude-plugin.ts`) but a command or hook is not registered by it (`docs/plugins-spec.md`, in its capability rules, says `.claude-plugin` hooks imply `subprocess`). The skill therefore stands in for the three typed commands of the other plugins.
- **Capabilities.** `prompt:inject` always; `fs:read` stages plugin assets into the project; `bash` and `subprocess` are elevated and must be listed explicitly in a grant, never `all` (`docs/plugins-spec.md` section 9.1). The manifest asks for `prompt:inject`, `fs:read` and `bash`.
- **Trust of a local install.** `apps/daemon/src/plugins/trust.ts` makes a `local` install `trusted`; `docs/plugins-spec.md` (the summary and section 9) says a local plugin starts `restricted`. The two disagree. **Not tried.**
- **Preview.** A `.html` file in the project is what Open Design lists and previews. This is a reading of the product, not of the code: **unverified**.

### DESIGN.md, in the two forms the ecosystem uses

- **Google Labs DESIGN.md** (alpha): `docs/spec.md` of <https://github.com/google-labs-code/design.md> at commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd` (2026-07-27, checked 2026-10-05). YAML frontmatter with `name`, `colors`, `typography`, `rounded`, `spacing`, `components`, a token syntax "inspired by" DTCG 2025.10 with `{path.to.token}` references, and a Markdown body. Its examples (`examples/*/design_tokens.json`) keep `letterSpacing` in `em` and `lineHeight` in `px`, which is not valid DTCG 2025.10.
- **Open Design design systems**: `docs/design-systems.md` and the folders under `design-systems/` in the repository above (154 of them at that commit). Each holds `DESIGN.md` (prose in numbered sections, no frontmatter), a `manifest.json`, and `tokens.css` (one `:root` block of custom properties; the canonical token source). The mapper therefore reads `tokens.css` for an Open Design folder.

### DESIGN.md and tokens.css to Weft tokens

`packages/design-md` (`@weft/design-md`) does the mapping, `plugins/shared/scripts/design-md.ts` is its command line, and `packages/design-md/test/` holds its cases. Targets are DTCG 2025.10 (<https://www.designtokens.org/tr/2025.10/format/>, which Weft's token loader reads). The loss table is part of every result.

| Source | Weft token | Loss row |
| --- | --- | --- |
| Color: hex, `rgb()`, `hsl()`, `hwb()`, `lab()`, `lch()`, `oklab()`, `oklch()`, `transparent` | `color`, with the color space, components, alpha and a hex fallback | none; a named color (`red`) or an unquoted `#fff` (YAML reads it as a comment) is `unsupported-value` |
| Length in `px` or `rem`; a bare `0` | `dimension` | `em`, `%`, `pt` and other units: `unsupported-value` |
| Typography entry | `typography` | `lineHeight` in `px`: becomes a multiplier of `fontSize` (`converted`); `letterSpacing` in `em`: becomes `px` at that font size (`converted`); a missing weight (400), line height (1.2) or letter spacing (0px) is filled because DTCG requires them (`converted`); other properties: `unsupported-value` |
| CSS font stack, `ms`/`s`, `cubic-bezier()` and keyword easings, unitless number, `box-shadow` | `fontFamily`, `duration`, `cubicBezier`, `number` (`fontWeight` for a weight), `shadow` | `none` shadow, `calc()`, `color-mix()`, `light-dark()`: `unsupported-value` |
| `{group.token}` and `var(--x)` | alias, in the target's group | a reference to nothing or a loop: `unresolved-alias`, the token is left out |
| `components:` | nothing | one `component` row per component |
| Markdown body | nothing | one `prose` row |
| Blocks other than `:root` (`[data-theme]`, `@media`) | nothing | one `theme` row each, with the number of properties |
| Unknown top-level sections | nothing | `unsupported-section` |
| A token name with a dot, a brace or a leading `$` | nothing | `invalid-name` |

Group names follow the source for a DESIGN.md (`colors`, `typography`, `rounded`, `spacing`) so its `{colors.primary}` references stay as written. For `tokens.css` the value's shape sets the type and the name the group (`colors`, `spacing`, `rounded`, `fontSizes`, `sizes`, `lineHeights`, `fontWeights`, `numbers`, `fontFamilies`, `shadows`, `duration`, `easing`). Weft's core catalog reads only `dimension` tokens (for `gap`), so the other groups are for screens that reference tokens and for export.

Inputs are untrusted: sources over 1,000,000 characters are refused, YAML aliases are refused (`maxAliasCount: 0`), nothing throws, and a property test runs random text through both mappers and loads the result with the token loader. Fixtures are copies of Open Design's `design-systems/minimal` and Google Labs' three examples, with their notices in `packages/design-md/test/fixtures/NOTICE` (both Apache-2.0).

### weft.json

The mapper's one setting is `plugins.open-design.tokensDir` in `weft.json` (SPEC section 10.6): a folder name inside the project where `design-md.js` writes its token file when no output path is given. The project loader checks it like any other file name: an absolute path, a `..` segment or a non-string is `W703` or `W701` and the script stops with exit code 1, and an unknown key in `plugins.open-design` is a `W702` warning. An argument always wins.

## Tests

`test/plugin.test.ts` runs without Open Design:

- `open-design.json` against Open Design's own schema (`test/schemas/open-design.plugin.v1.json`, Apache-2.0, vendored with its notice in `test/schemas/NOTICE`, whitespace formatted only), plus the specification's cross-field rules and the capability vocabulary, and that a wrong manifest is refused;
- `SKILL.md` frontmatter and every script path it names, no host variable, a real (not linked) copy of the guide;
- the folder as the installer takes it: no symlink, no `..`, under the size cap, the bundle present;
- every script, and the MCP server, from a copy of the folder placed at `.od-skills/…/` of a scratch project, including `design-md` on Open Design's `minimal` design system and on a Google Labs example, and the exit codes 1 and 2.

## Not checked yet

These need a running Open Design and have not been done. Nothing was installed into a real Open Design or agent configuration.

- `od plugin install ./plugins/open-design` succeeds, and the doctor (`od plugin doctor`) is clean; Open Design's own validator was not run (its package needs `zod` 3.25 and the repository's workspace install, and only the frontmatter parser, which has no imports, was run).
- The agent finds the Skill root, resolves `dist/…` from it, and the project directory is where it runs the scripts from.
- The `bash` capability is granted on a local install, and what the first run asks for.
- A `.html` page the skill writes is previewed by Open Design's file view.
- The weft MCP server registered by hand in Open Design's settings starts and lists its six tools.
- Whether a manifest-declared MCP server (`od.context.mcp[]`) could host the server after all.
