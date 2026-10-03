# Research

The question: what open format should describe user interfaces so that AI agents can read, write, validate and edit them — something between JSON and HTML. This file holds the prior art, the comparison, the chosen direction, and what the prototype has measured so far.

Every external fact carries its primary source and the date it was checked. Statements about what suits language models are design hypotheses; the benchmark (`bench/`, task T8) is what confirms or refutes them. Measurements quoted here come from this repository and name the file they come from.

## 1. Existing formats

| Format | What it describes | Fact | Source (checked) |
| --- | --- | --- | --- |
| Design Tokens (DTCG) | Design values: color, dimension, typography, themes, aliases | First stable version 2025.10, published 2025-10-28 | https://www.w3.org/community/reports/design-tokens/CG-FINAL-format-20251028/ (2026-10-03) |
| WAI-ARIA 1.2 | Roles, states and properties of accessible UI | W3C Recommendation of 2023-06-06; "an ontology of roles, states, and properties" | https://www.w3.org/TR/wai-aria-1.2/ (2026-10-03) |
| ARIA Authoring Practices Guide | Keyboard interaction and ARIA semantics per widget | About 30 patterns (tabs, dialog, combobox, grid, menu, …), each with keyboard and role/state guidance | https://www.w3.org/WAI/ARIA/apg/patterns/ (2026-10-03) |
| JSX | Markup inside JavaScript | "A syntax extension for JavaScript"; JSX turns into JavaScript | https://react.dev/learn/writing-markup-with-jsx (2026-10-03) |
| Lit templates | Web component templates | Written as JavaScript template literals tagged with `html`, with JavaScript expressions | https://lit.dev/docs/templates/overview/ (2026-10-03) |
| Svelte | Component files | A compiler turns components written in HTML, CSS and JavaScript into JavaScript | https://svelte.dev/docs/svelte/overview (2026-10-03) |
| MDX | Prose with components | Markdown plus JSX, JavaScript expressions and ESM import/export; compiled to JavaScript | https://mdxjs.com/docs/what-is-mdx/ (2026-10-03) |
| Storybook CSF | Named component states as examples | CSF 3 is current: default export holds metadata, each named export is a story, `args` are named inputs; "an open standard based on ES6 modules" | https://storybook.js.org/docs/api/csf (2026-10-03) |
| Custom Elements Manifest | Contract of a web component | Schema 2.1.0, released 2024-05-06; covers tag name, attributes, properties, events, CSS variables and parts | https://github.com/webcomponents/custom-elements-manifest (2026-10-03) |
| A2UI | Agent-generated UI as JSON | Flat component list with id references, client-side catalog of trusted components, streaming messages; v0.9.1 current, v1.0 release candidate | https://a2ui.org/specification/v0.9-a2ui/ , https://a2ui.org/specification/v1.0-a2ui/ (2026-10-03) |
| json-render | Agent-generated UI as JSON or YAML | Apache-2.0; catalog of components and actions defined with Zod (`defineCatalog`); flat spec `{ root, elements }` with child id lists; YAML wire format with a streaming parser; renderers for React, Vue, Svelte, Solid, React Native and others | https://github.com/vercel-labs/json-render (2026-10-03) |
| MCP Apps | Delivery of UI from MCP servers | SEP-1865: `ui://` resources carrying sandboxed HTML (`text/html;profile=mcp-app`); accepted 2026-01-26 | https://modelcontextprotocol.io/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp (2026-10-03) |

Two observations follow from the table. JSX, Lit, Svelte and MDX are code: a document in them is a program, so it cannot be validated against a closed vocabulary or rendered from an untrusted source without executing it. A2UI and json-render are data constrained by a catalog, which is exactly the trust boundary an agent-facing format needs, but both encode the tree as a flat map of ids.

## 2. What makes a format convenient for language models

These are the properties Weft was designed for. Each is a hypothesis; the column on the right says how the prototype tests it.

| Property | Why it should help a model | How it is tested |
| --- | --- | --- |
| Closed vocabulary, one meaning per element | No guessing what a `div` with classes means | Strict validation rejects unknown kinds (`W401`, `W2xx`) |
| One way to say a thing | Fewer variants to learn and to compare | Canonical form: `serialize(parse(x))` is byte-stable; tested on 10 000 generated documents |
| Explicit hierarchy | Nesting is visible locally instead of reconstructed from id references | Token cost and edit success against A2UI (T8) |
| Enumerated states | `state="loading"` instead of flag combinations | Catalog declares states; `W203` on undeclared ones |
| Named slots | The model sees where content may go, the validator can check it | `W207`, `W208` |
| Stable ids on every element | Edits and actions can address an element | Patches by id (SPEC §7) |
| No code | Bindings and named actions only; safe to render from an untrusted source | Injection tests in the renderer and JSX generator |
| Diagnostics written for repair | Path, expectation and the nearest valid value make one repair cycle enough | Repair-cycle validity target in T8 |
| Low token cost | Cheaper context, more room for the task | `bench/REPORT.md` |

## 3. Validation approaches

| Approach | For | Against | Source (checked) |
| --- | --- | --- | --- |
| JSON Schema (current version 2020-12) | Standard; accepted by provider structured-output modes | Context rules ("`tab` only inside `tabs`") are awkward; error messages are generic | https://json-schema.org/specification (2026-10-03) |
| Custom schema language | Exact nesting rules | Its own tooling; nobody knows it | — |
| Type system (Zod, TypeScript) | One source for types, runtime checks and JSON Schema export | Tied to the TypeScript ecosystem | json-render uses it: https://github.com/vercel-labs/json-render (2026-10-03) |

Chosen: Zod as the source of truth (`packages/core/src/model.ts`), JSON Schema as an export (`catalogJsonSchema`, `documentJsonSchema`), and a separate semantic layer for what neither expresses well — unique ids, parent and child rules, loop scope, tokens, actions, numeric ranges. Diagnostics are an API with stable codes (SPEC §6.2).

## 4. How agents interact with UI today

| Channel | For agents | Against | Source (checked) |
| --- | --- | --- | --- |
| Screenshots | Works on anything; sees the real rendering | High token cost (image tokens); coordinates are approximate and break on layout changes | https://playwright.dev/mcp/snapshots (2026-10-03) |
| DOM and selectors | Exact and complete | Large and noisy; selectors break | — |
| Accessibility tree (Playwright MCP snapshot) | "A structured tree of accessible elements with refs for interaction"; text only, low cost, deterministic | Depends on the quality of the page's markup; read-only | https://playwright.dev/mcp/snapshots (2026-10-03) |
| Figma API | `GET /v1/files/:key` and `GET /v1/files/:key/nodes` return the node tree with layout and auto-layout properties, component metadata and annotations | Describes design intent, not a running UI; layers are not semantics | https://developers.figma.com/docs/rest-api/file-endpoints/ (2026-10-03) |

Agents read UI through the accessibility tree and write it as JSX or HTML: two vocabularies for one object. Weft declares ARIA roles in the document, so what an agent writes can be compared with what a user of assistive technology — or a browsing agent — reads back. The prototype does this comparison in Chromium (`packages/render-react`).

## 5. Compared approaches

| Approach | For agents | Against |
| --- | --- | --- |
| A. Raw HTML or JSX | Most familiar syntax; full expressiveness | Cannot be validated or rendered safely; ambiguous semantics |
| B. Catalog JSON (A2UI, json-render) | Strict validation, streaming, id-addressed patches, trust boundary | Verbose; hierarchy is reconstructed from references |
| C. Accessibility snapshot | Compact; carries actions | Read-only: no layout, tokens or slots; no way back to code |
| D. Component manifest (Custom Elements Manifest, CSF) | Exact component contract | Describes components, not screens |
| E. Strict markup with a canonical JSON form (Weft) | Reads like HTML, validates like JSON, patches by id | New syntax that has to prove its benefit |

## 6. Directions considered

1. **A profile on top of A2UI** — A2UI as is, plus ARIA semantics and DTCG token references. Low risk; verbosity and the flat graph remain.
2. **Strict markup with canonical JSON** — one semantic model, two serializations: markup for models, JSON for tools.
3. **A writable accessibility snapshot** — extend the snapshot format so it can be written as well as read. Reading and writing coincide, but it is web-only and tied to one tool's format.

## 7. Decision

Direction 2, named Weft. It takes the catalog and trust boundary from A2UI and json-render, the component contract shape from Custom Elements Manifest, roles and states from WAI-ARIA and the APG patterns, design values from DTCG, and nested markup as the model-facing surface. Because roles live in the document, an agent reads and writes one vocabulary.

The main risk was that markup is not measurably better than catalog JSON for models. The stop criterion: if Weft gives neither 25% fewer tokens nor 10 points more successful edits than A2UI JSON, the creator decides whether to continue.

## 8. What the prototype has shown

Implemented (details in `done.md`): specification 0.1 (`SPEC.md`), parser, serializer, validator and CLI (`packages/core`), the core catalog of 29 components and a DTCG token loader (`packages/catalog`), a React renderer checked in Chromium (`packages/render-react`), importers from accessibility snapshots and DOM (`packages/from-aria`), a JSX generator (`packages/to-jsx`), patches and an MCP server with six tools (`packages/mcp`), a forward-compatibility suite (`compat/`), and a corpus of 12 screens in four formats with a benchmark harness (`corpus/`, `bench/`).

Measured (`bench/REPORT.md`, proxy tokenizer o200k_base, 12 screens):

| Format | Tokens | Weft as a share |
| --- | ---: | ---: |
| Weft | 2529 | — |
| HTML | 2536 | 100% |
| JSX | 2980 | 85% |
| A2UI v0.9 | 7734 | 33% |

- Weft needs 67% fewer tokens than A2UI JSON, so the token half of the stop criterion is met. Against HTML there is no size advantage; any advantage there has to come from validation and edit accuracy.
- The tokenizer is a proxy, not Claude's. Exact counts need an API key.
- A2UI's basic catalog v0.9 has no table, switch, menu, link, alert or form submit; the corpus approximates them, so the comparison is partly approximate (`corpus/README.md`).
- Writing the corpus exposed gaps in the first draft of the spec (bound text on buttons, submit buttons, empty states, numeric ranges); they were closed in T9.
- Round trips hold: document → render → accessibility snapshot or DOM → document keeps structure, roles and states on all 12 screens, and ids through DOM. Generated JSX renders the same HTML as the reference renderer on 12 screens and 29 catalog examples.

Not yet measured: how well real models generate and edit Weft compared with the baselines — first-try validity, validity after one repair cycle, and edit success. That is T8 in `plan.md`.

## 9. Open questions

| Question | State | Where |
| --- | --- | --- |
| Format versioning | Answered for 0.x: minor versions only add; lenient readers warn and keep unknown content; catalog changes are classified major, minor or none | SPEC §8, `compat/`, `diffCatalogs` |
| Extensibility without breaking agents | Answered: `x-<vendor>-` extensions with an ARIA fallback role; unknown elements render as `group` | SPEC §8 |
| Mapping to code and back | Partly: React renderer and JSX generator; importers from DOM and snapshots with a documented loss table. No other code targets, no import from existing code | SPEC §9, `roadmap.md` |
| Streaming and incremental generation | Open. Flat id lists in A2UI and json-render exist for progressive rendering; nested markup has to show it can do the same | `roadmap.md` |
| Host capabilities | Specified (a host advertises its catalogs), not implemented | SPEC §8, `roadmap.md` |
| Layout without becoming CSS | Open: only `stack` and `grid` | `roadmap.md` |
| Who keeps a registry of extension catalogs | Open | `roadmap.md` |
| Figma as a source | Open | `roadmap.md` |
