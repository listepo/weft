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

Status: prototype. The format is specified in [SPEC.md](SPEC.md); the reasoning is in [research.md](research.md).

## Develop

```bash
mise install
pnpm install
moon ci
```

## License

Apache License 2.0 — see [LICENSE](LICENSE). Copyright 2026 Ivan Tugay.
