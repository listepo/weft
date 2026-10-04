# Contributing

This page gets you from a clone to a green build and tells you where things live. The rules for changing Weft, including the ones agents follow, are in [AGENTS.md](../AGENTS.md); read it before your first change and do not expect this page to repeat it.

## The tools

Four programs matter, and each has one job:

| Program | What it is for |
| --- | --- |
| [mise](https://mise.jdx.dev) | Installs every tool at the pinned version (`mise.toml`): Node, pnpm, Rust with the WebAssembly target, moon, Deno, Bun, `cargo-nextest`, `wasm-pack`, `wasm-bindgen`. |
| [pnpm](https://pnpm.io) | Installs the JavaScript packages. |
| [moon](https://moonrepo.dev) | Runs the tasks (tests, type check, lint, the WebAssembly build) and caches their results. |
| Cargo | Builds and tests the Rust crates. |

[toolchain.md](../toolchain.md) lists every program and package with the reason it is there.

## Set up

```bash
mise install
pnpm install
moon run root:wasm
```

`mise install` installs the tools, `pnpm install` the packages, and the last command builds the Rust core as WebAssembly into `packages/core/wasm/`. That folder is generated and ignored by git. The TypeScript packages load it, so nothing that imports `@weft/core` works until it exists. Every moon `test` task and `root:typecheck` build it first, and moon rebuilds it when a crate changes. You only run it by hand before running a test with Vitest directly.

If `node`, `cargo` or `moon` is not found, your shell is not set up for mise. Put `mise exec --` in front of the command.

## The full check

This is what must pass before a change is done:

```bash
moon run :test root:typecheck root:lint root:rust-test root:rust-lint
```

| Task | What it runs |
| --- | --- |
| `:test` | The Vitest suite of every package. |
| `root:typecheck` | `tsc --noEmit` over the whole repository. |
| `root:lint` | `oxlint --deny-warnings` and `oxfmt --check` on TypeScript and JSON. |
| `root:rust-test` | `cargo nextest run --workspace`. |
| `root:rust-lint` | `cargo clippy -- -D warnings` and `cargo fmt --check`. |

`moon ci` runs the type check, the lint and every package's tests, cached, and `moon run root:fmt` formats TypeScript and JSON. A task lists every file it reads in its `inputs`; a task that forgets one serves a stale cached result, so add the file to `inputs` when you add a new kind of dependency.

To work on one piece:

```bash
moon run core:test                       # one package (core, catalog, from-aria, mcp, render-react, to-jsx, claude-code, bench)
cargo nextest run -p weft-core           # one crate
cd packages/core && pnpm exec vitest run --config ../../vitest.config.ts test/cli.test.ts   # one test file
```

## Where things live

| Path | What is there |
| --- | --- |
| `SPEC.md` | The format. It is the contract: code follows it, never the other way round. |
| `AGENT-SPEC.md` | The format written for a model: how to write, patch and repair. |
| `crates/weft-core`, `crates/weft-catalog` | The rules in Rust: parser, validator, patches, `explain`, the core catalog, tokens. |
| `crates/weft-cli` | The `weft` program. |
| `crates/weft-wasm` | The WebAssembly binding of the two crates. |
| `packages/core`, `packages/catalog` | TypeScript types and loaders over the Rust build; the catalog as data and the default tokens. |
| `packages/render-react`, `from-aria`, `to-jsx`, `mcp` | Renderer, importer, React generator, MCP server. |
| `plugins/claude-code` | The Claude Code plugin and its marketplace entry (`.claude-plugin/` at the root). |
| `corpus/` | Twelve reference screens, each in four formats, with data and tasks. |
| `compat/` | Fixtures for the versioning rules. |
| `bench/` | The benchmark harness; method and history in [test.md](../test.md). |
| `plan.md`, `todo.md`, `done.md`, `roadmap.md`, `ideas.md` | Active, upcoming and finished work. |

## Where tests live

- **TypeScript:** `<package>/test/*.test.ts`, run by Vitest with one `vitest.config.ts` at the root. Tests use `describe` and `test` from `vitest` and assert with `node:assert/strict`; property tests use `fast-check`.
- **Rust:** unit tests inside `crates/*/src`, integration tests in `crates/*/tests`, run by `cargo nextest`. `crates/weft-cli/tests/cli.rs` runs the real binary.
- **Differential fixtures:** `crates/weft-core/tests/fixtures/differential.json` and the catalog's equivalent are read by a Rust test and a Vitest test, so the native build and the WebAssembly build must give the same answers ([How it works](how-it-works.md#what-keeps-the-two-languages-in-step)). Change them only on purpose, with `WEFT_UPDATE_FIXTURES=1`.
- **Corpus and compatibility:** `bench/test/conformance.test.ts` checks every corpus screen, in canonical form, against the spec; `packages/core/test/compat.test.ts` checks `compat/`.
- **Agent guide:** `bench/test/agent-spec.test.ts` fails when `AGENT-SPEC.md` misses a component or a diagnostic code, or has an example that no longer validates.

## Making a change

The habits that matter most, in short. [AGENTS.md](../AGENTS.md) has all of them.

- **The spec changes first.** A behavior the spec does not describe starts with a `SPEC.md` edit in the same commit, and a change to the spec, catalog, diagnostics, patches or MCP tools updates `AGENT-SPEC.md` too.
- **Rules live in Rust.** A change to parsing, validation, patches or the catalog goes into the crates, never into a TypeScript copy.
- **Diagnostics are an API.** A published code never changes meaning.
- **Documents are untrusted input.** Parsers and validators never throw on bad input.
- **Docs are tested by hand.** Every command in this guide was run as printed. When you change behavior that a page shows, rerun its commands and update the output.
