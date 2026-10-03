# Weft — instructions for agents

**What this is.** An open format for describing UI that AI agents can read, write, validate and patch: strict XML-subset markup for models, canonical JSON for tools, one closed vocabulary defined by a catalog. This repository holds the specification and the TypeScript prototype.

If an `AGENTS.md` or `CLAUDE.md` exists higher in the tree, follow it too; on conflict, ask the creator.

## Read first

- `SPEC.md` — the format. It is the contract; code follows it, never the other way round.
- `plan.md` — active tasks and their execution plans.
- `research.md` — prior art and the reasons behind the design.

## Rules

- **The spec changes first.** A behaviour change that the spec does not describe starts with a `SPEC.md` edit in the same commit.
- **`packages/core/src/model.ts` is the shared contract.** Change it only together with `SPEC.md`.
- **Documents are untrusted input.** Parsers and validators never throw on bad input; they return diagnostics. No `eval`, no dynamic code, no network access from a document.
- **Diagnostics are an API.** A published code never changes meaning. Every diagnostic carries path, expectation and, where one exists, a hint.
- **No build step.** Node runs the TypeScript sources directly (type stripping), so only erasable syntax: no `enum`, no `namespace`, no parameter properties. Import with the `.ts` extension.
- **Tests** use `node:test` and `node:assert/strict`; property tests use `fast-check`. Test files live in `<package>/test/*.test.ts`.
- **Layout.** `packages/core` (model, parser, serializer, validator, CLI), `packages/catalog` (core catalog and tokens), `corpus/` (reference screens in every compared format), `bench/` (benchmark harness).

## Commands

```bash
mise install       # every program, at the pinned versions
pnpm install
moon ci            # typecheck, lint, every package's tests (cached); `pnpm run ci` does the same
moon run core:test # one package
moon run root:fmt  # format
```

Tasks live in `moon.yml` (root checks) and `.moon/tasks/all.yml` (the `test` task every package inherits). A task's `inputs` must list every file it reads, or moon serves a stale cached result.
