# Compatibility fixtures

Fixtures for SPEC §8: a reader built for Weft 0.1 must survive documents from the future and from other vendors. They are read by `packages/core/test/compat.test.ts` and `packages/render-react/test/compat.test.ts`. Each file starts with a comment naming what it exercises. Diagnostics below are for the `weft-core` catalog; "lenient" and "strict" are the validator modes.

| Fixture | Lenient | Strict | Rendering |
| --- | --- | --- | --- |
| `unknown-element.weft` | `W403` and `W401`, warnings | the same, errors | the unknown element is a `group` between its siblings |
| `unknown-attribute.weft` | `W403` and `W402`, warnings | the same, errors | the attribute stays in the model and is ignored; the button is unchanged |
| `unknown-with-children.weft` | `W403` and `W401`, warnings | the same, errors | a `group` holding the known children and the named slot content |
| `unknown-in-restricted-parent.weft` | `W403` and `W401`, warnings; no `W302` (unknown elements are opaque) and no `W303` for the `item` inside it | the same, errors | the unknown element is a `group` named by its `label`, its `item` is rendered |
| `extensions.weft` | none | none | `x-acme-chart` takes its declared `role` (`img`); extension attributes are ignored |
| `extension-without-role.weft` | `W210`, error | `W210`, error | not readable |
| `unsupported-major.weft` | `W404`, error | `W404`, error | not readable |

The `W4xx` warnings of the 0.2 fixtures are the only diagnostics a lenient reader reports for them. Every readable fixture round-trips: `serialize(parse(x))` re-parses to an equal document and keeps every unknown element, attribute and slot, and so does the canonical JSON.
