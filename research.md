# Research

Sources were checked on 2026-10-03. Statements about what suits language models are design hypotheses to be measured by the benchmark (T0, T8), not established facts.

## Prior art

| Format | Fact | Source |
| --- | --- | --- |
| Design Tokens (DTCG) | First stable version 2025.10 published 2025-10-28; covers theming, modern color spaces, aliases | https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/ |
| A2UI | Declarative JSON, flat component list with id references, client-side catalog of trusted components; v0.9.1 current, v1.0 release candidate | https://a2ui.org/specification/v0.9-a2ui/ , https://a2ui.org/specification/v1.0-a2ui/ |
| MCP Apps | SEP-1865: `ui://` resources carrying sandboxed HTML (`text/html;profile=mcp-app`); accepted 2026-01-26 | https://modelcontextprotocol.io/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp |
| Playwright MCP | Agents read a YAML accessibility snapshot with per-element refs instead of screenshots | https://playwright.dev/mcp/snapshots |
| json-render | JSON/YAML spec constrained by a Zod catalog; patch/merge/diff edit modes — **unverified** (secondary source only) | https://infoq.com/news/2026/03/vercel-json-render |
| Custom Elements Manifest | Schema version 2.1.0 describes attributes, events, slots, CSS properties — **unverified** (version not checked against the repository) | https://github.com/webcomponents/custom-elements-manifest |

## Compared approaches

| Approach | For agents | Against |
| --- | --- | --- |
| Raw HTML/JSX | Most familiar syntax, full expressiveness | Cannot be validated or run safely, ambiguous semantics |
| Catalog JSON (A2UI, json-render) | Strict validation, streaming, id-addressed patches | Verbose; hierarchy is reconstructed from references |
| Accessibility snapshot | Compact, carries actions | Read-only; no layout, tokens or slots |
| Component manifest (CEM, CSF) | Exact component contract | Does not describe a screen |
| Strict markup + canonical JSON (Weft) | Reads like HTML, validates like JSON, patches by id | New syntax; benefit must be proven |

## Decision

Weft takes the last approach: catalog and trust boundary from A2UI/json-render, component contract shape from Custom Elements Manifest, roles and states from WAI-ARIA, values from DTCG, and nested markup as the model-facing surface. Declaring ARIA roles in the document makes it comparable with the accessibility snapshot of the rendered UI, so an agent reads and writes one vocabulary.

The main risk is that markup is not measurably better than catalog JSON for models. T0 carries the stop criterion.
