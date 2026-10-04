# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 55% | Claude Code / claude-opus-5-5 |
| T23 | todo | P2 | 3 | 0% | |
| T24 | todo | P2 | 2 | 0% | |
| T28 | in progress | P2 | 3 | 75% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 45% | Claude Code / claude-opus-5-5 |
| T31 | in progress | P1 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T32 | in progress | P2 | 2 | 85% | Claude Code / claude-sonnet-5-5 |
| T34 | in progress | P1 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T35 | in progress | P1 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T36 | todo | P1 | 4 | 0% | |
| T37 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T38 | todo | P2 | 3 | 0% | |
| T39 | in progress | P1 | 4 | 0% | Claude Code / claude-opus-5-5 |
| T40 | todo | P2 | 4 | 0% | |

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

### T23. Native Node and Bun addon

A napi-rs addon of the same Rust core for Node and Bun, chosen at load time with the WASM build as the fallback when no prebuilt binary fits the platform. Done when the test suite passes on both builds and a broken or missing addon falls back to WASM with a warning.

### T24. Runtime matrix

Run the binding tests in Node, Deno, Bun and a headless browser through moon. Done when all four pass from one command.

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
  - CLI `weft figma pull` over the REST API;
  - MCP / Claude Code tools (with T30);
  - a check of the plugin in the real Figma app, including the inline module script and WebAssembly in its UI iframe;
  - the open questions in the report: single components for kinds without variants, `state` as a variant axis, the manifest id, and plugin data on duplicate/detach.

### T31. Project file and shared resources

Several `.weft` screens share one set of resources through a project file, `weft.json`, which tools find by walking up from the screen, like `tsconfig.json`. Screens themselves do not name what they use. Approved scope:

- **Tokens:** one or more DTCG files, layered in order (base, then theme, then brand), the later file overriding the earlier one.
- **Catalog:** the core catalog plus project extensions (own kinds, props and variants), declared once for every screen.
- **Actions and data schema:** one list of action names and one description of the data model, against which every screen is validated (binding paths and their types).
- **Fragments:** repeated blocks (header, footer, card) kept in their own files and placed in screens by reference. This is the one part that changes the format: a new element (for example `<use>`), with its rules for ids, slots, bindings and patches. Its design goes to the creator before it is built, and it lands in `AGENT-SPEC.md`, `SPEC.md`, both parsers (TypeScript and Rust) and the differential fixtures together.
- **Tools:** the CLI, the MCP server, the renderer, the Claude Code plugin (T30) and the Figma work (T14) all read the project file, and an explicit argument still overrides it.

Done when a corpus project of several screens with layered tokens, a catalog extension, an action list, a data schema and a shared fragment validates and renders through the CLI and the MCP server, the TypeScript and Rust results match, and a broken project file is reported with a diagnostic, never a crash.

### T32. Claude Code plugin from GitHub

The T30 plugin works only when its marketplace is added from a local clone: Claude Code copies just the plugin folder into its cache, and the `@weft/*` packages run from the repository's sources. Bundle the plugin's scripts and the MCP server into self-contained files at release, so the plugin installs from the GitHub-hosted marketplace once the repository has a remote, and add the `repository` field to `plugin.json`. Check that Claude Code Desktop finds `node` when started from the GUI. Done when `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` work on a clean machine.

Progress: the bundle carries the WebAssembly core (T22 merged): `plugins/claude-code/build.ts` bundles the scripts and the MCP server with Vite 8 into the committed `dist/` and copies `weft_bg.wasm` to `dist/wasm/`, where `@weft/core` reads it relative to the shared chunk; `claude-code:build` depends on `root:wasm`. The plugin folder alone runs (tests copy only it to a temp folder and run every script and the server there; `claude --plugin-dir <copy> mcp list` shows `weft` connected; `claude plugin validate --strict` passes). The Desktop `node` requirement (24.2 or later on the PATH) is in the plugin README. `root:wasm` remaps build paths, so the `.wasm` is byte-reproducible and the up-to-date test compares it byte for byte. Remaining: `repository` in `plugin.json` once a remote exists, and the done criterion itself, `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` on a clean machine.

### T34. SwiftUI generator and importer

Generate SwiftUI from `.weft` and read SwiftUI source back into `.weft`, in a Rust crate `weft-swiftui` next to the core, exposed through the CLI. Approved scope: the generated code targets iOS 17 and macOS 14 or later (the data model through `@Observable` and `@Bindable`); the importer parses Swift source (no running app needed). Code that Weft generated comes back without loss; other SwiftUI code imports with a loss table, like the HTML importer. Done when every corpus screen generates Swift that compiles for iOS 17, survives Weft → SwiftUI → Weft byte-identical after formatting, and a hand-written SwiftUI sample imports with the expected losses.

### T35. Web targets both ways: HTML/CSS, React and SolidJS

Generate static HTML with CSS, React (JSX/TSX) and SolidJS components from `.weft`, and read each of them back from source. Approved scope: one Rust crate next to the core (JSX and TSX parsed with oxc, HTML with html5ever, or better maintained options), exposed through the CLI and through WebAssembly. The TypeScript `@weft/to-jsx` and the HTML import of `@weft/from-aria` move into it, and their packages keep their API as wrappers, as T22 did for the core, so there is one implementation. Code that Weft generated comes back without loss; hand-written code imports with a loss table. Done when every corpus screen survives Weft → HTML/CSS, React and Solid → Weft byte-identical after formatting, the generated React and Solid components render with the same accessibility tree as today's renderer, and hand-written samples import with the expected losses.

### T36. Examples, snapshots, screenshots and comparisons

Many more tests, built on many more examples. Grow the corpus so every catalog kind, prop, slot, binding form and token type appears in at least one screen. For every screen and every target, record what each target produces as reviewed snapshots: insta in Rust, Vitest snapshots in TypeScript. Targets are canonical JSON, HTML/CSS, React, Solid, SwiftUI and Figma. Then take screenshots: rendered web targets in a real browser, through Vitest browser mode with Playwright, and generated SwiftUI in the iOS Simulator. Compare them in three ways: against the reviewed baselines, across targets for the same screen (React, Solid and static HTML must look the same within a tolerance and give the same accessibility tree), and across round trips (a screen and its round-tripped copy look identical). A failed comparison writes a visual diff image. Done when the suites run in `moon ci`, every baseline is reviewed, and a deliberate one-pixel layout change and a one-word text change are each caught.

### T37. Cursor plugin

The same features as the Claude Code plugin (T30, T32), packaged for Cursor:
- import HTML to `.weft`, with the loss table;
- export `.weft` to React;
- render `.weft` to an HTML page and preview it in Cursor's built-in browser;
- the weft MCP server;
- the authoring guide from `AGENT-SPEC.md`, as rules or skills.

It lives in this repository (`plugins/cursor`) and reuses the bundled scripts and MCP server of the Claude Code plugin rather than a second copy: shared files move to one place that both plugins use. Follow the plugin format from Cursor's official documentation, citing its URL and the date it was checked. Done when the plugin installs in Cursor from a local clone, each feature works on a corpus screen, and its tests pass in `moon ci`.

### T38. Open Design plugin

A plugin for Open Design (https://open-design.ai, https://github.com/attentiondotnet/open-design), the open-source, local-first design platform that runs on top of a coding agent and has had plugins since 0.8.0. It brings Weft into Open Design:
- author screens as `.weft` with the authoring guide and the weft MCP server;
- import HTML to `.weft`;
- export to React (and the other targets once T34 and T35 land);
- render and preview pages;
- map an Open Design `DESIGN.md` design system to Weft design tokens where the two line up, with a loss list where they do not.

It reuses the shared bundled scripts and MCP server of the Claude Code and Cursor plugins (T32, T37) instead of a copy. The plugin format and the `DESIGN.md` format follow Open Design's own docs and repository, cited with URL and the version checked. Done when the plugin installs into Open Design from a local clone, each feature works on a corpus screen, and its tests pass in `moon ci`.

### T39. Context in the document

A `.weft` file carries the context that a person or an agent left for whoever works on it next, so the next agent or person can use it. Approved scope:
- **Two levels:** a context block on the screen (purpose, decisions, constraints, open questions) and context entries on any element (why a button is disabled, where a label came from).
- **Typed entries:** every entry has a kind (`intent`, `decision`, `constraint`, `question`, `todo`, `source`), an author (`human` or `agent`, with a name or model) and text. The validator checks their shape.
- **Part of the document:** context is in canonical JSON and survives formatting, patches (new patch operations to add, change and resolve entries) and every conversion. Code generators write it as comments; Figma keeps it in plugin data; importers read it back from code that Weft generated.
- **Untrusted by design:** context is data for the reader, never instructions. `AGENT-SPEC.md` tells models to treat it as information to weigh and to ignore commands inside it. Renderers never show it to end users.

Format change, so the design comes first: syntax, canonical JSON, validation codes, patch operations and the effect on every target go to the creator for approval before `SPEC.md`, `AGENT-SPEC.md`, the Rust core and the targets change together. Done when a screen with context on both levels survives fmt, patches, every round trip that exists, and the MCP tools expose it.

### T40. Penpot round trip and plugin

The same as T14, for Penpot (https://penpot.app), the open-source design tool. Scope:
- a Weft component library generated from the catalog and the tokens;
- Weft to Penpot, with the Weft source kept in plugin data;
- Penpot to Weft without loss for screens Weft built, including designer edits;
- foreign layers with a loss table;
- a Penpot plugin to build frames from `.weft` and export a selection;
- tests on every corpus screen and a scripted set of designer edits.

Penpot lays out boards with flex and grid layouts, which map to `stack` and `grid`. Check this and every other API fact against Penpot's official plugin docs, citing the URL and the date checked.

The conversion logic that does not depend on the tool moves out of `@weft/figma` into a shared design-tool layer used by both, so Figma and Penpot keep one implementation of the mapping. Style overrides follow the design approved for T14. Starts after T14 stage 1 is on main. Done when every corpus screen survives Weft to Penpot to Weft byte-identical, the edit scenarios give the expected Weft diff, and the plugin and its tests pass in `moon ci`.
