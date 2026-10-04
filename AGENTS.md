# Weft — instructions for agents

**What this is.** An open format for describing UI that AI agents can read, write, validate and patch: strict XML-subset markup for models, canonical JSON for tools, one closed vocabulary defined by a catalog. This repository holds the specification and the TypeScript prototype.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `SPEC.md` — the format. It is the contract; code follows it, never the other way round.
- `AGENT-SPEC.md` — the format as an AI agent uses it: writing, patching, repairing each diagnostic.
- `plan.md` — active tasks and their execution plans.
- `research.md` — prior art and the reasons behind the design.

## Rules

- **The spec changes first.** A behaviour change that the spec does not describe starts with a `SPEC.md` edit in the same commit.
- **`AGENT-SPEC.md` stays current.** A change to `SPEC.md`, the core catalog, the diagnostic codes, the patch operations or the MCP tools updates `AGENT-SPEC.md` in the same commit, and the MCP primer (`packages/mcp/src/primer.ts`) must not contradict it. `bench/test/agent-spec.test.ts` catches a missing component or code and an example that no longer validates; the rest is on the author of the change. The benchmark's Weft primer (`bench/src/primers.ts`) changes only as a method change recorded in `test.md`.
- **`packages/core/src/model.ts` is the shared contract.** Change it only together with `SPEC.md`.
- **Documents are untrusted input.** Parsers and validators never throw on bad input; they return diagnostics. No `eval`, no dynamic code, no network access from a document.
- **Diagnostics are an API.** A published code never changes meaning. Every diagnostic carries path, expectation and, where one exists, a hint.
- **TypeScript has no build step.** Node runs the TypeScript sources directly (type stripping), so only erasable syntax: no `enum`, no `namespace`, no parameter properties. Import with the `.ts` extension. The one exception is the plugins: Claude Code and Cursor each copy only their own plugin folder into their caches, so the scripts and MCP server (with the WebAssembly core) in `plugins/shared` are bundled with Vite into the committed `dist/` of `plugins/claude-code` and of `plugins/cursor`, from one build (`moon run shared:build`; a test fails while either `dist/` or an `AGENT-SPEC.md` copy is stale). Never edit `dist/` by hand, and never add a second copy of the scripts to a plugin.
- **`@weft/core` and `@weft/catalog` run on the Rust core.** Their checks live in `crates/weft-core` and `crates/weft-catalog`; `crates/weft-wasm` binds them, and `moon run root:wasm` (wasm-pack) generates `packages/core/wasm/` and, with the web importers and generators of `crates/weft-web` (`@weft/from-aria`, `@weft/to-jsx`), `packages/core/wasm-web/` (loaded by `@weft/core/web`); both are gitignored. The plugin bundles redirect the core loader to the web one and ship that module only. Every `test` task and `root:typecheck` depend on it, so `moon` rebuilds the module when a crate changes; running Vitest directly needs it built first. A behaviour change goes into the Rust crates, never into a TypeScript copy.
- **Every new tool option gets a `weft.json` key in the same change.** A flag of the CLI, an argument of a script or an MCP server setting that a user can choose is also a setting of the project file: a row in `SPEC.md` §10.6, an entry in the table of `crates/weft-catalog/src/settings.rs` (`WEFT_UPDATE_FIXTURES=1 cargo test -p weft-catalog --test schema` regenerates `schemas/weft.schema.json`), and the tool reading it with the precedence argument > `weft.json` > default. A new export or import target adds its section under `export.<target>` or `import.<target>`.
- **Tests** run with Vitest (`describe`, `test`, hooks from `vitest`) and assert with `node:assert/strict`; property tests use `fast-check`. Vitest has no subtests: one `describe` per case with a `test` per check. One `vitest.config.ts` at the root serves every package. Test files live in `<package>/test/*.test.ts`.
- **Measurement history.** Every benchmark run that is kept, full or partial, adds one row to the History table in `test.md` in the same commit as its raw results in `bench/results/`: date, commit the run used, provider and model, run, samples, headline rates, results file, notes. Rows are never edited or removed; a re-score or a rerun adds a new row. A change to the method (tasks, checks, prompt, criteria) updates the method sections of `test.md` in the same commit.
- **Layout.** `crates/` (the Rust core, catalog, CLI and `weft-wasm` bindings), `packages/core` (model types, the WebAssembly wrappers, CLI), `packages/catalog` (core catalog and tokens), `corpus/` (reference screens in every compared format), `bench/` (benchmark harness).

## Commands

```bash
mise install       # every program, at the pinned versions
pnpm install
moon ci            # typecheck, lint, every package's tests (cached); `pnpm run ci` does the same
moon run core:test # one package
# one file, from the package folder: pnpm exec vitest run --config ../../vitest.config.ts test/<file>.test.ts
moon run root:fmt  # format
```

Tasks live in `moon.yml` (root checks) and `.moon/tasks/all.yml` (the `test` task every package inherits). A task's `inputs` must list every file it reads, or moon serves a stale cached result.
