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

### T29. Rust core test suite

The Rust core (T20) and catalog (T21) are checked mostly by the differential fixtures, which prove agreement with TypeScript but not the claims themselves, and they cover only inputs the generators reach. This task gives `weft-core` and `weft-catalog` their own tests: unit tests named as claims for every public function, and property tests that no input panics, that parse → serialize → parse is stable, and that formatting is idempotent. Done when every public function has tests for its documented behaviour, the property tests run in `cargo nextest`, and `moon ci` is green.

Result: tests went from 68 to 438 in `cargo nextest run --workspace` (`weft-core` 42 → 326, of which 21 are property tests; `weft-catalog` 16 → 102, of which 9 are property tests; `weft-cli` unchanged at 10). The claims are integration tests over the public API: `json`, `markup`, `validate`, `canonical`, `model`, `diagnostics`, `source` and `patch` in `weft-core`, `tokens`, `diff` and `core` in `weft-catalog`, and `codes.rs`, which proves that every code of the SPEC table is registered, summarised and produced by at least one input. Property tests (`proptest` 1.11.0, fixed seed, 96 to 128 cases, no persistence file) run in about a second: no panic on arbitrary strings, JSON, deep nesting or patches; parse → serialize → parse keeps the document; serialize and `stringify(canonicalize(d))` are stable; markup and its JSON round trip validate alike; remove-then-insert restores a document; a catalog never differs from itself and removals mirror additions. No bug was found where Rust differs from TypeScript, so there is no `fix:` commit. Findings that both cores share were reported to the creator and not changed: validating a document at the depth limit overflows a 2 MiB test-thread stack in a debug build, JSON documents are accepted up to about 384 element levels while markup stops at 256, `integer` is missing from the catalog diff's known prop fields, and a `$type` that is not a string on a token itself is ignored without a problem.
