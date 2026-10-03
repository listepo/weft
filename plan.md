# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T5 | in progress | P2 | 4 | 0% | Claude Code / opus-5.5 |
| T8 | todo | P1 | 3 | 0% | |

### T5. Reverse mapping

Accessibility snapshot or DOM → Weft (lossy) and Weft → JSX source. Done when document → render → snapshot → document preserves structure, roles, states and ids, and the losses are listed in the spec.

Plan:
1. `packages/from-aria`: accessibility snapshot → Weft document, mapping roles back to catalog kinds through the catalog, with a list of what could not be recovered.
2. `packages/to-jsx`: Weft document → React component source with `data` and `actions` props.
3. SPEC §9 lists the lossy fields.
4. Verify: for every corpus screen, document → render → snapshot → document keeps structure, roles, states and ids; generated JSX parses and matches the document tree; `pnpm run ci`.

### T8. Evaluation

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.
