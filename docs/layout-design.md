# Design: layout vocabulary

Status: **proposal, not approved.** Nothing here is implemented. This document is for the creator to approve, change or reject before `SPEC.md`, `AGENT-SPEC.md`, the catalog, the Rust core and the targets change. It is the design stage of T16 (T16.1). The prior art and its sources are in `research.md` §20.

## Problem

Weft has two layout kinds. `stack` lays its children out in one line, with `direction`, `gap`, cross-axis `align` and `wrap`. `grid` has a fixed number of equal `columns` and a `gap`. Most of what a layout usually says has no form:

- how the free space along a row is shared (push two buttons apart, right-align a group);
- which child takes the free space and which keeps its own size;
- inner spacing of a group, for example a glass panel;
- a maximum width, for a readable column;
- how a grid reflows on a narrow screen.

The corpus works around these gaps:

| Screen | Intent | What it does today |
| --- | --- | --- |
| `dashboard` | Title on the left, update time on the right | A wrapping row whose `gap` is `space.xl`, so the two only drift apart by a fixed amount. |
| `dashboard` | Owner on one side of the footer, links on the other | The footer is a column with `align="end"`, so everything is right-aligned instead. |
| `dashboard` | Three stat cards that stack on a phone | `grid columns="3"`: three columns at every width, squeezed on a phone. |
| `wizard-step` | Back on the left, Next on the right | A row with a `space.sm` gap: both buttons sit together on the left. |
| `glass` | Content inset in the frosted panel | Nothing: the heading and the button touch the panel's edge. |

Importers drop the same things. SPEC §9 lists them as `layout` losses: the SwiftUI importer drops `padding` and `frame`, the A2UI importer drops `justify` other than `start` and `weight`, the HTML and JSX importers drop inline layout styles, and the design tools have nowhere to put a designer's hug, fill or padding.

The task is to say more without becoming CSS. The design rules that bind it:

- a closed vocabulary (rule 1) and one way to say a thing (rule 2);
- values of design come from tokens (rule 6);
- every target (web, SwiftUI, Slint, A2UI, json-render, Figma, Penpot) either draws a form and reads it back, or lists it as a loss;
- nothing that only CSS can express;
- models' first-try validity, which the benchmark measures, must not suffer.

## Proposal

Five additions, all ordinary props in canonical JSON, plus a token group:

| Form | On | Type | Default | Meaning |
| --- | --- | --- | --- | --- |
| `justify` | `stack` (catalog prop) | enum `start`/`center`/`end`/`space-between`, literal or bound | `start` | How the free space along the stack's direction is shared. |
| `grow` | any element whose parent is a `stack` (universal attribute, §2.2) | boolean, literal only | absent = `false` | The element takes a share of its stack's free space along the stack's direction. |
| `padding` | `stack`, `grid` (catalog prop) | token, `dimension` | none | Inner spacing on all four sides. |
| `max-width` | `stack`, `grid` (catalog prop) | token, `dimension` | none | The element is as wide as its parent lets it be, but never wider than the token. |
| `min-column-width` | `grid` (catalog prop) | token, `dimension` | none | The grid shows at most `columns` columns, and fewer while a column would be narrower than the token. |
| `size.*` tokens | `packages/catalog/tokens/default.tokens.json` | `dimension` | — | Widths for `max-width` and `min-column-width`, as `space.*` holds gaps. |

Nothing changes in the syntax, the JSON shape (§3) or the patch operations (§7): these are props, written and patched like every other prop.

### Example: the dashboard

Before (`corpus/dashboard/screen.weft` today, the three sections shortened):

```xml
<screen id="dashboard" label="Team dashboard" state="ready" weft="0.1">
  <stack id="header" direction="row" gap="{token.space.xl}" wrap="true">
    <heading id="title" level="1" text="{$.team.name}"/>
    <text id="updated" text="{$.updatedAt}" tone="muted"/>
  </stack>
  <grid id="stats" columns="3" gap="{token.space.lg}">
    <section id="velocity" label="Velocity">…</section>
    <section id="bugs" label="Open bugs">…</section>
    <section id="incidents" label="Incidents">…</section>
  </grid>
  <stack id="notices" direction="column" gap="{token.space.sm}">
    …
  </stack>
  <stack id="footer" align="end" gap="{token.space.xs}">
    <text id="owner" tone="default">Owned by the platform team</text>
    <stack id="footer-links" align="start" direction="row" gap="{token.space.md}">
      <link id="docs" href="/docs">Docs</link>
      <link id="status" href="{$.statusUrl}">Status page</link>
    </stack>
  </stack>
</screen>
```

After, in canonical markup:

```xml
<screen id="dashboard" label="Team dashboard" state="ready" weft="0.1">
  <stack id="header" direction="row" gap="{token.space.md}" justify="space-between" wrap="true">
    <heading id="title" level="1" text="{$.team.name}"/>
    <text id="updated" text="{$.updatedAt}" tone="muted"/>
  </stack>
  <grid id="stats" columns="3" gap="{token.space.lg}" min-column-width="{token.size.sm}">
    <section id="velocity" label="Velocity">…</section>
    <section id="bugs" label="Open bugs">…</section>
    <section id="incidents" label="Incidents">…</section>
  </grid>
  <stack id="notices" direction="column" gap="{token.space.sm}" max-width="{token.size.lg}">
    …
  </stack>
  <stack id="footer" align="end" direction="row" gap="{token.space.md}" justify="space-between" wrap="true">
    <text id="owner" tone="default">Owned by the platform team</text>
    <stack id="footer-links" align="start" direction="row" gap="{token.space.md}">
      <link id="docs" href="/docs">Docs</link>
      <link id="status" href="{$.statusUrl}">Status page</link>
    </stack>
  </stack>
</screen>
```

- The header's `gap` is now the least space between the title and the time; `space-between` puts the time at the right edge, and on a narrow screen the row still wraps.
- The stats grid has three columns on a wide screen, two below about 3 × 240 px plus the gaps, one below about 2 × 240 px (with `size.sm` at 240 px).
- The notices keep a readable width on a wide screen.
- The footer puts the owner and the links at opposite ends of one line. The column with `align="end"` it replaces was a correct way to right-align a block; the row keeps `align="end"` (bottom-aligned across the row) because it is the only `align="end"` in the corpus and the coverage test needs it.

The changed parts in canonical JSON:

```json
{
  "kind": "stack",
  "id": "header",
  "props": {
    "direction": "row",
    "gap": { "token": "space.md" },
    "justify": "space-between",
    "wrap": true
  },
  "children": ["…"]
}
```

```json
{
  "kind": "grid",
  "id": "stats",
  "props": {
    "columns": 3,
    "gap": { "token": "space.lg" },
    "min-column-width": { "token": "size.sm" }
  },
  "children": ["…"]
}
```

### Example: the wizard step

Before and after of the navigation row in `corpus/wizard-step/screen.weft`:

```xml
<stack id="nav" direction="row" gap="{token.space.sm}">
  <button id="back" on-press="wizard.back">Back</button>
  <button id="next" disabled="{!$.plan}" variant="primary" on-press="wizard.next">Next</button>
</stack>
```

```xml
<stack id="nav" direction="row" gap="{token.space.sm}" justify="space-between">
  <button id="back" on-press="wizard.back">Back</button>
  <button id="next" disabled="{!$.plan}" variant="primary" on-press="wizard.next">Next</button>
</stack>
```

A phone layout where the two buttons share the width uses `grow` instead:

```xml
<stack id="nav" direction="row" gap="{token.space.sm}">
  <button id="back" grow="true" on-press="wizard.back">Back</button>
  <button id="next" disabled="{!$.plan}" grow="true" variant="primary" on-press="wizard.next">Next</button>
</stack>
```

`grow` is a prop like any other in JSON, sorted with the others:

```json
{
  "kind": "button",
  "id": "next",
  "props": { "disabled": { "bind": "$.plan", "not": true }, "grow": true, "variant": "primary" },
  "on": { "press": "wizard.next" },
  "children": ["Next"]
}
```

`wizard-step` is one of the twelve benchmark screens and is written in four formats (`screen.weft`, `.html`, `.jsx`, `.a2ui.json`); see **Corpus and benchmark** for why the proposal leaves the file itself unchanged.

### Example: the glass panel

```xml
<stack id="card" direction="column" gap="{token.space.md}" material="{token.material.glass}" padding="{token.space.lg}">
```

The heading, text and button now sit `space.lg` inside the frosted surface on every target.

### `justify`

- **Meaning.** The free space of a stack is its length along its direction minus its children's lengths and the gaps. `start` leaves it after the last child, `end` before the first, `center` half on each side, and `space-between` shares it equally between the children (none at the ends; a single child sits at the start). The `gap` stays the least space between two children.
- **Where free space exists.** A row takes the full width its parent gives it, as it does on every target today, so it has free space whenever its children are narrower. A column is as tall as its children unless its parent stretches it (a row with `align="stretch"`). `justify` on a column therefore matters only inside a stretched row, which SwiftUI cannot draw today (SPEC §5.1 note); see open question 4.
- **Catalog prop** of `stack`, because it belongs to the container. `grid` does not take it: its columns already share the width.
- **Values.** The four that every target draws (`research.md` §20). `space-around` has no SwiftUI form; `space-evenly` has one but no screen needs it; `stretch` is what `grow` says per child. They can be added by a later minor version.
- **Default** `start`, declared in the catalog. It is the initial value of every target, so a renderer writes nothing for it, and an importer does not write `justify="start"` back for a stack that does not say it.
- **Bindable** like `align`.

### `grow`

- **Meaning.** The element takes an equal share of its stack's free space along the stack's direction: width in a row, height in a column. Elements without `grow` keep the size of their content. With `grow` on several children, they share the free space equally; there are no ratios. A child that grows leaves no free space, so the stack's `justify` has no effect then.
- **Universal attribute** (§2.2), like the tilt attributes of T52, because it is a property of the child's place in its parent, not of its kind. As a catalog prop it would have to be declared on every kind that can sit in a stack, the same meaning under some thirty names (the reason the style-overrides proposal also rejected per-kind props). `<slot>` and `<each>` take none (§2.2); an element repeated by `<each>` inside a stack is that stack's child.
- **Boolean, literal only** (`bindable: false`): which child grows is structure, not data, like `submit`. A binding is `W217`.
- **No default value** in the attribute table: absent means it does not grow. `grow="false"` is valid and means the same, as for every boolean prop.
- **Why not a weight.** Figma (`layoutGrow` is 0 or 1), Penpot (`fill`) and SwiftUI (`frame(maxWidth: .infinity)`) have no ratios, so a number would be a loss on three targets. A2UI's `weight`, Compose's `weight` and Slint's `*-stretch` read in as `grow`.
- **Fixed size is not proposed.** Hug is what every child does without `grow`; a fixed width would need a `width` token and is left for a later minor version if a screen needs it.

### `padding`

- **Meaning.** Inner spacing on all four sides of the stack or grid, inside its material surface when it has one. Equal on all sides: per-side padding is a later minor addition if needed, as the style-overrides proposal also says.
- **Catalog prop** of `stack` and `grid`, as `gap` is. Other kinds keep the padding their renderer gives them (`section`, `dialog`, `alert`); a screen that needs a padded group wraps it in a stack.
- **Token only** (`tokenType: "dimension"`), like `gap`. `W307` when the token is not a dimension, `W306` when it is not in the token set.
- **Overlap with T14.** `docs/figma-style-overrides-design.md` proposes `style-padding` on every component. Both cannot exist for `stack` and `grid`, by its own rule "a style attribute that would say what a catalog prop already says is not added". See open question 6.

### `max-width`

- **Meaning.** The element is as wide as its parent lets it be, but never wider than the token. Where it sits inside a wider parent is the parent's cross-axis `align` (a column's `align="center"` centres it). It does not make a stack in a row grow; that is `grow`.
- **Catalog prop** of `stack` and `grid`. A readable column or a centred form is a group, so containers are where it is needed. Making it universal later is a minor change.
- **Token only**, `dimension`. The default token set gets a `size` group (open question 7 proposes `size.sm` 240 px, `size.md` 480 px, `size.lg` 720 px, `size.xl` 960 px). Projects add their own.
- No `min-width`, `height` or `max-height`: no screen needs them, and a height depends on the host's viewport, which Weft does not describe.

### `min-column-width`

- **Meaning.** The grid shows `columns` columns while each can be at least the token wide, and fewer below that, down to one. Children keep their order and fill the rows left to right. Without the prop the grid keeps `columns` at every width, as today.
- **Catalog prop** of `grid`, token only (`dimension`). `columns` stays required: it is the most columns the grid shows, and what every target without reflow draws.
- **Container, not viewport.** The rule depends on the grid's own width, so a grid inside a sidebar reflows like one on a phone. That is why no breakpoints are proposed: they are viewport rules that SwiftUI, Slint, Figma and Penpot cannot express, and they need either token types DTCG does not have or a `weft.json` key (open question 5).

### Validation and diagnostics

One new code, numbered after `W317`, the last semantic code on `main` (other open proposals claim `W120`, `W121`, `W225`–`W229` and `W510`–`W512`):

| Code | Severity | When |
| --- | --- | --- |
| `W318` (new) | error | `grow` on an element whose parent is not a `stack`: the root, a child of a `grid` or of any other kind, or the content of a slot. `<each>` is transparent, so an element repeated inside a stack is fine. An unknown or extension parent is skipped, as parent/child rules skip opaque elements (§8). The hint names the fix: move the element into a stack, or remove `grow`. |

Existing codes that apply unchanged:

| Code | On the new forms |
| --- | --- |
| `W203` | A `justify` value outside the four. |
| `W204` | `grow` that is not a boolean; a literal in `padding`, `max-width` or `min-column-width`. |
| `W217` | A binding in `grow`. |
| `W306`, `W307` | A token that is not in the token set, or not a `dimension`. |
| `W402` | Any of the props on a kind that does not declare it (`padding` on a `section`), in lenient mode a warning and kept. |

`W318` is a semantic check, so a partial parse (§6.3) reports it like any other: the parent of an element is known as soon as the element's start tag is read.

No check is proposed for combinations that are valid but have no effect (`justify` beside a child that grows, `justify` on a column that nothing stretches). They are harmless, and a warning there would be noise in the repair loop.

### Patches

Nothing new: `set` on `justify`, `padding`, `max-width`, `min-column-width` or `grow` changes it, and `null` removes it. A `move` of a growing element out of its stack fails the whole patch list with `W318`, like any patch whose result does not validate.

### Effect on targets: generating

Every target keeps the props it cannot draw: the code generators in their source comment (and SwiftUI also in its inert marker modifier), the design tools in plugin data, so a Weft-generated file or layer reads back exactly. The table says how each form is drawn.

| Target | `justify` | `grow` | `padding` | `max-width` | `min-column-width` |
| --- | --- | --- | --- | --- | --- |
| Static HTML | `data-justify` and a rule in the layout stylesheet: `justify-content: flex-start`, `center`, `flex-end`, `space-between` (as `data-align` works today) | `data-grow` and a rule: `flex: 1 1 0%; min-width: 0` in a row, `min-height: 0` in a column | Inline `padding: var(--weft-space-lg)` | Inline `width: 100%; max-width: var(--weft-size-lg)` with `box-sizing: border-box` | `grid-template-columns: repeat(auto-fill, minmax(max(var(--weft-size-sm), calc((100% - 2 * var(--weft-space-lg)) / 3)), 1fr))` for 3 columns: the second bound caps the count at `columns` without a media or container query |
| React, SolidJS, Lit (one render plan) | Same declarations as inline styles | Same | Same | Same | Same |
| Reference renderer (`render-react`) | `layoutStyle` | Applied in `renderNode` like the tilt, on whatever element the node draws | `layoutStyle` | `layoutStyle` | `layoutStyle` |
| SwiftUI | Row: `end` is a `Spacer(minLength: 0)` before the children, `center` one at each end, `space-between` an `HStack(spacing: 0)` with `Spacer(minLength: <gap>)` between the children. Column: a `layout` loss (see `justify`) | Row child: `.frame(maxWidth: .infinity)`. Column child: a `layout` loss | `.padding(theme.space.lg)` | `.frame(maxWidth: .infinity, alignment: …).frame(maxWidth: theme.size.lg)` | A small generated `Layout` that applies the same rule (iOS 16), or `GridItem(.adaptive(minimum:))`, which does not cap at `columns` (open question 5) |
| Slint | `alignment: start`, `center`, `end`, `space-between` on `HorizontalLayout`/`VerticalLayout` | `horizontal-stretch: 1` in a row, `vertical-stretch: 1` in a column, with `0` written on the siblings | `padding: <px>` | `max-width: <px>` | Loss: `GridLayout` has fixed columns, so it is drawn with `columns` |
| A2UI | `justify` (`spaceBetween` in A2UI's spelling) on `Row`/`Column` | `weight: 1` | Loss | Loss | Loss (the grid is already a loss, §9) |
| json-render | Kept as the prop: the exported catalog is the Weft catalog | Kept | Kept | Kept | Kept |
| Figma | `primaryAxisAlignItems`: `MIN`, `CENTER`, `MAX`, `SPACE_BETWEEN` | The child's `layoutSizingHorizontal` (row) or `layoutSizingVertical` (column) is `FILL` | The four paddings, bound to the token's variable | `maxWidth` bound to the variable, with `FILL` across the parent | Drawn with `columns` `FLEX` tracks: Figma has no automatic repetition |
| Penpot | `justifyContent` | `layoutChild.horizontalSizing` or `verticalSizing` is `fill` | The four paddings in px | `layoutChild.maxWidth` in px | Drawn with `columns` `flex` tracks |

Notes on the table:

- **Web values.** The exact CSS is T16.3's to settle against the `packages/visual` cross-target comparison; the table fixes the intent. The grid formula is a known pattern built only from `repeat`, `auto-fill`, `minmax`, `max` and `calc`, all in CSS Grid Level 1 and CSS Values (`research.md` §20).
- **SwiftUI spacers and `<each>`.** `space-between` over children that come from an `<each>` needs a spacer between repeated items but not after the last one; the generator already enumerates the items, so it can test the index. T16.4 owns this.
- **SwiftUI growth.** Several flexible children in an `HStack` share the space roughly equally; SwiftUI measures the least flexible first, so children with very different content minimums may not end up exactly equal. The visual baselines decide whether that is close enough.
- **Slint's default.** `LayoutAlignment` lists `stretch` first and the layout's `alignment` declares no default (`research.md` §20), while Weft's stacks do not stretch. Whether the Slint generator should write `alignment: start` on every stack today was not checked for this proposal (**unverified**); T16.5 checks it against its screenshots.
- **Design tools without a token.** Figma binds `padding` and `max-width` to variables (`VariableBindableNodeField` lists the paddings and `maxWidth`). Penpot gets values; the prop itself is in plugin data, as `gap` is today.

### Effect on targets: importing

| Importer | Reads back | Loss |
| --- | --- | --- |
| HTML and DOM (`dom.rs`) | `data-justify`, `data-grow`, the inline `padding`, `max-width` and grid template when they name `--weft-` properties (Weft's own output) | In foreign HTML: `justify-content` other than the four (`space-around`, `space-evenly`, `stretch`), `flex-grow` ratios (any positive grow is `grow`, unequal values are a loss), raw lengths, media and container queries. `layout` |
| React and SolidJS source (`from_jsx`) | The same conventions in inline styles | As for HTML |
| Accessibility snapshot | Nothing: `stack` and `grid` are not in the tree | Always, as today |
| SwiftUI | The generator's spacer patterns, `.frame(maxWidth: .infinity)` on a row child, `.padding(theme.<path>)`, the pair of frames, the grid form; and the marker modifier for what was not drawn | `Spacer` elsewhere, `layoutPriority`, fixed `frame` sizes, a padding that is not a theme token: `layout` (the `padding` and `frame` losses of §9 narrow to these) |
| Slint | `alignment` (`space-around`, `space-evenly` and `stretch` are a loss), a `*-stretch` above 0 as `grow`, `padding` and `max-width` in px that equal exactly one dimension token, as `spacing` is read today | Per-side padding, `min-width`, `preferred-width`, unequal stretch factors |
| A2UI | `justify` `start`, `center`, `end`, `spaceBetween`; `weight` above 0 as `grow` | `spaceAround`, `spaceEvenly`, `stretch`; unequal weights. The §9 row "`justify` other than `start`, `weight`" narrows to these |
| json-render (T13.2) | The props as they are | None |
| Figma and Penpot | Weft-built layers from plugin data, as every prop. Foreign layers: the alignments above, `FILL`/`fill` on the parent's main axis as `grow`, four equal paddings that are bound to or equal one dimension token, `maxWidth` likewise | `SPACE_AROUND`, `SPACE_EVENLY`, fixed sizes, `minWidth`, unequal paddings, grid tracks other than equal `FLEX`, absolute positioning: `layout` |

### Corpus and benchmark

`crates/weft-snapshots/tests/coverage.rs` fails while any catalog prop or enum value appears in no corpus screen, so every new prop and every `justify` value needs a screen.

- `wizard-step` is a benchmark screen (`bench/src/corpus.ts`, `SCREENS`), written in four formats with five tasks in `corpus/tasks.json`. Changing it changes the measured inputs, which is a method change recorded in `test.md`, and the HTML, JSX and A2UI versions would change with it. The proposal leaves it as it is; its after-form above is an example only.
- `dashboard` is not a benchmark screen. It takes the changes shown above: `space-between`, `min-column-width`, `max-width`.
- `glass` (not a benchmark screen) takes `padding`.
- A new non-benchmark screen, `corpus/layout`, covers the rest with common patterns: a toolbar where a search field grows beside a button (`grow`, `justify="start"`), a pager row (`justify="center"`), and dialog-style actions (`justify="end"`).

```xml
<screen id="layout" label="Layout" weft="0.1">
  <stack id="toolbar" direction="row" gap="{token.space.sm}" justify="start">
    <field id="search" grow="true" label="Search" type="search" value="{$.query}"/>
    <button id="filter" on-press="list.filter">Filter</button>
  </stack>
  <stack id="pager" direction="row" gap="{token.space.sm}" justify="center">
    <link id="previous" on-press="list.previous">Previous</link>
    <link id="next" on-press="list.next">Next</link>
  </stack>
  <stack id="actions" direction="row" gap="{token.space.sm}" justify="end">
    <button id="cancel" on-press="list.cancel">Cancel</button>
    <button id="save" variant="primary" on-press="list.save">Save</button>
  </stack>
</screen>
```

The benchmark's Weft primer (`bench/src/primers.ts`) does not change. `AGENT-SPEC.md` gains one layout line and `W318`; the MCP primer follows it. Models that do not know the new props write the same screens as today, so measured validity cannot drop by construction.

### Version and migration

- **No syntax or shape change.** Every form is a prop in `props`. A reader that predates them warns `W402` in lenient mode, keeps the attribute (§8) and draws the old layout: the title and time sit together, the grid keeps its columns. That is the graceful fallback a minor version needs.
- **Catalog.** Four new optional props and the enum are minor changes (§8, `diff.rs`: adding a prop is minor unless it is required; a new prop with a default is not "a default appears"). The catalog's `version` has been `0.1.0` since T3, although T50, T51 and T52.1 also added props and kinds. Open question 9 proposes bumping it to `0.2.0` with this change.
- **Universal attribute.** `grow` extends the §2.2 table. T52 added the tilt attributes the same way within `weft` 0.1, with no format version change, so this proposal follows that precedent and ships within 0.1. The alternative is to wait for T39's `weft` 0.2.
- **Tokens.** The `size` group is added to the default token set. It is not a format change; a project that uses its own tokens adds its own sizes.
- **`weft.json`.** No key: nothing here is a tool option.

### Implementation outline (after approval)

| Sub-task | Scope |
| --- | --- |
| T16.2 | `SPEC.md` §2.2 (`grow`), §5.1 (rows and a note per prop), §6.2 (`W318`), §8 (the catalog version), §9 (every target's mapping, with the forms not yet drawn listed as losses); `AGENT-SPEC.md` and the MCP primer; `packages/catalog/src/core.ts`, `catalog.json`, the `stack` and `grid` examples, the `size` tokens, `docs/catalog-and-tokens.md`; `crates/weft-core/src/rules.rs` (`grow` and `W318`) and the differential fixtures; `dashboard`, `glass` and `corpus/layout`. |
| T16.3 | Static HTML and the layout stylesheet, the JSX render plan (React, SolidJS, Lit), `render-react`, `dom.rs` and `from_jsx`, the `packages/visual` web baselines. |
| T16.4 | `crates/weft-swiftui` generator and importer, the grid layout helper if chosen, the SwiftUI baselines. |
| T16.5 | `crates/weft-slint` generator and reader, after T67.4 and T67.5. |
| T16.6 | `crates/weft-interop`: A2UI export and import, json-render export, and the T13.2 import if it has landed. |
| T16.7 | `packages/design-tool` (`layoutView`, `read.ts`, `foreign.ts`), the Figma and Penpot builds, their layer-tree snapshots and round trips. |

## Alternatives considered

| Alternative | Why not |
| --- | --- |
| A `style="…"` or `layout="…"` attribute with a small CSS-like language | A second grammar inside a value; breaks rule 2 and §2.1, as the style-overrides proposal found for styles. |
| Child sizing as a catalog prop on every kind | The same meaning under some thirty names; every catalog change touches them; generic tools cannot treat them alike. |
| `size="hug\|fill\|fixed"` per child | `hug` is what absence already says, so it is a second way to say it; `fixed` needs a width anyway. One boolean says the one thing every target can draw. |
| A numeric `weight` (A2UI, Compose, Slint, CSS) | Figma, Penpot and SwiftUI have no ratios; three targets would lose it. |
| A `spacer` element (SwiftUI's `Spacer`, Figma has none) | It adds an element with no role and no meaning to the tree, an id to keep, and two ways to push children apart (a spacer or `justify`). |
| Breakpoints as tokens or a `weft.json` key | Viewport rules that only the web can apply, need token types DTCG lacks, and make one screen mean different things per project. |
| Container queries or `ViewThatFits`-style alternatives (a list of layouts, the first that fits) | Strong, but a whole screen described twice; no design tool or Slint form. Can be revisited when a screen needs a row that turns into a column. |
| Literal lengths (`padding="16"`) | Breaks rule 6; brings units into the value grammar; Figma could not bind them to variables. |
| `margin` | Spacing between siblings is the parent's `gap`; a margin would be a second way to say it. |
| Doing nothing (status quo) | Every importer keeps losing layout, and agents keep writing gap-based approximations that look right on one target only. |

## Open questions for the creator

1. **Scope: which forms are in?** Recommendation: all five (`justify`, `grow`, `padding`, `max-width`, `min-column-width`); each answers a corpus workaround. Alternatives: only `justify` and `grow` now (the two that every target reads back, including A2UI), the rest in a later minor version; or drop `min-column-width` until a screen besides `dashboard` needs it.
2. **`justify` values and name.** Recommendation: `start`, `center`, `end`, `space-between`, named `justify` as in A2UI, CSS and Penpot. Alternatives: all six of A2UI and Penpot (`space-around` would be a SwiftUI loss); another name (`distribute`).
3. **Child sizing.** Recommendation: a universal boolean `grow`, literal only, valid only in a stack (`W318`). Alternatives: a numeric weight (lost on Figma, Penpot and SwiftUI); a `size` enum with `hug`/`fill`/`fixed` (and a `width` token for `fixed`); per-kind catalog props.
4. **`justify` and `grow` on columns.** Recommendation: allowed in both directions with one meaning; a column has free space only when a stretched row gives it some, and SwiftUI reports both on a column as a `layout` loss, as it already cannot draw `stretch`. Alternative: rows only, with a new diagnostic for a column.
5. **Responsive behaviour.** Recommendation: `min-column-width` on `grid`, capped by `columns`, and no breakpoints. For SwiftUI, a small generated `Layout` that applies the same rule (iOS 16; the generator targets iOS 17) rather than `GridItem(.adaptive)`, which shows more than `columns` on wide screens. Alternatives: `adaptive` with its difference listed in §9; breakpoints as tokens or in `weft.json`; no reflow at all.
6. **Padding: here or in style overrides?** Recommendation: `padding` here, on `stack` and `grid` only, because it is layout like `gap`; the style-overrides proposal then drops `style-padding` (or keeps it only for kinds that are not `stack` or `grid`, if the creator wants padding on sections and buttons). Alternative: no `padding` here and `style-padding` on every component through T14.
7. **Sizes: tokens only?** Recommendation: `dimension` tokens only (rule 6), and a `size` group in the default tokens: `size.sm` 240 px, `size.md` 480 px, `size.lg` 720 px, `size.xl` 960 px. Alternatives: other names or values; literal numbers in px (breaks rule 6).
8. **`max-width`: fill up to the cap, on containers only?** Recommendation: the element fills what its parent offers up to the token, and only `stack` and `grid` take it. Alternatives: hug up to the cap (a narrow form for a short form's fields, which is rarely meant); a universal attribute.
9. **Version.** Recommendation: ship within `weft` 0.1, as T52's tilt did, and bump the catalog to `weft-core` 0.2.0 with it (its version has stayed 0.1.0 through three minor additions). Alternatives: bundle with T39's `weft` 0.2 so readers move once; or leave the catalog version as it is.
10. **Corpus.** Recommendation: leave the benchmark screen `wizard-step` unchanged, change `dashboard` and `glass`, and add a non-benchmark `corpus/layout` screen for the remaining values. Alternative: change `wizard-step` in all four formats as a method change recorded in `test.md`.
11. **`W318` severity.** Recommendation: an error in both modes, because a `grow` that no target can draw is a mistake, not a newer feature. Alternative: a `mode` code (a warning for readers).
12. **`justify` default in the catalog.** Recommendation: declare `default: "start"`, so tools and the T12 schema know it. Alternative: no declared default, with `start` stated in SPEC only, as `align` has none for columns.
