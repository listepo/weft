# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 55% | Claude Code / claude-opus-5-5 |
| T22 | in progress | P1 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T23 | todo | P2 | 3 | 0% | |
| T24 | todo | P2 | 2 | 0% | |
| T28 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T29 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T14 | in progress | P2 | 5 | 0% | Claude Code / claude-opus-5-5 |
| T30 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T31 | in progress | P1 | 4 | 5% | Claude Code / claude-opus-5-5 |

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

### T22. WebAssembly bindings

Build the Rust core and catalog with wasm-bindgen (wasm-pack) into the `@weft/core` and `@weft/catalog` packages, keeping their TypeScript API, so the browser, Deno, Bun and Node run the same code. The renderer, importer, JSX generator, MCP server and benchmark stay TypeScript and call these packages. This introduces a build step: `AGENTS.md` drops the "No build step" rule for these two packages. Done when every existing TypeScript test passes on the WASM build.

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

### T29. Rust core test suite

The Rust core (T20) and catalog (T21) are checked mostly by the differential fixtures, which prove agreement with TypeScript but not the claims themselves, and they cover only inputs the generators reach. This task gives `weft-core` and `weft-catalog` their own tests: unit tests named as claims for every public function, and property tests that no input panics, that parse → serialize → parse is stable, and that formatting is idempotent. Done when every public function has tests for its documented behaviour, the property tests run in `cargo nextest`, and `moon ci` is green.

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

### T30. Claude Code plugin

A Claude Code plugin, used from Claude Code Desktop, that works on `.weft` files. It lives in this repository (`plugins/claude-code`) with a marketplace manifest at the root (`.claude-plugin/marketplace.json`), so it installs with `/plugin marketplace add`. Approved scope:

- Import: an HTML file to `.weft` through `@weft/from-aria` (`fromDom`), printing the loss table.
- Export: a `.weft` file to a React component through `@weft/to-jsx`.
- Render: a `.weft` file, with optional data and tokens, to an HTML page through `renderPage` of `@weft/render-react`, then opened in the Desktop app's built-in browser for preview.
- The plugin also registers the existing MCP server (`@weft/mcp`) and a skill that teaches `AGENT-SPEC.md`, so authoring, validation and patches work in the same session. The MCP server stays file-free; file reading and writing belong to the plugin's commands.

Done when the plugin installs from the marketplace in Claude Code Desktop, the three commands work on corpus screens, and their tests pass in `moon ci`.

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
