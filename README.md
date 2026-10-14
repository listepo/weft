# Weft

An open format for describing user interfaces that AI agents can read, write, validate and patch.

- **Markup for models.** A strict XML subset with a closed vocabulary: explicit hierarchy, named slots, enumerated states, stable ids.
- **JSON for tools.** The same tree as canonical typed data, validated against a component catalog.
- **Borrowed semantics.** Roles and states from WAI-ARIA, component contracts modelled on Custom Elements Manifest, design values from Design Tokens (DTCG 2025.10).

```xml
<screen id="login" label="Sign in" weft="0.1">
  <form id="f1" on-submit="auth.submit">
    <field id="email" label="Email" required="true" type="email" value="{$.email}"/>
    <button id="go" submit="true" variant="primary">Sign in</button>
  </form>
</screen>
```

Status: prototype. The format is specified in [SPEC.md](SPEC.md); the reasoning is in [research.md](research.md). [AGENT-SPEC.md](AGENT-SPEC.md) is the guide for AI agents that read, write, patch and repair Weft. If you are a developer or designer who wants to use Weft, start with the [guide in `docs/`](docs/README.md).

## Quality test

A benchmark checks how reliably models read and write Weft next to semantic HTML, React JSX and A2UI JSON: 12 reference screens in all four formats, 36 edit tasks and 24 questions, each run three times, with one repair cycle that feeds the validator's diagnostics back and an optional readback turn that states changed bindings in plain words. It reports validity, task success and output size per model and format.

The current run (T8) uses Bonsai 27B served locally by LM Studio, after a first round on Claude Sonnet 5.5, Opus 5.5 and Haiku 4.5. The method, how to run it and the history of every measurement are in [test.md](test.md).

## Develop

```bash
mise install
pnpm install
moon ci
```

The checks run in Rust (`crates/`). `@weft/core` and `@weft/catalog` call them through WebAssembly: `moon run root:wasm` builds the module into `packages/core/wasm/`, and every test task builds it first. Node 22.3 or later, Deno and Bun load it synchronously; browsers fetch it with top-level await.

### The iteration loop and the merge gate

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

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 Ivan Tugay.
