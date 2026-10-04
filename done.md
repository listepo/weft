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

### T20. Rust core

Port `packages/core` (semantic model, parser, serializer, validator, patch operations, diagnostics, CLI) to a Cargo crate. `SPEC.md` stays the contract and diagnostic codes keep their meaning. Done when the Rust core passes the existing core test suite through the bindings of T22 plus its own `cargo test`, and the CLI behaves like the TypeScript one.

Result: `crates/weft-core` and `crates/weft-cli` (binary `weft`, `validate` and `fmt`). A differential fixture written by the TypeScript core holds 1288 cases (622 markup, 357 JSON, 309 patch lists: SPEC examples, corpus, compat, catalog examples, every diagnostic case, patch failures and seeded fuzz); the Rust core reproduces all of them byte for byte. 51 Rust tests: unit tests, the differential check, seven runs of the real binary and a `cargo metadata` check that only the binary depends on clap and anyhow. `moon ci` runs `cargo nextest`, `clippy -D warnings` and `fmt --check`. Known gap: a JSON number that overflows `f64` (`1e400`) is Infinity for `JSON.parse` but a parse error for serde_json.

Execution plan:

1. Cargo workspace at the root (`Cargo.toml`, `members = ["crates/*"]`): `crates/weft-core` (library, no I/O) and `crates/weft-cli` (the `weft` binary: clap, anyhow, file I/O). Dependencies: serde, serde_json (`preserve_order`), indexmap, ryu-js (numbers printed as JavaScript prints them, so canonical JSON and messages match byte for byte), thiserror. Grammars are small hand-written matchers instead of the regex crate, which would add weight to the WASM build of T22.
2. Modules mirror `packages/core/src`: `model`, `rules`, `diagnostics`, `values`, `syntax`, `canonical`, `parse`, `serialize`, `shape` (the JSON shape checks that zod does in TypeScript, same paths and messages), `validate`, `patch`, `json`. Positions count UTF-16 code units and keys sort by UTF-16 code unit, as `SPEC.md` says; object keys keep JavaScript's order.
3. Tests: the TypeScript core suite's cases ported to `cargo test` (every diagnostic code, round-trips, patches, CLI); a differential check runs the TypeScript and Rust cores over the corpus, the examples and the compat fixtures and compares documents and diagnostics.
4. moon tasks for `cargo test`, `cargo clippy -D warnings` and `cargo fmt --check` in `moon ci`; `toolchain.md` and the shared `rust.md` list the crates.
5. The TypeScript suite running against the Rust core waits for the bindings of T22; this task ends with the Rust crate and CLI matching the TypeScript ones.
