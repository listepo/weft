---
title: Weft
tagline: An open format for describing user interfaces that AI agents can read, write, validate and patch.
repo: https://github.com/pyrlyn/weft
install: "git clone https://github.com/pyrlyn/weft && cd weft && cargo build -p weft-cli"
version: "0.1.0"
status: "Prototype · no release yet"
accent: "#9B8CFF"
order: 7
---

<!-- Website copy for the pyrlyn project site, copied to pyrlyn/landing as
content/projects/weft.md; front matter follows CONTENT_CONTRACT.md in that repository.
Sources (checked 2026-10-14): README.md, SPEC.md, docs/README.md and docs/cli.md; version from the workspace Cargo.toml (no release yet, hence status). The accent is a placeholder: the repository has no brand colour yet. -->

## Overview

Weft is an open, agent-friendly UI description format: a strict XML subset with a closed
vocabulary for models, and the same tree as canonical typed JSON for tools, validated against a
component catalog. Roles and states come from WAI-ARIA, component contracts from Custom Elements
Manifest, design values from Design Tokens.

Weft is a prototype with no release yet: the `weft` CLI builds from the repository. It is
licensed under Apache 2.0.

## Features

- **Markup for models.** Explicit hierarchy, named slots, enumerated states and stable ids.
- **JSON for tools.** The same tree as typed data, validated against a component catalog.
- **Borrowed semantics.** WAI-ARIA roles and states, Custom Elements Manifest contracts, DTCG design tokens.
- **A CLI.** `weft validate`, `weft fmt` and `weft explain` check, format and describe a screen.
- **Plugins.** Penpot, Figma, Cursor, Claude Code and Xcode integrations live in the repository.

## Install

Build the CLI from the repository:

```bash
git clone https://github.com/pyrlyn/weft && cd weft && cargo build -p weft-cli
```

## Usage examples

Validate a screen against the catalog:

```bash
weft validate login.weft --catalog packages/catalog/catalog.json --strict
```

Format a file, and explain what a screen does:

```bash
weft fmt messy.weft --write
weft explain login.weft --catalog packages/catalog/catalog.json
```

## Links

- Repository: <https://github.com/pyrlyn/weft>
- Specification: <https://github.com/pyrlyn/weft/blob/main/SPEC.md>
- Documentation: <https://github.com/pyrlyn/weft/tree/main/docs>
- License: Apache License 2.0 (see <https://github.com/pyrlyn/weft/blob/main/LICENSE>)
