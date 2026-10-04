# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 55% | Claude Code / claude-opus-5-5 |
| T23 | todo | P2 | 3 | 0% | |
| T24 | todo | P2 | 2 | 0% | |
| T28 | in progress | P2 | 3 | 75% | Claude Code / claude-opus-5-5 |
| T14 | in progress | P2 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T31 | in progress | P1 | 5 | 45% | Claude Code / claude-opus-5-5 |
| T32 | in progress | P2 | 2 | 85% | Claude Code / claude-sonnet-5-5 |
| T33 | in progress | P1 | 2 | 0% | Claude Code / claude-sonnet-5-5 |
| T34 | in progress | P1 | 5 | 0% | Claude Code / claude-opus-5-5 |

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

Progress: steps 1–8 are done (the TypeScript side runs on the Rust core through weft-wasm since T22); the fragments proposal is in `docs/fragments-design.md` and waits for the creator. Steps 9–14 are in progress.

### T32. Claude Code plugin from GitHub

The T30 plugin works only when its marketplace is added from a local clone: Claude Code copies just the plugin folder into its cache, and the `@weft/*` packages run from the repository's sources. Bundle the plugin's scripts and the MCP server into self-contained files at release, so the plugin installs from the GitHub-hosted marketplace once the repository has a remote, and add the `repository` field to `plugin.json`. Check that Claude Code Desktop finds `node` when started from the GUI. Done when `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` work on a clean machine.

Progress: the bundle carries the WebAssembly core (T22 merged): `plugins/claude-code/build.ts` bundles the scripts and the MCP server with Vite 8 into the committed `dist/` and copies `weft_bg.wasm` to `dist/wasm/`, where `@weft/core` reads it relative to the shared chunk; `claude-code:build` depends on `root:wasm`. The plugin folder alone runs (tests copy only it to a temp folder and run every script and the server there; `claude --plugin-dir <copy> mcp list` shows `weft` connected; `claude plugin validate --strict` passes). The Desktop `node` requirement (24.2 or later on the PATH) is in the plugin README. `root:wasm` remaps build paths, so the `.wasm` is byte-reproducible and the up-to-date test compares it byte for byte. Remaining: `repository` in `plugin.json` once a remote exists, and the done criterion itself, `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` on a clean machine.

### T33. Documentation for people

The repository explains Weft to models (`AGENT-SPEC.md`) and to implementers (`SPEC.md`), but not to the people who use it. Write a `docs/` guide in plain English: what Weft is and why, a ten-minute tour of a screen, how the pieces fit (Rust core, WebAssembly, the TypeScript packages, CLI, MCP server, Claude Code plugin), and one page per tool with commands that were run and their real output. `README.md` links to it. Done when every command in the guide runs as written on main and a reader new to the project can validate, render, import and export a screen by following it.

### T34. SwiftUI generator and importer

Generate SwiftUI from `.weft` and read SwiftUI source back into `.weft`, in a Rust crate `weft-swiftui` next to the core, exposed through the CLI. Approved scope: the generated code targets iOS 17 and macOS 14 or later (the data model through `@Observable` and `@Bindable`); the importer parses Swift source (no running app needed). Code that Weft generated comes back without loss; other SwiftUI code imports with a loss table, like the HTML importer. Done when every corpus screen generates Swift that compiles for iOS 17, survives Weft → SwiftUI → Weft byte-identical after formatting, and a hand-written SwiftUI sample imports with the expected losses.
