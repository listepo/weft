# Weft

An open, agent-friendly UI description format — strict markup for models, canonical JSON for tools — with a TypeScript prototype: parser, validator, catalog, renderer, importer and MCP server.

| # | Status | Priority | Complexity | Readiness | Agent |
| --- | --- | --- | --- | --- | --- |
| T4 | in progress | P1 | 4 | 85% | Claude Code / opus-5.5 |
| T5 | in progress | P2 | 4 | 0% | Claude Code / opus-5.5 |
| T6 | in progress | P1 | 3 | 90% | Claude Code / sonnet-5.5 |
| T8 | todo | P1 | 3 | 0% | |

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

Plan:
1. `packages/from-aria`: accessibility snapshot → Weft document, mapping roles back to catalog kinds through the catalog, with a list of what could not be recovered.
2. `packages/to-jsx`: Weft document → React component source with `data` and `actions` props.
3. SPEC §9 lists the lossy fields.
4. Verify: for every corpus screen, document → render → snapshot → document keeps structure, roles, states and ids; generated JSX parses and matches the document tree; `pnpm run ci`.

### T6. Agent interface

Patch operations of SPEC §7 and an MCP server exposing catalog, validate, patch and render. Done when an agent completes the corpus edit tasks through the tools alone and an invalid patch is rejected with diagnostics.

Plan:
1. `packages/core/src/patch.ts`: `applyPatches(document, patches, options)` — atomic, validated, returns the new document or diagnostics.
2. `packages/mcp`: stdio MCP server with tools `weft_catalog`, `weft_validate`, `weft_format`, `weft_patch`; `weft_render` is wired in after T4 merges.
3. Verify: unit tests per patch op and failure mode; an in-memory MCP client test that completes corpus edit tasks through the tools.

Remaining after the first merge: the `weft_render` tool, and SPEC §7 should point to `set text` for text edits once T9 lands.

### T8. Evaluation

Full benchmark run on two or three models against the baselines, and a report with a continue/stop recommendation. Done when first-try validity is at least 95%, validity after one repair cycle is at least 99%, and raw results are in the repository.
