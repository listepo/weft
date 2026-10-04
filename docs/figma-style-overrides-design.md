# Design: token style overrides

Status: **proposal, not approved.** Nothing here is implemented. This document is for the creator to approve, change or reject before any code is written. It came out of T14, the Figma round trip.

## Problem

Designers in Figma change fills, strokes, corner radius and padding all the time. The `weft-core` 0.1 catalog has no prop for any of them. Its only token-typed props are `stack.gap` and `grid.gap`. So `@weft/figma` cannot carry such an edit back into the markup. It keeps the source as written and reports a `tokens` loss.

Raw values cannot be the answer: design rule 6 says that values of design come from tokens. The question is how a document says "this element uses this token for its fill" and still keeps these rules:

- one way to say a thing (rule 2);
- a closed vocabulary (rule 1);
- byte-stable canonical form (§3).

## Proposal

### Syntax

A closed set of universal **style attributes**, each named `style-<property>`, allowed on every catalog component. A style attribute takes only a token reference:

```xml
<button id="save" style-fill="{token.color.accent}" style-radius="{token.radius.md}">Save</button>
<section id="summary" label="Summary" style-padding="{token.space.lg}" style-stroke="{token.color.border}"/>
```

| Attribute | Token type (DTCG) | Meaning | Figma field |
| --- | --- | --- | --- |
| `style-fill` | `color` | Background of the element's box | `fills[0]` color, bound to a variable |
| `style-stroke` | `color` | Border color | `strokes[0]` color |
| `style-stroke-width` | `dimension` | Border width | `strokeWeight` |
| `style-radius` | `dimension` | Corner radius, all four corners | `topLeftRadius` … `bottomRightRadius` |
| `style-padding` | `dimension` | Inner spacing, all four sides | `paddingLeft` … `paddingBottom` |
| `style-text-color` | `color` | Color of the element's own text | text layer `fills[0]` |

Rules:

- **Token references only.** A literal or a binding in a style attribute is an error. Raw values never enter the document. A value that matches no token stays a loss in importers.
- **The set is closed.** It grows only by a minor version of the format, like the catalog. A `style-` name outside the set is an unknown attribute (`W402`). In lenient mode that is a warning, and the attribute is kept.
- **No duplication.** A style attribute that would say what a catalog prop already says is not added. Spacing between children stays `gap`. Padding is not split per side: four equal values are the only form, which keeps one way to say it. Per-side padding is a later minor addition if the creator wants it.
- **Structural nodes take none.** `<slot>` and `<each>` take no style attributes, since they have no box. Extension elements take them like any other attribute.
- **Opting out.** A catalog component MAY list the style properties it accepts in a new optional `styles` field, for example `"styles": ["fill", "stroke", "radius"]`. An absent list means all of them. A component that draws no box of its own (`text`, `each`) can list none.

### Canonical JSON

Style attributes are ordinary props:

```json
{ "kind": "button", "id": "save", "props": { "style-fill": { "token": "color.accent" }, "variant": "primary" } }
```

- They sort with the other props by UTF-16 code unit, as §3 already requires, so serialization and canonicalization need no change.
- The `Value` type is unchanged. A style prop's value is always the `{ token }` form.
- Keeping them in `props` rather than in a new `style` member of `Node` means a 0.1 reader sees an unknown attribute (`W402`), keeps it and round-trips it. A new member would fail the 0.1 shape check (`W200`), and §8 forbids a minor version from breaking older readers that way.
- Patches need nothing new. `set` on a prop, with `null` to remove it, already covers style attributes.

### Validation and diagnostics

| Code | Severity | When |
| --- | --- | --- |
| `W225` (new) | error | A style attribute holds a literal or a binding instead of a token reference. |
| `W226` (new) | error | A style attribute the component's `styles` list does not accept. |
| `W306` (existing) | error | The token is not in the token set. |
| `W307` (existing) | error | The token's type does not match the property, such as a `dimension` in `style-fill`. |
| `W402` (existing) | mode | A `style-` name outside the closed set. |

The existing token checks (`W306`, `W307`) apply when tokens are given to `validate`, exactly as they do for `gap` today. Catalog diffs (`diffCatalogs`) treat the `styles` list like `allowedChildren`:

- adding a property, or removing the list, widens it, which is minor;
- removing a property, or adding a list, narrows it, which is major.

### Effect on renderers

- **render-react** writes each style attribute as an inline CSS declaration that reads the token's custom property, the same way `gap` does today:
  - `style-fill` → `background: var(--weft-color-accent)`;
  - `style-stroke` with `style-stroke-width` → `border: var(--weft-…) solid var(--weft-…)`. A stroke without a width uses `1px`, the CSS default for a visible border, so a stroke alone still shows.
  - `style-radius` → `border-radius`; `style-padding` → `padding`; `style-text-color` → `color`.
- Style attributes add nothing to the accessibility tree, so the ARIA snapshot tests stay unchanged.
- A renderer that does not know style attributes ignores them, as §8 already requires for unknown attributes. The page loses only decoration, never meaning.

### Effect on to-jsx

to-jsx emits the same declarations in the generated `style` object (`generate.ts` already does this for `gap`), keyed by the same `--weft-` custom properties. The equivalence tests compare the generated components with render-react's output, so the two must change together.

### Effect on importers

| Importer | Change |
| --- | --- |
| `@weft/figma` | **Build:** binds each style attribute's variable to the matching Figma field.<br>**Read:** a bound variable on one of these fields becomes a style attribute. A raw value that equals a token becomes a style attribute too, with the same matching and tie-break as `gap` today. Any other raw value stays a `tokens` loss.<br>**Corner radius:** Figma reports a radius binding as the four corner fields, so all four must name the same variable. Otherwise it is a loss. |
| `@weft/from-aria`, snapshot | No change. An accessibility snapshot has no styles. |
| `@weft/from-aria`, HTML | Optional. An inline style that reads a `--weft-` custom property, such as `background: var(--weft-color-accent)`, could become the style attribute. Anything else stays a `tokens` loss. Without this the HTML importer behaves as today. |
| SPEC §9 loss table | The `tokens` row gains a sentence: "a value that matches no token is not carried; a bound or matching one becomes a style attribute". |

### Version

This is a minor addition: `weft` 0.2 with catalog `weft-core` 0.2. A 0.1 reader keeps the attributes and warns. A 0.2 reader of a 0.1 document sees no difference.

## Alternatives considered

| Alternative | Why not |
| --- | --- |
| A `style` member on `Node` (`style: { fill: { token } }`) | Breaks 0.1 readers (`W200`) in a minor version, and patches would need a new target. |
| One `style="fill: {token.x}; radius: {token.y}"` attribute | A second mini-language inside a value. It breaks rule 2 and the value grammar of §2.1. |
| Per-kind props (`button.fill`, `section.padding`) | The same meaning under many names. Every catalog change would touch dozens of props, and generic tools (Figma, to-jsx) could not treat them uniformly. |
| Extension attributes (`x-figma-fill`) | They work today with no format change, but other renderers would ignore them by design. That hides a real design decision in a vendor namespace. |
| Keep reporting a loss | This is the current stage-1 behaviour. It is safe, but designers' visual work never reaches the code. |

## Open questions for the creator

1. Is the property set right? Candidates left out: per-side padding, shadow (`shadow` tokens), opacity, typography (`typography` tokens on text kinds).
2. Should `stack` and `grid`, which have role `none` and no box in the accessibility tree, accept `style-fill` and `style-padding`? Visually they are boxes in Figma.
3. Should the `styles` opt-out list exist in 0.2, or should every component accept every style property at first?
4. Should the HTML importer read `--weft-` custom properties back (optional row above)?
