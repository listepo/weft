# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T0 | in progress | P0 | 3 | 80% | Claude Code / opus-5.5 |
| T4 | in progress | P1 | 4 | 85% | Claude Code / opus-5.5 |
| T5 | todo | P2 | 4 | 0% | |
| T6 | in progress | P1 | 3 | 0% | Claude Code / sonnet-5.5 |
| T7 | in progress | P2 | 2 | 0% | Claude Code / sonnet-5.5 |
| T8 | todo | P1 | 3 | 0% | |
| T9 | in progress | P0 | 3 | 0% | Claude Code / opus-5.5 |

### T0. Corpus and benchmark harness

Reference screens in every compared format and a harness that measures them, so the format's benefit is a number and not a belief. Done when the benchmark runs with one command and reports token counts and task success for Weft and three baselines (HTML, JSX, A2UI JSON).

Stop criterion for the whole direction: if Weft gives neither 25% fewer tokens nor 10 points more successful edits than A2UI JSON, the creator decides whether to continue.

Plan:
1. `corpus/<screen>/` × 12 screens (login, signup, settings, data table, tabs, confirm dialog, wizard step, search results, todo list, profile, menu, empty/error states), each with `screen.weft`, `screen.html`, `screen.jsx`, `screen.a2ui.json`, `data.json`.
2. `corpus/tasks.json`: per screen, edit tasks and questions with machine-checkable expectations.
3. `bench/src`: token counting (local tokenizer as a proxy; provider token counting when an API key is present), a provider interface for model runs, a report writer.
4. Verify: `node bench/src/run.ts tokens` prints the table offline; tests cover the counters and the task checker.

Remaining after the first merge: replace the hand-copied catalog in `bench/src/weft-catalog.ts` and the stand-in Weft parser with `@weft/catalog` and `@weft/core` once T2 lands; validate every `corpus/*/screen.weft` with the real validator.

### T4. React renderer

Document → React with correct ARIA, proven by comparing the accessibility snapshot of the rendered page with the roles and names the document declares. Done when every corpus screen renders and matches 100%.

Plan:
1. `packages/render-react`: `render(document, { catalog, data, actions })` → React element; one mapping per catalog kind, bindings resolved against `data`, `each` expanded, events dispatched to named actions.
2. Unknown and extension elements render as a container with their fallback role (SPEC §8).
3. `expectedTree(document, data)`: the roles and names the document declares, in the shape of an accessibility snapshot.
4. Verify: server-render tests per kind; Playwright accessibility snapshot of every catalog example equals `expectedTree`; corpus screens are wired in after T0 and T2 merge.

Remaining after the first merge (browser check passes on 30 fixtures with 0 differences): follow the T9 spec revision (`text` prop, `button.submit`, `empty` slot) and run the browser check over the parsed corpus screens.

### T5. Reverse mapping

Accessibility snapshot or DOM → Weft (lossy) and Weft → JSX source. Done when document → render → snapshot → document preserves structure, roles, states and ids, and the losses are listed in the spec.

### T6. Agent interface

Patch operations of SPEC §7 and an MCP server exposing catalog, validate, patch and render. Done when an agent completes the corpus edit tasks through the tools alone and an invalid patch is rejected with diagnostics.

Plan:
1. `packages/core/src/patch.ts`: `applyPatches(document, patches, options)` — atomic, validated, returns the new document or diagnostics.
2. `packages/mcp`: stdio MCP server with tools `weft_catalog`, `weft_validate`, `weft_format`, `weft_patch`; `weft_render` is wired in after T4 merges.
3. Verify: unit tests per patch op and failure mode; an in-memory MCP client test that completes corpus edit tasks through the tools.

### T7. Versioning and extensibility tests

Compatibility suite for SPEC §8. Done when a 0.2 document with unknown nodes is read by the 0.1 validator and renderer without failure, with warnings, and round-trips unchanged.

Plan:
1. `compat/` fixtures: documents declaring `weft="0.2"` with unknown elements, attributes, slots-in-unknown, and `x-vendor-` extensions.
2. Tests: lenient parse yields only `W4xx` warnings, strict yields errors; `serialize(parse(x))` keeps unknown content; the renderer renders them with the fallback role; a catalog-diff helper classifies catalog changes as major or minor per SPEC §8.
3. Verify: `pnpm run ci`.

### T8. Evaluation

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.

### T9. Specification revision from corpus findings

Writing the corpus exposed gaps in SPEC 0.1: text of buttons, links and menu items cannot be bound; a button cannot be marked as submitting its form; lists and tables have no empty-state content; `label` is under-specified; numeric props have no range. Done when the spec, model, catalog, validator and corpus agree on the fixes and the benchmark report is regenerated. Carried out together with the remainder of T0.

Plan:
1. SPEC and `model.ts`: `text` prop on every text-bearing component (replaces `value` on `text`/`heading`), `button.submit`, `empty` slot on `list`/`table`, `label` wording, slot placement rule, `each` child count, `min`/`max`/`integer` on `PropDef`.
2. Catalog, examples and validator follow; new diagnostic codes are added, none reused.
3. Corpus uses the new constructs in all four formats; `bench` uses `@weft/core` and `@weft/catalog` instead of its stand-ins; `bench/REPORT.md` regenerated.
4. Verify: every corpus screen and example validates in strict mode in a test; `pnpm run ci`.
