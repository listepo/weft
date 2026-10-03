# Done

### T1. Semantic model and specification v0

The contract everything else builds on: node kinds, roles, states, slots, bindings, actions, token references, catalog shape, diagnostics, patches and versioning rules. Delivered as `SPEC.md` and the Zod model in `packages/core/src/model.ts`.

### T3. Core catalog and tokens

The `weft-core` 0.1 catalog of SPEC §5.1 as data, and Design Tokens resolution. Delivered as `packages/catalog`: `coreCatalog` with all 29 components, the generated `catalog.json`, a DTCG 2025.10 loader with alias resolution and problem reports, a default token set, and one example per component.

### T2. Parser, serializer, validator

Markup ↔ canonical JSON and the three validation layers of SPEC §6 with repair-oriented diagnostics. Delivered in `packages/core`: a strict tokenizer, `parse`, `serialize`, `canonicalize`, `validate` with lenient and strict modes, the diagnostic code registry (SPEC §6.2), JSON Schema export and the `weft validate` / `weft fmt` CLI. The round-trip property holds on 10 000 generated documents.
