# Development

Building Weft, the fast iteration loop and the merge gate. Setting up the tools and the repository layout: [contributing.md](contributing.md).

## Develop

```bash
mise install
pnpm install
moon ci
```

The checks run in Rust (`crates/`). `@weft/core` and `@weft/catalog` call them through WebAssembly: `moon run root:wasm` builds the module into `packages/core/wasm/`, and every test task builds it first. Node 22.3 or later, Deno and Bun load it synchronously; browsers fetch it with top-level await.

## The iteration loop and the merge gate

While you work, run only the checks the change affects:

```bash
moon run root:changed              # commit, stage and edit freely: it reads the diff against main
moon run root:changed -- --dry-run # print the selection and the commands, run nothing
```

It compares the working tree with the merge base with `main` and runs, in parallel, the Rust tests of the changed crates and every crate that depends on them (`cargo nextest -E 'rdeps(...)'`, clippy on the same crates), and the TypeScript projects the change reaches through `package.json` dependencies and paths named in sources, running only the tests that import a changed module (Vitest `--changed`) when imports are the only link. A change to the Rust core or the WebAssembly module reaches every project that loads it; a docs-only change runs nothing. When it cannot trust the selection (no merge base, a changed lockfile, toolchain pin, moon or workspace configuration, root `Cargo.toml`, or a file in `crates/` that belongs to no crate) it says why and runs the full check. The selection is best effort for files a test reads without importing them, so it is a fast loop, not a verdict.

The merge gate stays the full check, and a change merges only after it exits 0:

```bash
moon run :test root:typecheck root:lint root:rust-test root:rust-lint root:runtimes
```

`moon run root:runtimes` runs the `@weft/core` and `@weft/catalog` suites on Node, Deno, Bun and headless Chromium (`pnpm exec playwright install chromium` once). The browser leg skips the suites that read files (listed in `runtimes/vitest.config.ts`); the other three run everything.
