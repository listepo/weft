# Roadmap

Approved work that is not in `plan.md` yet. It covers what the research plan (`research.md`) set out and the prototype has not delivered. Stages 0–7 of that plan are done (`done.md`); stage 8, the evaluation with real models, is T8 in `plan.md`. Each item keeps its id when it moves to `plan.md`.

### T12. Constrained generation

A JSON Schema of the canonical form generated per catalog (kinds, props, enum values, slots), usable as a provider's structured-output schema. The benchmark compares constrained JSON generation with free Weft markup on validity and edit success.

### T16. Layout vocabulary

Decide how much layout Weft describes beyond `stack` and `grid` (alignment, sizing, responsive behaviour) without becoming CSS. Spec change first, then catalog, renderer and generator.

### T17. Extension catalogs

Load several catalogs at once with namespaced kinds, and define how an extension catalog is published and found. Answers the open question of who keeps the registry.

### T41. Hosted Penpot plugin

Penpot installs a plugin only from the URL of its `manifest.json`, so users need a hosted copy of `plugins/penpot/dist`. Once the repository is on GitHub, CI builds the plugin and deploys `dist/` to GitHub Pages, which serves the right content types and `Access-Control-Allow-Origin: *`. Done when Penpot installs the plugin from the Pages URL and the README gives that URL.
