# Done

### T1. Semantic model and specification v0

The contract everything else builds on: node kinds, roles, states, slots, bindings, actions, token references, catalog shape, diagnostics, patches and versioning rules. Delivered as `SPEC.md` and the Zod model in `packages/core/src/model.ts`.

### T3. Core catalog and tokens

The `weft-core` 0.1 catalog of SPEC §5.1 as data, and Design Tokens resolution. Delivered as `packages/catalog`: `coreCatalog` with all 29 components, the generated `catalog.json`, a DTCG 2025.10 loader with alias resolution and problem reports, a default token set, and one example per component.

### T2. Parser, serializer, validator

Markup ↔ canonical JSON and the three validation layers of SPEC §6 with repair-oriented diagnostics. Delivered in `packages/core`: a strict tokenizer, `parse`, `serialize`, `canonicalize`, `validate` with lenient and strict modes, the diagnostic code registry (SPEC §6.2), JSON Schema export and the `weft validate` / `weft fmt` CLI. The round-trip property holds on 10 000 generated documents.

### T7. Versioning and extensibility tests

Compatibility suite for SPEC §8. Delivered as `compat/` fixtures (documents from a newer minor version, unknown elements and attributes, vendor extensions, an unsupported major version), core and renderer tests proving that a 0.1 reader warns instead of failing, keeps unknown content on round-trip and renders it with its fallback role, and `diffCatalogs` in `packages/catalog`, which classifies a catalog change as major, minor or none.

### T0. Corpus and benchmark harness

Reference screens in every compared format and a harness that measures them. Delivered as `corpus/` (12 screens in Weft, HTML, JSX and A2UI v0.9 with shared data, 36 edit tasks and 24 questions), and `bench/` (offline token report, provider interface, per-format checkers, strict conformance test of every corpus screen and catalog example). Measured with a proxy tokenizer: Weft needs 67% fewer tokens than A2UI JSON, 15% fewer than JSX and the same as HTML. The model-run half of the stop criterion (edit success) needs an API key and is carried by T8.

### T9. Specification revision from corpus findings

Closed the gaps the corpus exposed in SPEC 0.1: a bindable `text` prop on every text-bearing component (replacing `value` on `text` and `heading`), `button.submit`, an `empty` slot on `list` and `table`, precise `label` semantics, component-placed slots, `<each>` content rules, and `min`/`max`/`integer` on numeric props. Spec, model, catalog, validator (codes W224, W313, W314), corpus and benchmark agree.

### T4. React renderer

Document → React with correct ARIA, proven by comparing the accessibility snapshot of the rendered page with the roles and names the document declares. Delivered as `packages/render-react`: `render`, `WeftView`, `expectedTree`, snapshot parsing, formatting and diffing, a static page writer and a corpus gallery. The Chromium check passes with zero differences on 32 fixtures, all 12 corpus screens and 2 data variants.

### T6. Agent interface

Patch operations of SPEC §7 and an MCP server. Delivered as `applyPatches` in `packages/core` (atomic, validated, codes W501–W509) and `packages/mcp` with the tools `weft_primer`, `weft_catalog`, `weft_validate`, `weft_format`, `weft_patch` and `weft_render`. Hand-written patches complete 17 corpus edit tasks through `weft_patch` in tests; a run with a live model is part of T8.

### T5. Reverse mapping

Accessibility snapshot or DOM → Weft (lossy) and Weft → JSX source. Delivered as `packages/from-aria` (`fromAriaSnapshot`, `fromDom`, each returning the document, a list of losses and diagnostics W601–W602) and `packages/to-jsx` (`toJsx`, a self-contained React component with no Weft runtime dependency, injection-safe). Document → render → import → document preserves structure, roles, states and (from DOM) ids on all 12 corpus screens; generated JSX renders HTML equal to the reference renderer's on 12 corpus screens and 29 catalog examples. The losses are listed in SPEC §9.

### T19. mise toolchain and moon monorepo

Every program the project uses is installed by `mise install` (Node, pnpm, Rust with the `wasm32-unknown-unknown` target, moon, Deno, Bun, wasm-pack, wasm-bindgen, the napi-rs CLI). moon runs the workspace tasks: each package has its own `test` task, the root has `typecheck`, `lint` and `fmt`, and `moon ci` runs everything with caching. `pnpm run ci` keeps working and goes through moon. Done when a clean checkout passes `mise install && pnpm install && moon ci`, and `toolchain.md` lists every program.

Result: `mise install` provides every program; moon runs `test` in each of the seven packages plus root `typecheck` and `lint`, 767 tests, a repeated run is fully cached; a clean clone passes `mise install && pnpm install && moon run :test root:typecheck root:lint`. pnpm moved from the `npm:` backend to the registry (aqua) backend at the same version, because `npm:pnpm` 12 installs a placeholder instead of the native binary.

Execution plan:

1. `mise.toml`: add the tools at their latest versions; existing pins stay.
2. `.moon/workspace.yml`, `.moon/toolchains.yml`, shared JS task file, `moon.yml` per package and at the root (pattern from runa, which already runs moon 2.5).
3. Root `package.json` scripts call moon; `.gitignore` gets `.moon/cache`.
4. `AGENTS.md` commands, `README.md`, `toolchain.md`.
5. Verify: `moon ci` and `pnpm run ci` pass; a second run is cached.

### T25. Benchmark rigor

Fix the benchmark limitations found in T8: three samples per task with the spread reported; answer checks that accept a format's own spelling of the same action or value (`press:nav.reset`, `actions.nav.reset()`) and re-score saved replies without new calls; parallel requests; a Message Batches API mode at half the price. Done when the harness tests cover each of these and a re-score of the saved T8 replies runs offline.

The creator asked to run the benchmark on a local model too: Bonsai 27B (`prism-ml/bonsai-27b`) served by LM Studio's OpenAI-compatible API.

Execution plan:

1. `bench/src/provider.ts`: an OpenAI-compatible provider (LM Studio at `http://localhost:1234/v1` by default, `--base-url` to change it); only the final `content` counts, reasoning text is ignored.
2. `bench/src/run.ts`: `--provider anthropic|openai`, `--samples N` (default 3), `--concurrency N` (p-limit), `--batch` (Anthropic Message Batches: one batch for first replies, one for repairs), and `rescore <results.json>` that re-checks saved replies offline.
3. `bench/src/run-tasks.ts`: results carry the sample index; summaries give the mean over samples and the min–max spread; answer checks accept a format's own spelling of an action (`press:nav.reset`, `actions.nav.reset()`).
4. Tests with mocked fetch for both providers and the batch flow, the answer spellings, sampling and the rescore command.
5. Verify with `moon ci`, then a short live run against Bonsai.

Outcome: the T8 runs saved no replies, so the offline re-score was run on the saved Bonsai runs of this task instead (`run.ts rescore bench/results/read-2026-10-04T19-35-27-209Z.json`). Smoke runs on Bonsai are in the `test.md` history; the full runs belong to T8.

### T27. Agent specification

The creator asked for a specification written for AI agents that read and write Weft, and for an `AGENTS.md` rule that keeps it current. `SPEC.md` is the format contract; agents need the working side of it: what the host gives them, how to write a valid screen, how to edit with patches, and how to repair each diagnostic code. Done when `AGENT-SPEC.md` exists, a test keeps it from drifting from the catalog, the diagnostic codes and the validator, and `AGENTS.md` requires keeping it current.

Execution plan:

1. `AGENT-SPEC.md` at the root: host inputs, writing rules (syntax, ids, values, text, events, structure, the core catalog, extensions), patches, the repair loop with one fix per diagnostic code, reading a screen, a checklist.
2. `bench/test/agent-spec.test.ts`: every catalog kind and every diagnostic code is in the guide; every `xml` example is valid in strict mode and canonical; every `json` patch list applies to the first example.
3. `.moon/tasks/all.yml` lists `/AGENT-SPEC.md` as a test input; `README.md` links the guide; `AGENTS.md` gets the rule.
4. Verify with `moon ci`.

### T20. Rust core

Port `packages/core` (semantic model, parser, serializer, validator, patch operations, diagnostics, CLI) to a Cargo crate. `SPEC.md` stays the contract and diagnostic codes keep their meaning. Done when the Rust core passes the existing core test suite through the bindings of T22 plus its own `cargo test`, and the CLI behaves like the TypeScript one.

Result: `crates/weft-core` and `crates/weft-cli` (binary `weft`, `validate` and `fmt`). A differential fixture written by the TypeScript core holds 1288 cases (622 markup, 357 JSON, 309 patch lists: SPEC examples, corpus, compat, catalog examples, every diagnostic case, patch failures and seeded fuzz); the Rust core reproduces all of them byte for byte. 51 Rust tests: unit tests, the differential check, seven runs of the real binary and a `cargo metadata` check that only the binary depends on clap and anyhow. `moon ci` runs `cargo nextest`, `clippy -D warnings` and `fmt --check`. Known gap: a JSON number that overflows `f64` (`1e400`) is Infinity for `JSON.parse` but a parse error for serde_json.

Execution plan:

1. Cargo workspace at the root (`Cargo.toml`, `members = ["crates/*"]`): `crates/weft-core` (library, no I/O) and `crates/weft-cli` (the `weft` binary: clap, anyhow, file I/O). Dependencies: serde, serde_json (`preserve_order`), indexmap, ryu-js (numbers printed as JavaScript prints them, so canonical JSON and messages match byte for byte), thiserror. Grammars are small hand-written matchers instead of the regex crate, which would add weight to the WASM build of T22.
2. Modules mirror `packages/core/src`: `model`, `rules`, `diagnostics`, `values`, `syntax`, `canonical`, `parse`, `serialize`, `shape` (the JSON shape checks that zod does in TypeScript, same paths and messages), `validate`, `patch`, `json`. Positions count UTF-16 code units and keys sort by UTF-16 code unit, as `SPEC.md` says; object keys keep JavaScript's order.
3. Tests: the TypeScript core suite's cases ported to `cargo test` (every diagnostic code, round-trips, patches, CLI); a differential check runs the TypeScript and Rust cores over the corpus, the examples and the compat fixtures and compares documents and diagnostics.
4. moon tasks for `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` in `moon ci`; `toolchain.md` and the shared `rust.md` list the crates.
5. The TypeScript suite running against the Rust core waits for the bindings of T22; this task ends with the Rust crate and CLI matching the TypeScript ones.

### T21. Rust catalog

Port `packages/catalog` (core catalog and design tokens) to a Cargo crate used by the Rust core. Done when the catalog tests pass against it.

Result: `crates/weft-catalog` with `core_catalog()` (the embedded `catalog.json`), `load_tokens`, `token_types` and `diff_catalogs`. A differential fixture written by the TypeScript package holds 305 token inputs (the default tokens, non-objects and seeded fast-check trees covering T001–T006) and 110 catalog pairs diffed both ways (the `diff.test.ts` table and seeded edits of the core catalog); the Rust crate reproduces all of them. Its tests also validate every example in strict mode with the default tokens, and resolve a 200 000-token alias chain without recursion. The TypeScript catalog tests themselves run against the Rust crate once the bindings of T22 exist.

Execution plan:

1. `crates/weft-catalog` (library, no I/O; depends on weft-core, serde_json, indexmap):
   - `CORE_CATALOG_JSON` embeds `packages/catalog/catalog.json`, which `catalog-json.test.ts` already keeps equal to `core.ts`; `core_catalog()` deserializes it into `weft_core::Catalog`.
   - `load_tokens(&Json)` ports `tokens.ts` (DTCG subset, T001–T006, alias chains and cycles, JavaScript key order); `token_types` gives the path → type map the core's `tokens` option takes.
   - `diff_catalogs(&Json, &Json)` ports `diff.ts` (SPEC §8 levels). It takes JSON, not the typed `Catalog`, because a field the model does not know is itself a breaking change and the typed model rejects it.
   - weft-core exports `order_keys` and `to_compact`, which the port needs for key order and `JSON.stringify` messages.
2. Tests: unit tests per module, plus a differential fixture `crates/weft-catalog/tests/fixtures/differential.json` written by `packages/catalog/test/differential.test.ts`: the default tokens, bad inputs and seeded fast-check token trees for `loadTokens`; the `diff.test.ts` rows (moved into a shared table) in both directions plus the core catalog for `diffCatalogs`. The Rust test reproduces every case, checks `core_catalog()` against the embedded JSON, and validates every example in strict mode.
3. moon: the catalog `test` task gets the fixture as an input; `rust-test` already covers the crate. `toolchain.md` needs no new rows.

### T26. Vitest for the TypeScript tests

Run every TypeScript test suite with Vitest (on Vite) instead of `node:test`: the 32 test files in `packages/*/test` and `bench/test` move to Vitest's API, `fast-check` stays for property tests, and each package's moon `test` task calls `vitest run`. Vitest's browser mode then serves the browser leg of T24, and the same suites check the WASM build of T22. `AGENTS.md` changes its test rule from `node:test` to Vitest. Done when every existing test passes under Vitest with the same count, `moon ci` is green, and `node:test` is no longer used.

Result: `vitest` 5.0.3 and `vite` 8.3.2 (its required peer) are root dev dependencies; one root `vitest.config.ts` serves every package, and the inherited moon `test` task runs `vitest run --config $workspaceRoot/vitest.config.ts` with the same inputs plus the config. The 32 test files import `describe`, `test` and the hooks from `vitest` and still assert with `node:assert/strict`, which kept the diff to the imports and the few subtest blocks. Vitest has no subtests, so the `t.test` checks of `from-aria/test/roundtrip.test.ts` and `to-jsx/test/equivalence.test.ts` became `describe` blocks of `test`s; `t.skip` in the browser test is Vitest's context skip. Test counts under `node:test` before and under Vitest after: catalog 122 and 122, core 152 and 152, mcp 32 and 32, bench 76 and 76, render-react 154 and 154, to-jsx 139 and 139, from-aria 107 and 83, 782 and 758 in total. The 24 fewer in from-aria are the 24 parent tests (two per corpus screen) that held only the shared import and no assertion; every test name that carried an assertion is unchanged, and to-jsx keeps its parent's parse check as its own test. `WEFT_UPDATE_FIXTURES=1` still rewrites both differential fixtures byte for byte. The Rust and package sources are untouched. `AGENTS.md` and `toolchain.md` name Vitest, and `node:test` is gone from the repository apart from history docs. Vitest's browser mode is not set up; T24 does that.

### T30. Claude Code plugin

A Claude Code plugin, used from Claude Code Desktop, that works on `.weft` files. It lives in this repository (`plugins/claude-code`) with a marketplace manifest at the root (`.claude-plugin/marketplace.json`), so it installs with `/plugin marketplace add`. Approved scope:

- Import: an HTML file to `.weft` through `@weft/from-aria` (`fromDom`), printing the loss table.
- Export: a `.weft` file to a React component through `@weft/to-jsx`.
- Render: a `.weft` file, with optional data and tokens, to an HTML page through `renderPage` of `@weft/render-react`, then opened in the Desktop app's built-in browser for preview.
- The plugin also registers the existing MCP server (`@weft/mcp`) and a skill that teaches `AGENT-SPEC.md`, so authoring, validation and patches work in the same session. The MCP server stays file-free; file reading and writing belong to the plugin's commands.

Done when the plugin installs from the marketplace in Claude Code Desktop, the three commands work on corpus screens, and their tests pass in `moon ci`.

Result: the plugin lives in `plugins/claude-code` (workspace package `@weft/claude-code`) with a marketplace at `.claude-plugin/marketplace.json`. Skills `/weft:import` (`fromDom`, serialize, loss table), `/weft:export` (`toJsx`), `/weft:render` (`renderPage` with `--data` and `--tokens`, default tokens otherwise; prints a `file://` URL that Claude opens in the Desktop browser pane) run scripts in `scripts/`; `weft:spec` links `AGENT-SPEC.md`; `.mcp.json` registers the file-free `weft` MCP server. Scripts never overwrite without `--force` and exit 0, 1 (invalid input) or 2 (usage or I/O). `claude plugin validate --strict` passes for the plugin and the marketplace, and a headless `claude --plugin-dir` session lists the four skills and a connected `weft` server. 95 Vitest tests cover every corpus screen through the three scripts, the failure paths, the manifests and the MCP server start. The plugin runs from a clone added as a local marketplace (a GitHub-hosted marketplace would copy only the plugin folder, without the packages), see `plugins/claude-code/README.md`.

### T22. WebAssembly bindings

Build the Rust core and catalog with wasm-bindgen (wasm-pack) into the `@weft/core` and `@weft/catalog` packages, keeping their TypeScript API, so the browser, Deno, Bun and Node run the same code. The renderer, importer, JSX generator, MCP server and benchmark stay TypeScript and call these packages. This introduces a build step: `AGENTS.md` drops the "No build step" rule for these two packages. Done when every existing TypeScript test passes on the WASM build.

Result: `crates/weft-wasm` binds the Rust core and catalog with wasm-bindgen; `moon run root:wasm` (wasm-pack, `--target web`) generates `packages/core/wasm/` (gitignored, about 0.94 MB without wasm-opt), and every `test` task and `root:typecheck` depend on it. `@weft/core` and `@weft/catalog` keep every exported name and signature; `parse`, `validate`, `applyPatches`, `serialize`, `canonicalize`, `stringify`, `didYouMean`, `loadTokens` and `diffCatalogs` run in Rust, and `syntax.ts`, `values.ts` and the checks in `rules.ts` are deleted. Catalog functions are methods of a `Catalog` handle class (wasm-bindgen cannot take `Option<&Catalog>`); the TypeScript side caches one handle per catalog object in a `WeakMap` and rebuilds it when the catalog's JSON text changes. Source positions cross as a pre-order list, and `ParseResult.source` is built on first read. `weft-core` gained two re-exports, `to_document` and `JSON_DEPTH_LIMIT`. All 782 TypeScript tests pass on the WASM build (core 152, catalog 122, from-aria 107, mcp 32, render-react 154, to-jsx 139, bench 76, as before), including the differential fixtures byte for byte; Rust tests went from 68 to 90 (21 in `weft-wasm`, one in `deps.rs`). A smoke run of `parse` and `serialize` passed on Node, Deno and Bun; browsers are untested (T24). Known differences: per call the WASM path is slower than the TypeScript core was (parse with a catalog about 29 µs against 9 µs, mostly re-serializing the catalog to check it is unchanged; the core suite takes 4.7 s against 2.6 s); an object that is not a catalog throws; `serialize` and `canonicalize` of a cyclic value throw instead of overflowing the stack; lone surrogates come back as U+FFFF. After merging T26, T28 and T30 from main, all 853 Vitest tests (from-aria 83 after T26, the new claude-code 95) and 105 Rust tests pass; the plugin's `test` task inherits the `root:wasm` dependency.

Execution plan:

1. **One binding crate, `crates/weft-wasm`** (`cdylib` + `rlib`, `wasm-bindgen = "=0.2.129"` to match the pinned CLI). It holds both packages' functions: the catalog crate depends on the core, so one module ships the core once and keeps one memory. Plain Rust functions (JSON text in, JSON text out) carry the logic and the unit tests; the `#[wasm_bindgen]` exports are one-line wrappers. A `Catalog` handle class parses a catalog once per catalog object instead of on every call. `weft-core` gains one re-export, `to_document` (JSON → model, already used by `validate`), which `serialize`, `canonicalize` and `stringify` need.
2. **What moves.** To WASM: `parse`, `validate`, `serialize`, `canonicalize`, `stringify`, `applyPatches`, `didYouMean` (core) and `loadTokens`, `diffCatalogs` (catalog). Stays TypeScript: types, zod schemas and their JSON Schema export, `DIAGNOSTIC_CODES`, `diagnostic` and `hasErrors` (the importer builds W601/W602 diagnostics with them), the rule regexes and `ARIA_ROLES`, `coreCatalog` (the source of `catalog.json`), `tokenTypes` (a projection of a map), the CLI. The TypeScript implementations that moved are deleted, so there is one implementation. `index.ts` keeps every name and signature. `ParseResult.source` is rebuilt (lazily) from positions the binding returns, and `ValidateOptions.source` is passed back, so line and column survive.
3. **Boundary: JSON text both ways.** `JSON.stringify` writes keys in JavaScript order and drops `undefined` members exactly as zod's optional fields read them, and `parse_json` already reproduces `JSON.parse`. On the way out `JSON.parse` defines a `__proto__` key as an own data property, as `Object.fromEntries` did in the TypeScript core (serde-wasm-bindgen assigns properties, which would hit the setter). Values JSON cannot carry (`NaN`, `±Infinity`, `BigInt`, functions, symbols) become a marker object the schema rejects wherever it appears, so they stay rejected instead of turning into `null`; cyclic input becomes text deeper than the depth limit, so `validate` reports W200 as before. Lone surrogates, which UTF-8 cannot carry, become U+FFFF: also outside XML and also one UTF-16 unit, so W113/W221 fire at the same place.
4. **Loading.** `wasm-pack build --target web`. `packages/core/src/wasm.ts` instantiates it with `initSync` from bytes read through `process.getBuiltinModule("node:fs")` (Node ≥ 22.3, Deno, Bun) and falls back to `await init()` (fetch, streaming compile) where there is no file system, as in browsers. Every exported function stays synchronous after import. `@weft/catalog` reaches the module through an internal `@weft/core/wasm` subpath.
5. **Build.** A `root:wasm` moon task (inputs: crates, `Cargo.*`, `catalog.json`; outputs: `packages/core/wasm/`) that every `test` task and `root:typecheck` depend on. The output is generated and gitignored. wasm-opt is off: wasm-pack would download binaryen, which mise does not pin.
6. **Tests.** The TypeScript differential fixtures now come from the WASM build, so they keep proving the binding reproduces byte for byte what the TypeScript core produced. `crates/weft-cli/tests/deps.rs` gains an assertion for `weft-wasm`. Verify with `moon run :test root:typecheck root:lint root:rust-test root:rust-lint`, test counts compared with the baseline (core 152, catalog 122, from-aria 107, mcp 32, render-react 154, to-jsx 139, bench 76).
7. **Docs.** `AGENTS.md` (build step for these two packages, layout), `README.md` (build command), `toolchain.md` (`wasm-bindgen` crate).

### T29. Rust core test suite

The Rust core (T20) and catalog (T21) are checked mostly by the differential fixtures, which prove agreement with TypeScript but not the claims themselves, and they cover only inputs the generators reach. This task gives `weft-core` and `weft-catalog` their own tests: unit tests named as claims for every public function, and property tests that no input panics, that parse → serialize → parse is stable, and that formatting is idempotent. Done when every public function has tests for its documented behaviour, the property tests run in `cargo nextest`, and `moon ci` is green.

Result: tests went from 68 to 438 in `cargo nextest run --workspace` (`weft-core` 42 → 326, of which 21 are property tests; `weft-catalog` 16 → 102, of which 9 are property tests; `weft-cli` unchanged at 10). The claims are integration tests over the public API: `json`, `markup`, `validate`, `canonical`, `model`, `diagnostics`, `source` and `patch` in `weft-core`, `tokens`, `diff` and `core` in `weft-catalog`, and `codes.rs`, which proves that every code of the SPEC table is registered, summarised and produced by at least one input. Property tests (`proptest` 1.11.0, fixed seed, 96 to 128 cases, no persistence file) run in about a second: no panic on arbitrary strings, JSON, deep nesting or patches; parse → serialize → parse keeps the document; serialize and `stringify(canonicalize(d))` are stable; markup and its JSON round trip validate alike; remove-then-insert restores a document; a catalog never differs from itself and removals mirror additions. No bug was found where Rust differs from TypeScript, so there is no `fix:` commit. Findings that both cores share were reported to the creator and not changed: validating a document at the depth limit overflows a 2 MiB test-thread stack in a debug build, JSON documents are accepted up to about 384 element levels while markup stops at 256, `integer` is missing from the catalog diff's known prop fields, and a `$type` that is not a string on a token itself is ignored without a problem.

### T33. Documentation for people

The repository explains Weft to models (`AGENT-SPEC.md`) and to implementers (`SPEC.md`), but not to the people who use it. Write a `docs/` guide in plain English: what Weft is and why, a ten-minute tour of a screen, how the pieces fit (Rust core, WebAssembly, the TypeScript packages, CLI, MCP server, Claude Code plugin), and one page per tool with commands that were run and their real output. `README.md` links to it. Done when every command in the guide runs as written on main and a reader new to the project can validate, render, import and export a screen by following it.

Result: `docs/` holds thirteen pages: an index, what Weft is, a ten-minute tour of the `login` screen, how the pieces fit (with a diagram and the differential tests), one page each for the `weft` command, the MCP server, the Claude Code plugin, rendering, importing, exporting to React, the catalog and tokens, and patches, and a contributing page that links `AGENTS.md`. Every console block was run in order from a clean checkout and its output is the real output, with the repository path shown as `/path/to/weft`. `docs/examples/mcp-call.mjs` is a small MCP client that lets the guide call the MCP tools from a shell. `README.md` links to the guide. The slash commands of the plugin could not be run in this session (the nested Claude session was not signed in), so the guide shows the scripts they call. Work still in progress (T14, T31, T32) appears only as one line under "Coming".

### T37. Cursor plugin

The same features as the Claude Code plugin (T30, T32), packaged for Cursor:
- import HTML to `.weft`, with the loss table;
- export `.weft` to React;
- render `.weft` to an HTML page and preview it in Cursor's built-in browser;
- the weft MCP server;
- the authoring guide from `AGENT-SPEC.md`, as rules or skills.

It lives in this repository (`plugins/cursor`) and reuses the bundled scripts and MCP server of the Claude Code plugin rather than a second copy: shared files move to one place that both plugins use. Follow the plugin format from Cursor's official documentation, citing its URL and the date it was checked. Done when the plugin installs in Cursor from a local clone, each feature works on a corpus screen, and its tests pass in `moon ci`.

Result: `plugins/cursor` is a Cursor plugin (`.cursor-plugin/plugin.json`, skills `weft-import`, `weft-export`, `weft-render` and `weft-spec` with a copy of `AGENT-SPEC.md`, the glob-attached rule `rules/weft-files.mdc`, `mcp.json` for the weft server) listed by the root `.cursor-plugin/marketplace.json`; it links into `~/.cursor/plugins/local/weft` from a clone. The bundle exists once as sources, in the new `plugins/shared` (scripts, build, shared tests): `moon run shared:build` writes the identical `dist/` and spec copy into `plugins/claude-code` and `plugins/cursor`, because Cursor, like Claude Code, caches only the plugin's own folder (checked against Cursor 3.23.12's cache); git stores the identical files once. The Claude Code bundle is byte for byte unchanged. Format and sources (cursor.com/docs/plugins and /reference/plugins, `cursor/plugins`, checked 2026-10-05) are in `plugins/cursor/README.md`. Tests (`moon ci`): both manifests against Cursor's official JSON schemas, the bundle freshness test for both plugins, and every feature on a corpus screen from a copy of the Cursor folder; `docs/cursor-plugin.md` explains it for people. Not yet checked inside a signed-in Cursor: loading from the local folder, a skill resolving its script path, `PATH` for `node`, the browser pane opening the page, Import from Repo.

### T34. SwiftUI generator and importer

Generate SwiftUI from `.weft` and read SwiftUI source back into `.weft`, in a Rust crate `weft-swiftui` next to the core, exposed through the CLI. Approved scope: the generated code targets iOS 17 and macOS 14 or later (the data model through `@Observable` and `@Bindable`); the importer parses Swift source (no running app needed). Code that Weft generated comes back without loss; other SwiftUI code imports with a loss table, like the HTML importer. Done when every corpus screen generates Swift that compiles for iOS 17, survives Weft → SwiftUI → Weft byte-identical after formatting, and a hand-written SwiftUI sample imports with the expected losses.

Execution plan:

1. **Crate** `crates/weft-swiftui` (depends on `weft-core`, `weft-catalog`, `serde_json`). `generate(document, options) -> Result<String, Vec<Diagnostic>>` is pure Rust; `import_swiftui(source, options) -> ImportResult { document, losses, diagnostics }` (loss `{ kind, path, note }` with the loss kinds of SPEC §9) sits behind the default cargo feature `import`, which adds `tree-sitter` and `tree-sitter-swift` (the maintained Swift grammar; sources cited in `research.md`). Core and catalog stay untouched.
2. **Generator.** Input must validate in strict mode with no errors; extension elements, integer segments in binding paths and bound token props are refused with a diagnostic, never a panic. One file per screen, prefix from the screen id (`login` → `Login`): `@Observable final class LoginModel` with typed properties inferred from how each path is used (writable text → `String`, writable flag → `Bool`, `<each in>` → array of a nested item struct, member access → nested struct), `enum LoginAction: String` with the action names as raw values, `struct LoginEvent { action, id, item }`, `struct LoginTheme` with the referenced tokens as nested `CGFloat` properties (values from `--tokens`, default tokens otherwise), and `struct LoginScreen: View` with `@Bindable var model` and `var perform: (LoginEvent) -> Void`. Ids → `.accessibilityIdentifier`, labels → the control's title or `.accessibilityLabel`, writable props → `$model.path` (`$model.todos[todoIndex].done` in loops), events → `send(.action, "id", item:)` in a `Button` action, `.onTapGesture`, `.onChange(of:)`, `.onSubmit` or `.sheet(onDismiss:)`. Every kind gets a native SwiftUI form (VStack/HStack, LazyVGrid, Section, Text with `.accessibilityHeading`, AsyncImage, Button, Form, TextField/SecureField, Toggle, Picker, List, Grid/GridRow, TabView, `.sheet`, GroupBox, Menu); what SwiftUI cannot say (states, explicit default values, bound enum props, `ordered`, `required`…) travels in a few fileprivate helpers (`weftProp`, `weftHidden`, `weftEach`, `weftFieldType`) emitted in the same file. The mapping table goes into `crates/weft-swiftui/README.md`.
3. **Importer.** tree-sitter parse, then a walk of `body`: view calls and modifier chains map back to kinds and props, `send(.case, …)` maps back through the action enum's raw values, `model.…`/`$model.…`/loop variables back to bindings, `theme.…` back to tokens. Generated code returns exactly; other SwiftUI (`@State` properties, `Text`, `Button`, `Toggle`, `List`, `ForEach`, `NavigationStack`, custom views…) maps with losses (ids, bindings, actions, layout, props, kinds, text, structure), the result is placed so that it validates in lenient mode without errors, and input size, element count and depth are bounded (`W601`, `W602`).
4. **CLI.** `weft swiftui <screen.weft> [--catalog] [--tokens]` prints Swift; `weft import-swiftui <file.swift> [--catalog]` prints canonical markup and the loss table on stderr. The core catalog is the default catalog for both. Code in its own module of `crates/weft-cli` to stay clear of T31.
5. **Tests.** Every corpus screen and catalog example: generate, import, serialize, byte-equal to the source; `xcrun --sdk iphonesimulator swiftc -typecheck -target arm64-apple-ios17.0-simulator` (and macOS 14) on the generated files, skipped with a message when `xcrun` is missing; a hand-written sample in `tests/fixtures` with its expected markup and loss table; a proptest that the importer never panics and always returns a document that validates; `deps.rs` pins the new crate's dependencies; CLI end-to-end cases. `moon` `rust-test` inputs gain `/corpus/**/*`.
6. **WebAssembly.** Check `cargo build --target wasm32-unknown-unknown -p weft-swiftui --no-default-features` (generator) and with the importer; report, do not wire into `weft-wasm`.
7. Docs: `toolchain.md` rows, `research.md` parser choice, `SPEC.md` §9 "To SwiftUI"/"From SwiftUI" lines (and `AGENT-SPEC.md` only if the MCP-facing surface changes, which it does not). Verify with `moon run :test root:typecheck root:lint root:rust-test root:rust-lint`.

Result: `crates/weft-swiftui` holds the generator, which is pure Rust and builds for `wasm32-unknown-unknown` with `--no-default-features`. It also holds the importer, which sits behind the default `import` feature (tree-sitter 0.27.0 and tree-sitter-swift 0.7.4). Their C does not build for wasm32, so the importer is CLI-only and nothing is wired into `weft-wasm`.

- **Generator coverage.** All 29 core kinds, `<each>`, ids, labels, events, bindings and tokens are mapped. The mapping table is in the crate README, and SPEC §9 has "To SwiftUI" and "From SwiftUI".
- **Corpus screens.** All 42 screens (corpus, catalog examples and an edge-case fixture) typecheck with `xcrun swiftc -typecheck -swift-version 6` for iOS 17 and for macOS 14. All 42 also survive Weft → SwiftUI → Weft byte-identical, with no losses.
- **Hand-written sample.** `tests/fixtures/import/Settings.swift` is typechecked too. It imports to a pinned document with a pinned loss table.
- **Property test.** Arbitrary and Swift-shaped input never panics, and the result always validates in lenient mode.
- **CLI.** `weft swiftui` and `weft import-swiftui` take the catalog and tokens from the arguments, then `weft.json` (through the T31 loader), then the defaults. The output directory comes from `--out-dir`, then `export.swiftui.outDir` or `import.swiftui.outDir`, then stdout. Both settings are in SPEC §10.6, `settings.rs` and the schema.
- **Pins and docs.** `deps.rs` pins the crate's dependencies and requires the C parser to stay optional. `research.md` cites the parser choice, and `toolchain.md` lists the new programs and crates.
- **Not done.** No simulator accessibility run: it needs an app target and a UI test runner, so the typecheck and the pinned identifiers stand in for it. A screen with catalog-extension kinds (like `rating` in `examples/project`) or with colour tokens is refused with a message, because SwiftUI has forms only for the core kinds and dimension tokens.

### T38. Open Design plugin

A plugin for Open Design (https://open-design.ai, https://github.com/attentiondotnet/open-design), the open-source, local-first design platform that runs on top of a coding agent and has had plugins since 0.8.0. It brings Weft into Open Design:
- author screens as `.weft` with the authoring guide and the weft MCP server;
- import HTML to `.weft`;
- export to React (and the other targets once T34 and T35 land);
- render and preview pages;
- map an Open Design `DESIGN.md` design system to Weft design tokens where the two line up, with a loss list where they do not.

It reuses the scripts, the MCP server and the build in `plugins/shared` (T37), which write the same bundle into the Claude Code and Cursor plugins, instead of a copy. The plugin format and the `DESIGN.md` format follow Open Design's own docs and repository, cited with URL and the version checked. Done when the plugin installs into Open Design from a local clone, each feature works on a corpus screen, and its tests pass in `moon ci`.

Result: `plugins/open-design` is an Open Design plugin: `SKILL.md` and `open-design.json` at the folder root (checked in tests against Open Design's own manifest schema, vendored with its notice), `dist/` and `references/AGENT-SPEC.md` from the same `moon run shared:build` that writes the Claude Code and Cursor plugins, so there is still one copy of the scripts. The skill authors screens with the guide, the MCP tools when registered, or `render.js` (strict) as the fallback, and runs `import.js`, `export.js`, `render.js` and the new `design-md.js` by a path from the Skill root. `@weft/design-md` (`packages/design-md`) maps a Google Labs `DESIGN.md` (YAML frontmatter) and an Open Design `tokens.css` to DTCG 2025.10 tokens with a loss list (components, prose, other themes, computed values, `em` and unitless lengths, `none` shadows; typography line height and letter spacing are rewritten and reported); it never throws, refuses YAML aliases and oversized input, and is tested on Open Design's `minimal` system and Google's three examples (Apache-2.0, notices kept) and with property tests. Deviations from the plan: the mapped groups keep the source's names (`colors`, `rounded`, `spacing`, `typography`) so `{colors.primary}` references stay as written, instead of `color`, `radius`, `space`, `font`; and the plugin's one setting is `plugins.open-design.tokensDir` in `weft.json`. The manifest has no plugin-root variable and, in the code read, a declared MCP server is only recorded, so `dist/server.js` ships undeclared and the README says how to register it. Checks that need a running Open Design (install, doctor, granting `bash`, preview) are listed in the plugin README as not done; the canonical repository is `nexu-io/open-design`, not the one-star copy named in the card.
