# Roadmap

Approved work that is not in `plan.md` yet. It covers what the research plan (`research.md`) set out and the prototype has not delivered. Stages 0–7 of that plan are done (`done.md`); stage 8, the evaluation with real models, is T8 in `plan.md`. Each item keeps its id when it moves to `plan.md`.

### T10. Host capabilities

SPEC §8 says a host advertises `{ weft, catalogs }` and an agent writes only what is advertised; no code does this. The MCP server reports its capabilities, loads catalogs, tokens and actions from configuration, and turns token and action checks on when they are given.

### T11. Streaming and incremental generation

A2UI and json-render use flat id lists so that a UI can render while a model is still writing it. Weft must show the same for nested markup: a truncated document parses into a renderable prefix with diagnostics only for the unfinished tail, and the renderer shows it. If it cannot, the trade-off is measured and written into `research.md`.

### T12. Constrained generation

A JSON Schema of the canonical form generated per catalog (kinds, props, enum values, slots), usable as a provider's structured-output schema. The benchmark compares constrained JSON generation with free Weft markup on validity and edit success.

### T13. Interoperability with A2UI and json-render

Export a Weft document to A2UI v0.9 messages and to a json-render spec, and import from both, each with a loss table in SPEC §9. Done when every corpus screen converts both ways and the losses are listed.

### T15. Second code target and catalog import

A generator for Lit web components and an importer that turns a Custom Elements Manifest (schema 2.1.0) into a Weft catalog, to show that the format is not bound to React. Done when a corpus screen renders through the Lit target with the same accessibility tree as the React renderer.

### T16. Layout vocabulary

Decide how much layout Weft describes beyond `stack` and `grid` (alignment, sizing, responsive behaviour) without becoming CSS. Spec change first, then catalog, renderer and generator.

### T17. Extension catalogs

Load several catalogs at once with namespaced kinds, and define how an extension catalog is published and found. Answers the open question of who keeps the registry.

### T18. Follow-ups from the prototype

- `fromDom` recovers slot membership from the renderer's `data-weft-slot` wrappers; SPEC §9 stops listing slots as always lost from DOM.
- Catalog fields for the validator rules that are still tied to specific kinds (`tabs.selected` names a `tab`, `screen` only at the root).
- Corpus: per-row accessible names for the Delete buttons in `data-table`; singular and plural in the `todo-list` counter.

### T32. Claude Code plugin from GitHub

The T30 plugin works only when its marketplace is added from a local clone: Claude Code copies just the plugin folder into its cache, and the `@weft/*` packages run from the repository's sources. Bundle the plugin's scripts and the MCP server into self-contained files at release, so the plugin installs from the GitHub-hosted marketplace once the repository has a remote, and add the `repository` field to `plugin.json`. Check that Claude Code Desktop finds `node` when started from the GUI. Done when `/plugin marketplace add <owner>/weft` and `/plugin install weft@weft` work on a clean machine.
