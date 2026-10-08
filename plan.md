# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 55% | Claude Code / claude-opus-5-5 |
| T28 | in progress | P2 | 3 | 75% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 60% | Claude Code / claude-opus-5-5 |
| T14.2 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T31 | in progress | P1 | 5 | 75% | Claude Code / claude-opus-5-5 |
| T32 | in progress | P2 | 2 | 85% | Claude Code / claude-sonnet-5-5 |
| T39 | in progress | P1 | 4 | 20% | Claude Code / claude-opus-5-5 |
| T73 | todo | P2 | 2 | 0% | |
| T68 | in progress | P2 | 4 | 0% | Grok Bot / grok |
| T69 | todo | P2 | 5 | 0% | |
| T12.3 | todo | P2 | 4 | 0% | |
| T16 | todo | P2 | 5 | 0% | |
| T17 | todo | P2 | 4 | 0% | |
| T41 | todo | P2 | 2 | 0% | |

### T8. Evaluation

Paused until T20–T25 land: the final runs use the Rust core and the stricter benchmark.

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.

Execution plan:

1. Repair cycle in `bench/src/run-tasks.ts`: when an edit reply is invalid (unparseable or format errors), send one follow-up prompt with the previous reply and the validator's diagnostics, and record first-try and after-repair validity and success separately. Same rule for every format, so the baselines get the same help. Cover it with a mocked-provider test in `bench/test/checkers.test.ts`.
2. Summaries and the rendered table show first-try valid, valid after repair, first-try success, success after repair, and mean output tokens.
3. Models: Claude Sonnet 5.5, Claude Opus 5.5, Claude Haiku 4.5 (`DEFAULT_MODELS`).
4. Raw results go into the repository: stop ignoring `bench/results/`.
5. Trial run (one screen, all formats, one model) to check cost and the harness; the creator reviews it before the full run.
6. Full edit and read runs on the three models; `run.ts tokens` again so the report has Anthropic token counts.
7. `bench/EVALUATION.md`: results against the done criteria (Weft first-try validity at least 95%, after one repair at least 99%), comparison with the baselines, failure analysis, continue/stop recommendation.
8. Verify with `pnpm run ci`.

### T28. Binding readback against inverted conditions

In the Bonsai edit smoke run (`login.e2`) the model was asked to disable Sign in while `$.busy` is true. It changed `{!$.email}` to `{!$.busy}` and kept the `!`. The markup is valid, but the condition is inverted. The HTML and JSX baselines got it right. Validation cannot see intent. The model can, if the core tells it in plain words what a binding means. Depends on T20.

1. `weft-core`: `explain(document)` reads every binding as a sentence, e.g. `button#submit disabled: while $.busy is false`. `explain_changes(before, after)` lists only the props, events and bindings that changed.
2. CLI: `weft explain <file> [--against <old-file>]`.
3. `AGENT-SPEC.md` repair loop: before answering, read back the changed bindings and compare them with the instruction.
4. Benchmark: an optional readback turn after a valid edit, the same for every format. This is a method change, recorded in `test.md`. Rerun `login` on Bonsai with 3 samples.

Done when the tests for `explain` pass, the CLI prints readbacks, and the rerun is in the `test.md` history.

Execution plan (steps 1–3; step 4 waits for the creator's go-ahead on the method change and a local LM Studio model):

- Core, new module `crates/weft-core/src/explain.rs` with exports in `lib.rs` only, so no other core module changes. `explain(document, catalog)` returns one `Readback { path, target, name, sentence }` per bound or token prop, event and `<each>`; `explain_changes(before, after, catalog)` returns `Change { target, name, kind, before, after }` for every prop (literals included), event and loop that was added, removed or changed, matching elements by id. Elements are named `kind#id`, or by their SPEC §6.1 diagnostic path when they have no valid id (reusing `path_segment`), so slots appear as `slot[name]`. Sentences never hide a negation: a negated binding reads `true while $.busy is falsy (NOT $.busy)`, a plain binding on a boolean prop `true while $.busy is truthy`, other bindings `reads $.x` with `; user input writes $.x` on writable props, tokens `design token space.md`, events `runs action auth.submit`, loops `repeats its children once per item of $.todos, as $todo`. Pure, no I/O; unit tests named as claims in the module.
- CLI: `weft explain <file> [--against <old-file>] [--catalog <file>]` in `crates/weft-cli`, one line per readback or change on stdout; diagnostics with errors in either file print as in `validate` and exit 1; usage and I/O failures exit 2. End-to-end cases in `crates/weft-cli/tests/cli.rs`, including the `login.e2` inversion.
- `AGENT-SPEC.md` §4: read back the changed bindings before answering (`weft explain --against` when a tool is available, otherwise read `!` as NOT) and compare each with the instruction; one checklist line. `bench/test/agent-spec.test.ts` must stay green.
- Verify: `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`, `cargo nextest run --workspace`, the binary on a corpus screen, `moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Progress: steps 1–3 are done. `explain` and `explain_changes` live in `crates/weft-core/src/explain.rs` with unit tests, `weft explain` in `crates/weft-cli` with end-to-end tests, and `AGENT-SPEC.md` §4.1 holds the readback loop. Remaining: step 4 (the optional readback turn in the benchmark, recorded in `test.md`, and the `login` rerun on Bonsai with 3 samples), which waits for the creator's go-ahead on the method change and a local LM Studio model.

### T14. Figma round trip and plugin

Convert Weft to Figma and back without loss, and ship a Figma plugin. Approved scope:

- **Library:** generate a Weft component library in Figma from the catalog and the design tokens (one component set per catalog kind, variants for its enum props, Figma variables for the tokens). Screens are built from instances of this library.
- **Weft to Figma:** every node becomes an auto-layout frame or a library instance, and its Weft source (kind, props, bindings, data paths, ids) is stored in the node's plugin data.
- **Figma to Weft, lossless:** a screen that was never edited comes back byte-identical after `weft fmt`. A designer's edits come back too: text, order, added or removed library instances, variants, and visual edits (spacing, colors, sizes, radii). A visual edit maps to a token when it matches one and otherwise to a style override, which needs a format extension (`AGENT-SPEC.md`, the parsers in TS and Rust, the catalog). Its design goes to the creator before it is built.
- **Foreign layers:** layers that did not come from Weft (vectors, images, free frames) convert lossily, with a loss table, as before: Figma layers carry no semantics.
- **Where it runs:**
  - a Figma plugin: open or paste a `.weft` file to build frames; select a frame to export `.weft`.
  - CLI `weft figma pull`: reads through the REST API (`GET /v1/files/:key/nodes`). The REST API is read-only, so building frames stays in the plugin.
  - MCP / Claude Code tools (with T30), working through the Figma MCP server.

Done when every corpus screen survives Weft to Figma to Weft byte-identical, a scripted set of designer edits comes back with the expected Weft diff, and the plugin, CLI command and MCP tools pass their tests in `moon ci`.

Execution plan, stage 1 (library, both conversions, plugin; the CLI command, the MCP tools and style overrides are later stages):

1. `packages/figma` (`@weft/figma`), pure TypeScript against a narrow interface of the Plugin API subset it uses. A type test checks that the official `@figma/plugin-typings` types satisfy that interface, so the real `figma` object and the in-memory fake (`test/fake-figma.ts`) are interchangeable.
2. Library: one page with a component set per catalog kind (variants for its enum props and `state`, with an `(unset)` value where the prop has no default) and one variable collection holding the design tokens (dimensions as FLOAT in px, colors as COLOR). Kinds and tokens are recorded in plugin data, so a second run finds and reuses them.
3. Weft to Figma: kinds whose content is `none` or `text` become library instances, the rest auto-layout frames (`stack` direction, gap bound to its token variable, align, wrap; `grid` columns); `<each>` and named slots become marked frames; text children become text layers. Each node keeps its Weft source (kind, id, props, events) in plugin data, and the root keeps the document version.
4. Figma to Weft: a node is rebuilt from its stored source, and the Figma state is compared with the state that source would build. What is equal keeps the source as written (so an unedited screen is byte-identical); what differs is the designer's edit: text, label, child order, removed nodes, variant props, hidden, stack direction/align/wrap, and a gap that matches a token (bound variable or equal value). Copied nodes get fresh ids. New library instances become new nodes with generated ids and stand-ins for required props. Visual edits with no Weft prop or no matching token are reported as losses.
5. Foreign layers (text, free or auto-layout frames, groups, images, vectors) convert lossily with a loss table in the shape of `@weft/from-aria`; reuse its id, literal and limit helpers.
6. `plugins/figma`: `manifest.json`, a thin `code.ts` over `@weft/figma` (build from pasted or opened `.weft`, export the selected frame), `ui.html`; bundled into one file with Vite, with a test that the bundle builds.
7. Tests (Vitest): every corpus screen round-trips byte-identical through the fake; a scripted set of designer edits yields the expected Weft; foreign layers yield the expected losses; library generation is idempotent.
8. `packages/figma/README.md` with the Plugin API facts the code relies on, each with its official URL and the date checked. `docs/figma-style-overrides-design.md`: the style-override proposal for the creator (not built).
9. Wire into pnpm, moon, `tsconfig.json`, `toolchain.md`; verify with `mise exec -- moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Progress (stage 1 done, on branch `t14-figma`):

- Steps 1–9 are done. `@weft/figma` builds the library, both conversions, foreign layers and the plugin message handler. `plugins/figma` holds the manifest, a thin `code.ts`, `ui.html` and the Vite bundle. The bundle is tested by running it in a bare VM over the fake.
- Tests:
  - all 12 corpus screens round-trip byte-identical with no losses;
  - 19 scripted designer edits match the expected Weft patches and loss kinds;
  - building the library twice reuses it.
- `from-aria` now exports `freshId`, `literal`, `fillRequired` and the limit helpers, which both importers share.
- After T22 (WebAssembly core), the plugin is split. Figma's main thread has no WebAssembly, so it only builds and reads layers (`code.js`, with the core's loader stubbed). The UI iframe parses and serializes with the core inlined in `ui.html`. A harness runs both halves, the main thread without WebAssembly. The main thread interprets no values: the UI formats `text` and `label` for the build, and `finishRead` reads typed text with the core's `readValue` (a new `weft-wasm` export), so the value rules have one implementation.
- `docs/figma-style-overrides-design.md` waits for the creator's approval.
- Remaining:
  - style overrides: after approval, the format extension in TS and Rust, then the Figma mapping for colors, radii and padding (only `gap` maps to a token today);
  - a pull from a real Figma file, to confirm the REST facts marked unverified in `packages/figma/README.md` (rotation in radians, number precision);
  - MCP / Claude Code tools (with T30);
  - a check of the plugin in the real Figma app, including the inline module script and WebAssembly in its UI iframe;
  - the open questions in the report: single components for kinds without variants, `state` as a variant axis, the manifest id, and plugin data on duplicate/detach.

Execution plan, stage 2 (this round; style overrides stay out until the creator approves them):

1. T14.1: the pull over the REST API — done (`done.md`).
2. MCP / Claude Code tools through the Figma MCP server. The Figma MCP server's `use_figma` tool runs Plugin API code, but its own description says `setPluginData` is not supported there, and plugin data is readable only by the plugin that wrote it. The creator chose to move the Weft source to shared plugin data, which any plugin and the REST API (`plugin_data=shared`) can read: T14.2 (card below).

### T14.2. Weft source in shared plugin data

Move the Weft source the Figma plugin stores on layers, components, pages, variables and collections from private plugin data (`setPluginData`, readable only by the plugin that wrote it) to shared plugin data (`setSharedPluginData` under the namespace `weft`, readable by any plugin, the Figma MCP server's `use_figma` and the REST API with `plugin_data=shared`). Files built before the change keep working. Done when a build writes only shared data, a file with only private data reads back the same and is migrated, and the REST pull reads shared data first.

Execution plan:

1. Check the shared plugin data API (namespace rules, size, empty string removes) and the REST `sharedPluginData` field against Figma's docs; record them with URL and date in `packages/figma/README.md`.
2. `packages/figma/src/data.ts`: `NAMESPACE = "weft"` (Penpot's namespace too) and `dataOf(node)`, a `PluginData` view that reads shared data first and falls back to private data; a value found only in private data is moved to shared data when it is read (on load) and private data is cleared when a key is written. Migration is best effort: a node that cannot be written (a REST node, a read-only file) still reads.
3. Route every plugin data access in `packages/figma/src` (`build.ts`, `layer.ts`, `library.ts`, `modes.ts`) through `dataOf`; add the shared methods to `FPluginData` (checked against `@figma/plugin-typings` by `api-types.test.ts`).
4. REST: ask `plugin_data=shared,<plugin id>`; `rest.ts` reads `sharedPluginData.weft` first, then `pluginData[<plugin id>]` for frames built before the change. `import.figma.pluginId` stays for those frames.
5. Fake Plugin API and fake REST server: shared data per namespace. Tests: a build writes only shared data (layer snapshots unchanged); a frame whose data was moved back to private reads back byte-identical and ends with only shared data; the library and token collection are found from private data; the pull reads shared data and still reads a private-only frame with the plugin id.
6. Docs: `packages/figma/README.md`, `plugins/figma/README.md`, SPEC or docs where the storage location is described; rebuild the plugin bundles.

### T31. Project file and shared resources

Several `.weft` screens share one set of resources through a project file, `weft.json`, which tools find by walking up from the screen, like `tsconfig.json`. Screens themselves do not name what they use. Approved scope:

- **Tokens:** one or more DTCG files, layered in order (base, then theme, then brand), the later file overriding the earlier one.
- **Catalog:** the core catalog plus project extensions (own kinds, props and variants), declared once for every screen.
- **Actions and data schema:** one list of action names and one description of the data model, against which every screen is validated (binding paths and their types).
- **Fragments:** repeated blocks (header, footer, card) kept in their own files and placed in screens by reference. This is the one part that changes the format: a new element (for example `<use>`), with its rules for ids, slots, bindings and patches. Its design goes to the creator before it is built, and it lands in `AGENT-SPEC.md`, `SPEC.md`, both parsers (TypeScript and Rust) and the differential fixtures together.
- **Tools:** the CLI, the MCP server, the renderer, the Claude Code plugin (T30) and the Figma work (T14) all read the project file, and an explicit argument still overrides it.

Done when a corpus project of several screens with layered tokens, a catalog extension, an action list, a data schema and a shared fragment validates and renders through the CLI and the MCP server, the TypeScript and Rust results match, and a broken project file is reported with a diagnostic, never a crash.

Execution plan (part A is built now; part B, fragments, is design only and waits for the creator):

1. Spec first. `SPEC.md` gets a Project section: `weft.json` found by walking up from the screen (an explicit argument overrides it), its members (`tokens` list of DTCG files layered in order, `catalog` extension file, `actions` list, `data` schema file), paths relative to the project file and never leaving its directory, token layering (later token wins, aliases resolved after the merge, as the DTCG resolver module orders sets), the catalog extension rules (new kinds are added; an entry for a core kind merges into it; the merged catalog may only widen the core catalog by the version rules of §8, anything those rules call major is a conflict), the data schema (a JSON Schema 2020-12 subset), and new codes: `W315`/`W316` for binding paths and types against the data schema, `W7xx` for project problems. `AGENT-SPEC.md`, the MCP primer and tool descriptions follow. Sources go into `research.md`.
2. Core, TypeScript and Rust (`packages/core/src/data.ts`, `crates/weft-core/src/data.rs`): `compileDataSchema` and `checkData(document, …)` as a separate pass, so `ValidateOptions` and `ParseOptions` keep their fields (T22 and T29 construct them); new codes in both registries.
3. Catalog, TypeScript and Rust (`packages/catalog/src/project.ts`, `crates/weft-catalog/src/project.rs`): pure `resolveProject(content)` and `loadProject(text, read)` with an injected reader; token layering, catalog extension with conflict diagnostics, action list, data schema. Never throws or panics.
4. File reading stays at the edges: `packages/catalog/src/node.ts` (walk up, read files) for Node tools; the Rust CLI walks up itself and gets `--project`; `render-react`'s `write-page` reads the project too. The MCP server reads no files: tools take an optional `project` argument with the contents, and `createServer` takes one from the host.
5. Differential fixtures: data checks in the core fixture, project resolution in the catalog fixture; regenerate with `WEFT_UPDATE_FIXTURES=1`.
6. An example project (`examples/project/`, outside `corpus/` because corpus tests read every directory there as a screen) with several screens, layered tokens, a catalog extension, actions and a data schema; CLI and MCP tests on it and on broken project files.
7. Part B: `docs/fragments-design.md`, a proposal for `<use>` fragments, for the creator to approve.
8. Verify with `mise exec -- moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Scope extension (creator): everything that can be configured is configurable through `weft.json`. The file becomes a config with optional sections, each with documented defaults; an explicit CLI or tool argument overrides `weft.json`, which overrides the defaults.

9. Spec: `SPEC.md` §10.6 lists the sections — `validate` (`mode`), `format` (`write`), `render` (`data`, `tokens`, `outDir`), `export.<target>` and `import.<target>` (`react` and `html` today, `outDir`), `mcp` (`limits`), `plugins` (free-form, one object per plugin) — their defaults, the precedence, and how a later task adds its section (T34 SwiftUI, T35 web targets, T14 Figma: `export.swiftui`, `export.html`, `export.solid`, `export.figma` with a rem base and a token strategy, `import.figma`, `import.penpot`). An unknown key is `W702`, now always a warning; a wrong type is `W701` and the default applies.
10. `schemas/weft.schema.json` (JSON Schema 2020-12), referenced from `$schema`; a test keeps it and the loader in step.
11. Loader (Rust, through weft-wasm for TypeScript): the sections parsed into `Project.settings`, never throwing.
12. Tools: the Rust CLI (`validate.mode` with `--lenient` to override, `format.write` with `--print`, `explain` reads the project's catalog), `write-page` (`render.data`, `render.tokens`), the Claude Code plugin scripts (project resources plus `render`, `export.react`, `import.html`), the MCP server (`weft-mcp --project <file>` read once at start by the host: resources, `validate.mode` as the default of `strict`, `mcp.limits`; the `project` tool argument never changes limits).
13. `AGENTS.md`: every new tool option gets a `weft.json` key in the same change.
14. Rebuild `plugins/claude-code/dist`, merge `main`, full check.

Progress: part A and steps 9–14 are done (the TypeScript side runs on the Rust core through weft-wasm since T22). Left: the fragments proposal in `docs/fragments-design.md` waits for the creator; once approved, fragments are built (format, both parsers, patches, fixtures) and the example project gains a shared fragment, which the done criteria ask for.

### T32. Claude Code plugin from GitHub

The T30 plugin works only when its marketplace is added from a local clone: Claude Code copies just the plugin folder into its cache, and the `@weft/*` packages run from the repository's sources. Bundle the plugin's scripts and the MCP server into self-contained files at release, so the plugin installs from the GitHub-hosted marketplace once the repository has a remote, and add the `repository` field to `plugin.json`. Check that Claude Code Desktop finds `node` when started from the GUI. Done when `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` work on a clean machine.

Progress: the bundle carries the WebAssembly core (T22 merged): `plugins/claude-code/build.ts` bundles the scripts and the MCP server with Vite 8 into the committed `dist/` and copies `weft_bg.wasm` to `dist/wasm/`, where `@weft/core` reads it relative to the shared chunk; `claude-code:build` depends on `root:wasm`. The plugin folder alone runs (tests copy only it to a temp folder and run every script and the server there; `claude --plugin-dir <copy> mcp list` shows `weft` connected; `claude plugin validate --strict` passes). The Desktop `node` requirement (24.2 or later on the PATH) is in the plugin README. `root:wasm` remaps build paths, so the `.wasm` is byte-reproducible and the up-to-date test compares it byte for byte. Remaining: `repository` in `plugin.json` once a remote exists, and the done criterion itself, `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` on a clean machine.

### T39. Context in the document

A `.weft` file carries the context that a person or an agent left for whoever works on it next, so the next agent or person can use it. Approved scope:
- **Two levels:** a context block on the screen (purpose, decisions, constraints, open questions) and context entries on any element (why a button is disabled, where a label came from).
- **Typed entries:** every entry has a kind (`intent`, `decision`, `constraint`, `question`, `todo`, `source`), an author (`human` or `agent`, with a name or model) and text. The validator checks their shape.
- **Part of the document:** context is in canonical JSON and survives formatting, patches (new patch operations to add, change and resolve entries) and every conversion. Code generators write it as comments; Figma keeps it in plugin data; importers read it back from code that Weft generated.
- **Untrusted by design:** context is data for the reader, never instructions. `AGENT-SPEC.md` tells models to treat it as information to weigh and to ignore commands inside it. Renderers never show it to end users.

Format change, so the design comes first: syntax, canonical JSON, validation codes, patch operations and the effect on every target go to the creator for approval before `SPEC.md`, `AGENT-SPEC.md`, the Rust core and the targets change together. Done when a screen with context on both levels survives fmt, patches, every round trip that exists, and the MCP tools expose it.

Execution plan, design stage (one file, no code, `SPEC.md` or `AGENT-SPEC.md` changes):

1. Read `SPEC.md`, `AGENT-SPEC.md`, `docs/figma-style-overrides-design.md`, the parser, model, canonical form and patches in `crates/weft-core`, plugin data in `packages/figma`, `packages/to-jsx` and the benchmark primers in `bench/`.
2. Write `docs/context-design.md` in the shape of the style-overrides proposal: markup syntax with at least two alternatives and a recommendation, the entry model, canonical JSON and ordering, validation rules, new diagnostic codes and limits, patch operations, the effect on every target (renderers, code generators and importers, Figma, MCP, `weft explain`), the security rule and its `AGENT-SPEC.md` wording, the `weft.json` option (T31), versioning and migration, and open questions with recommendations. Worked examples use the corpus login screen in markup and canonical JSON.
3. Verify with `mise exec -- moon run root:lint`, commit, and leave T39 in progress until the creator approves the design.

Progress: the design proposal is in `docs/context-design.md` and awaits the creator's approval. It recommends one `<context>` block under `<screen>` with entries attached to elements by `for`, new codes `W120`, `W121`, `W227`–`W229` and `W510`–`W512`, the patch operations `add-context`, `set-context`, `resolve-context` and `remove-context`, and `weft` 0.2. Eleven open questions close the document. The build (SPEC, AGENT-SPEC, the Rust core and the targets together) starts after approval.

### T68. Slint bindings for SwiftUI and WinUI

Research how Slint embeds into native apps (the `slint::platform` custom platform, the software renderer, the window adapter, input and accessibility) and whether Slint has official bindings for SwiftUI (macOS, iOS) or WinUI 3 in `slint-ui/slint`, its code and its open pull requests. If official bindings exist, record where and use them. If not, build them as a separate project, `~/GitHub/listepo/apps/slint-bindings`, in the public repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), pushed to `main`: a Rust core (`slint-bindings-core`) and a C ABI (`slint-bindings-ffi`), a Swift package that hosts a Slint component in SwiftUI through `NSViewRepresentable`, and a WinUI 3 package for Windows. Weft consumes those bindings and does not keep its own embedding: the Weft desktop work builds on T67 (the Slint the screens become) and on this project. Done when the research with its sources is written down in that repository, and a Slint component generated by T67 shows and takes input inside a SwiftUI window on macOS through the bindings; the WinUI 3 half is checked on Windows.

### T69. Figma-like desktop design editor

A desktop design editor in the spirit of Figma for Weft documents, on macOS and Windows. Weft is the document format: an editor document is a set of Weft screens (`.weft`, canonical markup and JSON, SPEC §2–§3) with the project's catalog and tokens (SPEC §10), so whatever the editor saves validates, formats and converts with the existing tools, and agents edit the same files through the CLI and MCP server.

**Prerequisite: the Slint bindings.** The macOS (Swift) and Windows (WinUI 3) integration MUST use the Slint bindings of T68: the project `~/GitHub/listepo/apps/slint-bindings`, repository `listepo/slint-bindings` (https://github.com/listepo/slint-bindings), with its crates `slint-bindings-core` (custom `slint::platform`, software renderer, window adapter, input) and `slint-bindings-ffi` (C ABI), its `swift/` package (a Slint surface in SwiftUI through `NSViewRepresentable`) and its `windows/` package (the same surface in WinUI 3). No separate or ad-hoc embedding lives in Weft. A gap found there (input, IME, focus, DPI, accessibility, rendering, lifecycle) is fixed in slint-bindings and consumed from it, never worked around here. Depends on T68, on T67 (Weft → Slint) and on T67.2 (the rest of the catalog and the read-back of edited Slint).

**Architecture split.**

| Part | Built with | What it holds |
| --- | --- | --- |
| Editing surface (shared) | Slint, embedded through slint-bindings, one code base for both platforms | The canvas (pan, zoom, rulers, grid, snapping), the rendered screens and frames (each Weft screen rendered through the T67 converter), selection, hover and resize handles, the layers tree of the edited design, the property inspector of the selected element (props, bindings, tokens, states, events, slots), the component and catalog panel (insert a kind, frames and component instances), and any other editing widget that must behave the same on both platforms |
| App shell (native) | SwiftUI and AppKit on macOS, WinUI 3 on Windows | Windows and tabs, menus and the menu bar, toolbars, file open and save dialogs, settings, the document browser and recent files, sidebars and panels outside the editing surface, notifications, the platform's undo menu items and keyboard shortcuts wired to the shared core |
| Document core (shared) | Rust, the Weft crates | Loading and saving `.weft` files and projects, validation (`weft-core`), every edit as a Weft patch (SPEC §7) so undo and redo are patch inverses and agents' patches apply the same way, the catalog and tokens (`weft-catalog`), and the Weft ↔ Slint conversion (`weft-slint`) that renders the screens and reads edits back |

The native shell and the shared surface talk only through the slint-bindings C ABI and a small editor API on top of it (open, save, select, apply patch, events back to the shell); neither side reaches into the other's toolkit.

**Milestones** (each becomes a T69.x task with its own plan when claimed, within the task size rule):

1. Skeleton: a native window on each platform hosting one Slint surface through slint-bindings, showing one corpus screen rendered by `weft-slint`; open and save through the native file dialogs.
2. Canvas: several screens as frames, pan and zoom, rulers, selection and hover from the Weft element tree, keyboard navigation.
3. Editing: move, reorder, insert and delete elements, resize within the layout rules, the layers tree and the property inspector writing Weft patches; undo and redo; validation diagnostics shown on the canvas and in the inspector.
4. Components and tokens: the catalog panel, catalog-extension kinds as components, design tokens from the project (spacing, colours, typography) in the inspector, the light and dark appearances.
5. Round trip with the outside: the saved files stay canonical (`weft fmt` leaves them unchanged), the existing exporters (SwiftUI, web, Slint, A2UI) run from the native menus, and files edited by agents or by hand reload without losing the selection where ids still exist.
6. Packaging: signed builds for macOS (arm64 only) and Windows x86_64, with the toolchain rows in `toolchain.md`.

**Acceptance criteria.**

- The same Slint editing surface code runs inside the SwiftUI app on macOS and inside the WinUI 3 app on Windows through slint-bindings, with no editor-specific embedding code in either shell.
- App chrome (windows, menus, toolbars, dialogs, settings, document browser) is native on each platform and contains no Slint.
- Every corpus screen opens, renders, and saves back byte-identical when nothing was edited; a scripted set of edits (move, insert, delete, change a prop, bind a prop, apply a token) produces the expected Weft diff, and the result passes strict validation.
- Undo and redo restore the exact previous document; edits are recorded as Weft patches.
- Tests: unit tests of the document core and the editor API, integration tests of the scripted edits over the corpus, and a check that every rendered screen's Slint compiles; the native shells have UI smoke tests on their platforms.

**Out of scope.** Multiplayer and real-time collaboration, cloud storage, accounts and sharing, comments and review threads, prototyping and interaction flows, vector drawing tools and boolean shape operations, image editing, plugins for the editor, Figma file import (T14 covers the Figma round trip), a web version (later, through Slint's WebAssembly build), iOS and Android, Intel Macs, and Linux.

Not started; nothing is built until the creator approves the plan of the first milestone.

### T73. CI workflow for the documented merge gate

There is no `.github/` in the repo; the documented merge gate (`moon run :test root:typecheck root:lint root:rust-test root:rust-lint root:runtimes`, README.md:50) runs only by hand — and publishing from GitHub (T32) will need it. Done means: the gate runs as a workflow on pull requests (and on main once the repo has a remote).

### T12.3. Constrained generation: benchmark

Measure whether constrained JSON beats free markup on validity and edit success, and what it costs in output tokens.

**Out of scope (all of T12).** Any change to the markup format or the validator. Streaming of constrained JSON (T11 covers partial markup). Grammar-level decoding (GBNF, regex) for markup. Making constrained JSON the format agents are told to use.

**Context.**
- The benchmark has no structured output: `bench/src/provider.ts` has `Provider.complete(prompt: string)` only (Anthropic Messages and batches, and an OpenAI-compatible `/chat/completions` endpoint for LM Studio). The formats are `weft`, `html`, `jsx` and `a2ui` (`bench/src/neutral.ts`, `formats.ts`); replies are taken from a code fence (`extractDocument`). The method is in `test.md` (strict validation, one repair prompt, 3 samples, a History row for every kept run); `bench/src/primers.ts` changes only as a method change.
- The repository records nothing about what each provider supports for structured output (recursive `$ref`, `anyOf` size, schema size limits). Those facts are checked when the task is claimed and written into `research.md` with sources.

**Scope.** `Provider.complete(prompt, { schema? })` for Anthropic and OpenAI-compatible servers; two new formats, `weft-json` (free canonical JSON) and `weft-json-constrained`, to separate "JSON instead of markup" from "constrained instead of free"; a JSON primer and an adapter that reuses the Weft neutral tree; corpus JSON derived from `screen.weft` at run time (no second committed copy); the same repair rule; `test.md` method sections, a trial run with its History row, and a results section in the T8 evaluation report.

**Done when.**
- A mocked-provider test shows that the schema is sent and that the reply is scored by the same checks as every other format.
- A trial run is in `test.md` History with its raw results in `bench/results/`, and the comparison (valid and success, first try and after repair, mean output tokens) for markup, free JSON and constrained JSON is in the evaluation report.

**Dependencies.** T12.1 and T12.2. T8 (the harness, models and evaluation report; it is in progress in `bench/src/run-tasks.ts`, so the two are sequenced). T28 step 4 (a readback method change to the same harness).

**Open questions for the creator.**
1. Three-way (with the free `weft-json` format) or two-way? Which models (Anthropic plus a local LM Studio model)? What budget? Part of T8's continue/stop decision or a separate report? (T12 question 5.)
2. If a provider cannot take the schema whole (size or recursion limits): a reduced profile per provider, or report the provider as unsupported? (T12 question 6.)
3. Narrow the schema with project data (token names, action names, data paths as enums) once the measurement shows whether it is worth it? T12.1 describes the catalog only. (T12 question 1.)

### T16. Layout vocabulary

Weft has two layout kinds today. `stack` lays children out in one line, with `direction`, `gap`, cross-axis `align` and `wrap`; `grid` has a fixed number of equal `columns` and a `gap`. Most of what a layout usually says has no form in Weft: how leftover space on the main axis is shared, which child grows and which hugs its content, inner spacing, a maximum width, and how a grid reflows on a narrow screen. Screens work around these gaps, and every importer drops them as `layout` losses. The task decides how much more Weft says without becoming CSS: a small closed set that every target (web, SwiftUI, Slint, A2UI, json-render, Figma, Penpot) can draw and read back, and nothing that only CSS can express. This changes the format, so the design comes first, as in T39.

**Context.**
- `SPEC.md` §5.1 holds the `stack`/`grid` rows and the `stack.align` note (T55 set its default: a row centres its children, a column keeps its host's layout). §2.2 holds the universal attributes; T52's tilt attributes are the precedent for a per-child attribute. §8: a new prop is a minor catalog change, a new or changed default is major. §9 loss tables: the SwiftUI importer drops `padding` and `frame`, Slint loses grid `row`/`col` placement, A2UI export loses `columns` and `wrap`, A2UI import notes `justify` other than `start` and `weight`.
- Catalog: `packages/catalog/src/core.ts` generates `packages/catalog/catalog.json`, embedded by `crates/weft-catalog/src/core.rs`; examples `packages/catalog/examples/stack.weft` and `grid.weft`; change classifier `crates/weft-catalog/src/diff.rs`.
- Generators: `crates/weft-web/src/html.rs` `layout()` and `base.css`; `crates/weft-web/src/jsx/` (React, SolidJS and Lit share one render plan, `_align` in `jsx/runtime.rs`); `packages/render-react/src/render.ts` `layoutStyle`; `crates/weft-swiftui/src/generate.rs` (`VStack`/`HStack` alignment, `LazyVGrid`, no wrap); `crates/weft-slint/src/generate.rs` (writes `spacing` only); `crates/weft-interop/src/a2ui/export.rs` and the json-render export (T13.1); `packages/design-tool/src/view.ts` `layoutView`, used by the Figma and Penpot builds (it draws `stretch` as `start`).
- Importers: `crates/weft-web/src/dom.rs` and `from_jsx/`; `crates/weft-swiftui/src/import/read.rs`; `crates/weft-slint/src/read.rs`; `crates/weft-interop/src/a2ui/import.rs`; `packages/design-tool/src/read.ts` and `foreign.ts`. Figma `primaryAxisAlignItems` and sizing modes are already typed in `packages/figma/src/api.ts`, Penpot `horizontalSizing`/`verticalSizing` in `packages/penpot/src/api.ts`.
- Corpus workarounds: `dashboard` separates the title and the update time with a `space.xl` gap in a wrapping row, right-aligns its footer with a column `align="end"`, and its 3-column `grid` never reflows; `wizard-step` cannot push Back and Next apart. `crates/weft-snapshots/tests/coverage.rs` fails while any catalog prop or enum value appears in no corpus screen, so new props need a corpus screen.
- `research.md` §9 lists "Layout without becoming CSS" as open, with no prior-art section. `docs/figma-style-overrides-design.md` proposes `style-padding` (not approved) and overlaps with this task.

**Scope.**
1. Design: a `research.md` prior-art section with URLs and dates (A2UI `justify`/`align`/`weight`, Figma auto layout hug/fill/fixed and min/max, Penpot flex/grid sizing, SwiftUI `Spacer`/`frame`/`layoutPriority`/`ViewThatFits`, Slint `alignment` and `*-stretch`, CSS flex/grid/container queries, Compose), and `docs/layout-design.md` in the shape of `docs/context-design.md`: the candidate vocabulary (main-axis distribution on `stack`, child sizing, padding, max width, grid reflow), markup and canonical JSON, catalog props versus universal attributes, token types, defaults, diagnostic codes, a mapping and loss table per target, versioning, and worked examples on `dashboard` and `wizard-step`. Nothing below starts until the creator approves the design.
2. Spec and catalog: `SPEC.md` §5.1 or §2.2 and the §9 mappings and loss tables; the `AGENT-SPEC.md` layout line and `packages/mcp/src/primer.ts`; `core.ts` and a regenerated `catalog.json`, the catalog examples, `docs/catalog-and-tokens.md`; for a new universal attribute or code, `crates/weft-core/src/rules.rs` and the differential fixtures.
3. Generators: static HTML and the base stylesheet, React/SolidJS/Lit, the reference renderer, SwiftUI, Slint, A2UI, json-render, and the Figma and Penpot builds through `design-tool`.
4. Importers: HTML/DOM, React/SolidJS, SwiftUI, Slint, A2UI and Figma/Penpot read the new vocabulary back, and the `layout` loss rows narrow.
5. Corpus and baselines: replace the workarounds in `dashboard` and `wizard-step`, or add a screen, so coverage holds; retake and review the insta snapshots, the Figma/Penpot layer trees, and the `packages/visual` web and SwiftUI baselines.
6. `weft.json`: no key is expected; if the design adds a tool option (for example breakpoints that are not tokens), it gets a §10.6 row and a `settings.rs` entry in the same change.

**Out of scope.** Arbitrary CSS (lengths outside tokens, absolute positioning, per-side margins, z-index). Visual style (fill, stroke, radius), which belongs to the style-overrides proposal. Animation. Changing T55's cross-axis default. `bench/src/primers.ts`, which changes only as a method change recorded in `test.md`.

**Done when.**
- The creator has approved the design.
- The new props or attributes validate in strict and lenient mode, and their codes are in `AGENT-SPEC.md` (`bench/test/agent-spec.test.ts` passes).
- A corpus screen that uses each one passes the `packages/visual` cross-target comparison (React, SolidJS, the reference renderer) and has a reviewed SwiftUI baseline.
- Every target either maps each new form or lists it as a loss in §9; generated code with its source comment removed reads back to the same props on every importer whose row says "maps"; the Figma and Penpot round trips keep it.
- The full check exits 0.

**Dependencies.** T55 and T52 (done) are the precedents. Coordinate with T39 (both may bump `weft` to 0.2) and with T14's style overrides (padding). Land after T67.4 and T67.5, which edit the same `read.rs`. T69's "resize within the layout rules" builds on this.

**Open questions for the creator.**
1. Which are in: main-axis distribution, child sizing (grow/hug/fixed), padding, max width, responsive reflow?
2. Child sizing as a universal attribute (a format change, like tilt) or as props on the kinds that sit in a stack?
3. Responsive behaviour: a grid that fits as many columns as a minimum width allows, breakpoints (as tokens or a `weft.json` key), or none?
4. Padding here, or in style overrides as `style-padding`?
5. Sizes as `dimension` tokens only, or literal numbers too?
6. Ship as a catalog minor within `weft` 0.1, or together with T39's 0.2?

**Split.** T16.1: design (docs only). T16.2: spec, catalog, core, `AGENT-SPEC.md`, corpus. T16.3: web generators and importers, plus the reference renderer. T16.4: SwiftUI generator and importer. T16.5: Slint generator and reader. T16.6: A2UI and json-render. T16.7: Figma and Penpot through `design-tool`.

### T17. Extension catalogs

A project can extend the core catalog with exactly one file today. Real hosts combine several sources: the core, one or more component libraries, and their own kinds. So a project needs to load several catalogs at once without their kinds colliding. Once catalogs travel between projects, two more questions need answers: how a catalog is published and found, and who keeps the registry. `research.md` §9 lists "Who keeps a registry of extension catalogs" as open; `docs/catalog-and-tokens.md` and `docs/what-is-weft.md` both say there is no registry; `docs/fragments-design.md` defers namespaces and sharing across projects to this task.

**Context.**
- SPEC §10.2: `catalog` is one file name. §10.4 merges that one extension into the core catalog: new kinds need a whole definition, core kinds are extended by merge or join, the extension may only widen the core (§8 rules via `crates/weft-catalog/src/diff.rs`), codes `W706` (invalid) and `W707` (narrows), and the merged catalog takes the extension's `name` and `version`.
- Code: `Loader::extend_catalog` and `extend_definition` in `crates/weft-catalog/src/project.rs`. `Project.catalog` is a single `Catalog` with no record of which catalog each kind came from. The CLI's `--catalog` loads one whole catalog that replaces the project's (`load_catalog` in `crates/weft-cli/src/main.rs`). `weft_capabilities` already returns `catalogs: [{ name, version }]`, the core first and then the extension (T10); `weft_catalog` lists kinds with no source.
- Kind names follow `[a-z][a-z0-9]*(-[a-z0-9]+)*` (SPEC §2), so there is no `:`. `x-<vendor>-` names are opaque extensions that no catalog may define (§8, §10.4).
- T15.1/T15.2 import a Custom Elements Manifest into an extension whose kinds are the custom-element tags (`acme-button`), with `--name`/`--version`/`--catalog` and the `import.cem.*` settings (`crates/weft-import/src/cem.rs`, `crates/weft-cli/src/cem.rs`, fixture `crates/weft-import/tests/fixtures/cem/acme-ui.json`). A library catalog and a project catalog cannot be used together today.
- Generators already name extension kinds by convention (SwiftUI: `promo-card` becomes `PromoCardView`, SPEC §9); the Figma and Penpot libraries build one component set per kind.
- `@weft/catalog` is `private` in `packages/catalog/package.json`; nothing is published anywhere yet.
- Documents and validators never touch the network (AGENTS.md). Project file names stay inside the project directory (§10.2, `W703`), which rules out reading a `node_modules` in a parent folder.

**Scope.**
- Design first: `docs/extension-catalogs-design.md`, in the shape of `docs/context-design.md`, goes to the creator before any code. It covers the namespace syntax with alternatives; merge order and conflict rules across several extensions; which kinds a catalog may extend (core only, or also another extension's); the catalog's own declaration (prefix, the base versions it targets); the publication format and discovery; the registry model; and the effect on generators, the Figma and Penpot libraries, the T12 schema, and fragments (T31 part B).
- After approval: SPEC §5, §10.2 (`catalog` also takes an ordered array), §10.4, and new `W7xx` codes for cross-catalog conflicts; the loader keeps which catalog each kind came from, with differential fixtures; CLI `--catalog` can be repeated; MCP `project.catalog` accepts an array, `weft_capabilities` lists every catalog, `weft_catalog` shows each kind's catalog; `import-cem` gets a namespace option with its `import.cem.*` key (SPEC §10.6, `settings.rs`, `schemas/weft.schema.json`); `AGENT-SPEC.md` §1 and §2.8 and the primer; `docs/projects.md`, `docs/catalog-and-tokens.md`, and a publishing guide; the `research.md` §9 row answered.

**Out of scope.** Hosting a registry service. Network fetching during validation or rendering. Changing the `x-` extension rules. Fragments themselves (T31 part B). New generator features beyond naming namespaced kinds.

**Done when.**
- `examples/project` loads the core plus two extensions (one imported from `acme-ui.json`, one written by hand), and kinds from both validate in strict mode.
- A kind defined by two catalogs gets a diagnostic that names both catalogs, never a panic.
- `weft_capabilities` lists all three catalogs; the TypeScript and Rust differential fixtures agree.
- A catalog shipped in a package directory resolves by the documented convention with no network access.
- `schemas/weft.schema.json` is regenerated; SPEC, AGENT-SPEC and the docs are updated.

**Dependencies.** Done: T31 (project file and settings), T15.1/T15.2 (CEM import), T10 (capabilities). T12: its schema must cover the merged catalogs. Affected: T14/T40 (one design-tool library per catalog), T44 (SwiftUI custom views), T69 milestone 4 (catalog kinds as components), the fragments design (T31 part B).

**Open questions for the creator.**
1. Namespace syntax: a hyphen prefix (`acme-button`, like custom elements; no grammar change, every generator already handles it), a colon (`acme:button`; changes the §2 grammar, both parsers, the format version and every generator's naming), or a collision rule only, with no namespaces?
2. Who owns the prefix: the catalog author (declared in the catalog and checked), or the project (an alias at load time, so it can fix a collision)?
3. May an extension extend another extension's kinds, or only core kinds? When two extensions widen the same core prop (`button.variant`), is that a union or a conflict?
4. Distribution: a field in an npm package, a file committed to the repository, or a URL with an integrity hash fetched only by an explicit command (for example `weft catalog add`)?
5. Registry: none (a convention plus a list in the docs), a curated index in this repository, or a hosted service? Who keeps it?
6. Should a catalog declare the base catalog versions it targets? Confirm that screens stay silent about their catalogs (SPEC §10).

**Split.** T17.0: design document only. T17.1: spec and loader for several catalogs and namespaces, with fixtures. T17.2: CLI, MCP, `import-cem` namespace, settings, AGENT-SPEC and docs. T17.3: package resolution and the publishing guide, plus the registry as the creator decides.

### T41. Hosted Penpot plugin

Penpot installs a plugin only from the URL of its `manifest.json`, and plugins are hosted outside Penpot. Today the Weft Penpot plugin is available only to someone who clones the repository and runs `moon run penpot-plugin:serve`. The repository is public on GitHub (`listepo/weft`), so CI can build the plugin and publish `dist/` on GitHub Pages, which serves the files with the right content types and `Access-Control-Allow-Origin: *`.

**Context.**
- `plugins/penpot`: `build.ts` runs the shared `@weft/design-plugin` build and writes `dist/manifest.json`, `plugin.js` (the SES sandbox script, no WebAssembly) and `ui.html` (the core inlined). `manifest.json` uses version 2, so paths resolve from the manifest's folder and a subpath works; it has `name`, `description`, `code` and `permissions`, but no `icon`. `moon.yml` has `build` (depends on `root:wasm`, `runInCI: false`) and `serve` (`serve.config.ts`, port 4400, `cors: true`). `dist/` is gitignored; `test/bundle.test.ts` builds into a temp folder and checks the manifest.
- `plugins/penpot/README.md` says hosting is T41 and that nobody has checked whether a real Penpot needs the CORS header; `packages/penpot/README.md` lists hosting among the unverified items, and T40's result in `done.md` leaves hosting open. The plugin's UI needs no network access.
- Plugin identity: Penpot derives `host` from the manifest URL and gives each install a random id, unless a plugin with the same name and host is already registered. Weft keeps shared plugin data under the `weft` namespace, so boards built with a localhost install stay readable from the hosted one.
- GitHub: `main` has no `.github/` folder, and nothing mentions Pages. Open PR #4 (T73) adds `.github/workflows/ci.yml` (the merge gate on pull requests and pushes to `main`, `runs-on: xcode-27`, actions pinned to commit SHAs, `permissions: contents: read`) and moves XcodeGen to `mise.osx.toml` so `mise install` works on Linux. Pages is not enabled (`GET /repos/listepo/weft/pages` returns 404).

**Scope.**
- A Pages deploy job next to T73's gate: only on pushes to `main`, after the gate passes; builds with `moon run penpot-plugin:build` and publishes `plugins/penpot/dist` under a fixed path through `actions/upload-pages-artifact` and `actions/deploy-pages`, pinned to SHAs; `pages: write` and `id-token: write` on that job only, the `github-pages` environment, and a concurrency group so two deploys never race.
- A check step after the deploy: fetch `manifest.json`, `plugin.js` and `ui.html` from the Pages URL and fail unless each returns 200 with a JSON, JavaScript or HTML content type and `Access-Control-Allow-Origin: *`.
- An offline Vitest test that reads the workflow and pins that the deploy runs only on `main` after the gate, builds the Penpot plugin, publishes exactly its `dist/`, and pins every action to a SHA.
- An `icon` in `manifest.json` if Penpot's plugin manager needs one; the build copies it into `dist/`, and `bundle.test.ts` checks it.
- Docs: `plugins/penpot/README.md` gives the install URL first and keeps local serving for development; the root `README.md` mentions the URL; `packages/penpot/README.md` moves hosting from unverified to verified, with the date.
- No new tool option, so no `weft.json` key.

**Out of scope.** Listing the plugin in Penpot's own plugin catalogue. Hosting the Figma plugin. Versioned or per-PR preview deployments. A custom domain. T40's other open checks in a real Penpot (async variants, tokens, grid cells, plugin data on copies, UI theme).

**Done when.**
- A push to `main` deploys, and the check step passes.
- Penpot (design.penpot.app) installs the plugin from the Pages URL in its plugin manager, and Build and Export both work on a corpus screen.
- `plugins/penpot/README.md` gives that URL.
- The full check exits 0.

**Dependencies.** T73: extend its `ci.yml` rather than adding a second gate; T73 also makes `mise install` work on Linux. The creator enables Pages with the source set to "GitHub Actions" (a repository setting; agents do not change it).

**Open questions for the creator.**
1. URL: `https://listepo.github.io/weft/penpot/manifest.json` (a subpath, leaving room for other pages) or the site root?
2. Deploy on every push to `main`, or only on a release tag, so users never get an unreleased plugin?
3. Runner for the deploy build: GitHub-hosted `ubuntu-latest` or the gate's `xcode-27` runner?
4. A repository has one Pages site and each deploy replaces it whole: should the deploy assemble a site now (an index page, room for docs or a `render-react` gallery)?
5. Which icon should the plugin manager show?
