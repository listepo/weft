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

### T14. Figma importer

Weft from a Figma file node tree (`GET /v1/files/:key/nodes`): auto-layout frames become `stack` or `grid`, component instances map to catalog kinds through a mapping file, with a loss table. Lossy by design: Figma layers carry no semantics.

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

### T30. Claude Code plugin

A Claude Code plugin, used from Claude Code Desktop, that works on `.weft` files. It lives in this repository (`plugins/claude-code`) with a marketplace manifest at the root (`.claude-plugin/marketplace.json`), so it installs with `/plugin marketplace add`. Approved scope:

- Import: an HTML file to `.weft` through `@weft/from-aria` (`fromDom`), printing the loss table.
- Export: a `.weft` file to a React component through `@weft/to-jsx`.
- Render: a `.weft` file, with optional data and tokens, to an HTML page through `renderPage` of `@weft/render-react`, then opened in the Desktop app's built-in browser for preview.
- The plugin also registers the existing MCP server (`@weft/mcp`) and a skill that teaches `AGENT-SPEC.md`, so authoring, validation and patches work in the same session. The MCP server stays file-free; file reading and writing belong to the plugin's commands.

Done when the plugin installs from the marketplace in Claude Code Desktop, the three commands work on corpus screens, and their tests pass in `moon ci`.
