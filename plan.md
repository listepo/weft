# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T0 | in progress | P0 | 3 | 0% | Claude Code / sonnet-5.5 |
| T2 | in progress | P0 | 4 | 0% | Claude Code / opus-5.5 |
| T3 | in progress | P1 | 3 | 0% | Claude Code / sonnet-5.5 |
| T4 | todo | P1 | 4 | 0% | |
| T5 | todo | P2 | 4 | 0% | |
| T6 | todo | P1 | 3 | 0% | |
| T7 | todo | P2 | 2 | 0% | |
| T8 | todo | P1 | 3 | 0% | |

### T0. Corpus and benchmark harness

Reference screens in every compared format and a harness that measures them, so the format's benefit is a number and not a belief. Done when the benchmark runs with one command and reports token counts and task success for Weft and three baselines (HTML, JSX, A2UI JSON).

Stop criterion for the whole direction: if Weft gives neither 25% fewer tokens nor 10 points more successful edits than A2UI JSON, the creator decides whether to continue.

Plan:
1. `corpus/<screen>/` × 12 screens (login, signup, settings, data table, tabs, confirm dialog, wizard step, search results, todo list, profile, menu, empty/error states), each with `screen.weft`, `screen.html`, `screen.jsx`, `screen.a2ui.json`, `data.json`.
2. `corpus/tasks.json`: per screen, edit tasks and questions with machine-checkable expectations.
3. `bench/src`: token counting (local tokenizer as a proxy; provider token counting when an API key is present), a provider interface for model runs, a report writer.
4. Verify: `node bench/src/run.ts tokens` prints the table offline; tests cover the counters and the task checker.

### T2. Parser, serializer, validator

Markup ↔ canonical JSON and the three validation layers of SPEC §6 with repair-oriented diagnostics. Done when `parse(serialize(x))` equals `x` on 10 000 generated trees and every diagnostic carries code, path and expectation.

Plan:
1. `packages/core/src/syntax.ts`: strict tokenizer checks on top of the XML parser, positions for every node and attribute.
2. `parse.ts`, `serialize.ts`, `canonical.ts`: typed literals by catalog, canonical ordering.
3. `validate.ts`: schema and semantic layers, lenient and strict modes, `diagnostics.ts` with the code registry.
4. `cli.ts`: `weft validate <file>` and `weft fmt <file>`.
5. Verify: unit tests per diagnostic code, fast-check round-trip property, `pnpm run ci`.

### T3. Core catalog and tokens

The `weft-core` 0.1 catalog of SPEC §5.1 as data, and Design Tokens resolution. Done when every component has prop schemas, slots, states and one example, and an unknown token is a validation error.

Plan:
1. `packages/catalog/src/core.ts`: the catalog, checked against `CatalogSchema`; exported JSON and generated JSON Schema.
2. `tokens.ts`: load a DTCG 2025.10 file, resolve aliases, list token paths by `$type`; `tokens/default.tokens.json`.
3. `examples/<kind>.weft` for every component.
4. Verify: tests that the catalog matches SPEC §5.1, alias cycles are reported, examples list is complete.

### T4. React renderer

Document → React with correct ARIA, proven by comparing the accessibility snapshot of the rendered page with the roles and names the document declares. Done when every corpus screen renders and matches 100%.

### T5. Reverse mapping

Accessibility snapshot or DOM → Weft (lossy) and Weft → JSX source. Done when document → render → snapshot → document preserves structure, roles, states and ids, and the losses are listed in the spec.

### T6. Agent interface

Patch operations of SPEC §7 and an MCP server exposing catalog, validate, patch and render. Done when an agent completes the corpus edit tasks through the tools alone and an invalid patch is rejected with diagnostics.

### T7. Versioning and extensibility tests

Compatibility suite for SPEC §8. Done when a 0.2 document with unknown nodes is read by the 0.1 validator and renderer without failure, with warnings, and round-trips unchanged.

### T8. Evaluation

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.
