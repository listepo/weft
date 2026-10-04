# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T8 | in progress | P1 | 3 | 55% | Claude Code / claude-opus-5-5 |
| T22 | in progress | P1 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T23 | todo | P2 | 3 | 0% | |
| T24 | todo | P2 | 2 | 0% | |
| T26 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |
| T28 | in progress | P2 | 3 | 0% | Claude Code / claude-opus-5-5 |
| T29 | in progress | P1 | 3 | 0% | Claude Code / claude-sonnet-5-5 |

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

Execution plan:

1. **One binding crate, `crates/weft-wasm`** (`cdylib` + `rlib`, `wasm-bindgen = "=0.2.129"` to match the pinned CLI). It holds both packages' functions: the catalog crate depends on the core, so one module ships the core once and keeps one memory. Plain Rust functions (JSON text in, JSON text out) carry the logic and the unit tests; the `#[wasm_bindgen]` exports are one-line wrappers. A `Catalog` handle class parses a catalog once per catalog object instead of on every call. `weft-core` gains one re-export, `to_document` (JSON → model, already used by `validate`), which `serialize`, `canonicalize` and `stringify` need.
2. **What moves.** To WASM: `parse`, `validate`, `serialize`, `canonicalize`, `stringify`, `applyPatches`, `didYouMean` (core) and `loadTokens`, `diffCatalogs` (catalog). Stays TypeScript: types, zod schemas and their JSON Schema export, `DIAGNOSTIC_CODES`, `diagnostic` and `hasErrors` (the importer builds W601/W602 diagnostics with them), the rule regexes and `ARIA_ROLES`, `coreCatalog` (the source of `catalog.json`), `tokenTypes` (a projection of a map), the CLI. The TypeScript implementations that moved are deleted, so there is one implementation. `index.ts` keeps every name and signature. `ParseResult.source` is rebuilt (lazily) from positions the binding returns, and `ValidateOptions.source` is passed back, so line and column survive.
3. **Boundary: JSON text both ways.** `JSON.stringify` writes keys in JavaScript order and drops `undefined` members exactly as zod's optional fields read them, and `parse_json` already reproduces `JSON.parse`. On the way out `JSON.parse` defines a `__proto__` key as an own data property, as `Object.fromEntries` did in the TypeScript core (serde-wasm-bindgen assigns properties, which would hit the setter). Values JSON cannot carry (`NaN`, `±Infinity`, `BigInt`, functions, symbols) become a marker object the schema rejects wherever it appears, so they stay rejected instead of turning into `null`; cyclic input becomes text deeper than the depth limit, so `validate` reports W200 as before. Lone surrogates, which UTF-8 cannot carry, become U+FFFF: also outside XML and also one UTF-16 unit, so W113/W221 fire at the same place.
4. **Loading.** `wasm-pack build --target web`. `packages/core/src/wasm.ts` instantiates it with `initSync` from bytes read through `process.getBuiltinModule("node:fs")` (Node ≥ 22.3, Deno, Bun) and falls back to `await init()` (fetch, streaming compile) where there is no file system, as in browsers. Every exported function stays synchronous after import. `@weft/catalog` reaches the module through an internal `@weft/core/wasm` subpath.
5. **Build.** A `root:wasm` moon task (inputs: crates, `Cargo.*`, `catalog.json`; outputs: `packages/core/wasm/`) that every `test` task and `root:typecheck` depend on. The output is generated and gitignored. wasm-opt is off: wasm-pack would download binaryen, which mise does not pin.
6. **Tests.** The TypeScript differential fixtures now come from the WASM build, so they keep proving the binding reproduces byte for byte what the TypeScript core produced. `crates/weft-cli/tests/deps.rs` gains an assertion for `weft-wasm`. Verify with `moon run :test root:typecheck root:lint root:rust-test root:rust-lint`, test counts compared with the baseline (core 152, catalog 122, from-aria 107, mcp 32, render-react 154, to-jsx 139, bench 76).
7. **Docs.** `AGENTS.md` (build step for these two packages, layout), `README.md` (build command), `toolchain.md` (`wasm-bindgen` crate).

### T23. Native Node and Bun addon

A napi-rs addon of the same Rust core for Node and Bun, chosen at load time with the WASM build as the fallback when no prebuilt binary fits the platform. Done when the test suite passes on both builds and a broken or missing addon falls back to WASM with a warning.

### T24. Runtime matrix

Run the binding tests in Node, Deno, Bun and a headless browser through moon. Done when all four pass from one command.

### T26. Vitest for the TypeScript tests

Run every TypeScript test suite with Vitest (on Vite) instead of `node:test`: the 28 test files in `packages/*/test` and `bench/test` move to Vitest's API, `fast-check` stays for property tests, and each package's moon `test` task calls `vitest run`. Vitest's browser mode then serves the browser leg of T24, and the same suites check the WASM build of T22. `AGENTS.md` changes its test rule from `node:test` to Vitest. Done when every existing test passes under Vitest with the same count, `moon ci` is green, and `node:test` is no longer used.

### T28. Binding readback against inverted conditions

In the Bonsai edit smoke run (`login.e2`) the model was asked to disable Sign in while `$.busy` is true. It changed `{!$.email}` to `{!$.busy}` and kept the `!`. The markup is valid, but the condition is inverted. The HTML and JSX baselines got it right. Validation cannot see intent. The model can, if the core tells it in plain words what a binding means. Depends on T20.

1. `weft-core`: `explain(document)` reads every binding as a sentence, e.g. `button#submit disabled: while $.busy is false`. `explain_changes(before, after)` lists only the props, events and bindings that changed.
2. CLI: `weft explain <file> [--against <old-file>]`.
3. `AGENT-SPEC.md` repair loop: before answering, read back the changed bindings and compare them with the instruction.
4. Benchmark: an optional readback turn after a valid edit, the same for every format. This is a method change, recorded in `test.md`. Rerun `login` on Bonsai with 3 samples.

Done when the tests for `explain` pass, the CLI prints readbacks, and the rerun is in the `test.md` history.

### T29. Rust core test suite

The Rust core (T20) and catalog (T21) are checked mostly by the differential fixtures, which prove agreement with TypeScript but not the claims themselves, and they cover only inputs the generators reach. This task gives `weft-core` and `weft-catalog` their own tests: unit tests named as claims for every public function, and property tests that no input panics, that parse → serialize → parse is stable, and that formatting is idempotent. Done when every public function has tests for its documented behaviour, the property tests run in `cargo nextest`, and `moon ci` is green.

