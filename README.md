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

A benchmark checks how reliably models read and write Weft next to semantic HTML, React JSX and A2UI JSON: 12 reference screens in all four formats, 36 edit tasks and 24 questions, each run three times, with one repair cycle that feeds the validator's diagnostics back. It reports validity, task success and output size per model and format.

The current run (T8) uses Bonsai 27B served locally by LM Studio, after a first round on Claude Sonnet 5.5, Opus 5.5 and Haiku 4.5. The method, how to run it and the history of every measurement are in [test.md](test.md).

## Develop

```bash
mise install
pnpm install
moon ci
```

The checks run in Rust (`crates/`). `@weft/core` and `@weft/catalog` call them through WebAssembly: `moon run root:wasm` builds the module into `packages/core/wasm/`, and every test task builds it first. Node 22.3 or later, Deno and Bun load it synchronously; browsers fetch it with top-level await.

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 Ivan Tugay.
