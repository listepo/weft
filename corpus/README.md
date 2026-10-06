# Weft corpus

Twelve reference screens, each written in four formats that describe the same content, structure, states, bindings and actions. `bench/` measures them (token cost) and runs model edit and read tasks against them.

```
corpus/<screen>/
  screen.weft        Weft 0.1 markup, written to SPEC.md, in canonical form
  screen.html        semantic HTML with ARIA and data-bind / data-action attributes
  screen.jsx         an idiomatic React function component: ({ data, actions }) => ...
  screen.a2ui.json   A2UI v0.9 messages (basic catalog)
  data.json          the data model every binding resolves against
corpus/tasks.json    edit tasks and comprehension questions with machine-checkable expectations
```

Screens: login, signup, settings, data-table, tabs, confirm-dialog, wizard-step, search-results, todo-list, profile, menu, error-state.

## Coverage screens

Ten more screens exist only as `screen.weft` and `data.json`: dashboard, account, orders, inbox, leaderboard, booking, appearance, glass, tilt, showroom. booking uses the number, date, colour, segmented and combobox controls with bound values, and appearance uses them with absent props, literal values, fractional steps and disabled states, glass takes the `material` token on a stack and a grid (a frosted-glass surface; the screenshot suites show it over a striped backdrop, so the blur is visible), and tilt turns a card by `rotate-y`, `rotate-x` and `rotate-z` with a `perspective`, and holds a bound field inside a tilted section. Together with the twelve above they use every catalog kind, prop, enum value, state, event and slot, every binding form (literal, `{{` escaped literal, `{$.path}`, `{!$.path}`, a loop path, an array-index segment, `<each>`, bound and literal `hidden`) and every token type, which a Rust test checks. The generators, importers, snapshots and screenshots run over all twenty-one; `bench/` does not use them, so they have no HTML, JSX or A2UI version. leaderboard holds the array-index binding, which the SwiftUI generator refuses, so the SwiftUI tests skip it and check the refusal instead.

The showroom screen is the one with a `model` (SPEC §5.1) and the only corpus screen with binary files, in `showroom/assets/`: `gem.glb`, `gem.usdz` and `gem.png`, an octahedron that `showroom/assets/generate.ts` writes. The shape and the generator are written for this repository, so the assets carry no third-party licence and are Apache-2.0 like the rest of it. `bench/test/model-assets.test.ts` keeps them under 4 KiB (the still 8 KiB) and checks that the glb and the usdz are what the generator writes. The screenshot suites show the still, not a live viewer, because a live `model-viewer` or `RealityView` has no fixed first frame in CI.

## How the files were produced

`screen.weft` was written by hand. The first versions of the other three formats were derived from it by a throwaway transpiler that was never committed. Since then every file is edited by hand, and a change to a screen is made in all four formats in the same commit (the T9 revision, for example, rewrote the search results empty state in all four).

Agreement is enforced by tests rather than by generation:

- `bench/test/conformance.test.ts` parses every `screen.weft` with the reference parser and validates it in strict mode against the core catalog and the default tokens: zero diagnostics, and the file must equal its canonical serialization (`node packages/core/src/cli.ts fmt --write <file>`).
- `bench/test/corpus.test.ts` parses every format into the neutral tree of `bench/src/neutral.ts` and asserts that HTML and JSX yield exactly the Weft tree and that A2UI carries the same stream of names, bindings and actions.

Each format follows its own idiom; none is minified or padded. Ids appear in HTML and JSX only where an idiom needs them (`label for`, tabs).

## HTML conventions

- `data-bind="prop:$.path; prop2:!$.path"` binds an element property to the data model. Properties: `value`, `checked`, `open`, `src`, `href`, `disabled`, `hidden`, `text` (element content), `label` (the accessible name, written as `aria-label`). `!` negates.
- `data-action="event:action.name"` names the host action an event fires. Events: `press`, `submit`, `change`, `close`.
- Repetition: `<template data-each="$.items" data-as="item">`; inside it paths are `$item.field`.
- Empty state: `<template data-empty>` inside a list or table holds the content shown instead of the items when there are none (Weft's `empty` slot).
- `data-variant` carries button variants (`primary`, `danger`), `data-state` the Weft `state`, `data-tone` the alert tone. `class` names (`stack`, `row`, `gap-sm`, `gap-md`) carry layout and spacing intent only.
- A `type="submit"` button inside a form fires the form's `submit` action.

## JSX conventions

Values are read from `data.path`; controlled inputs write with `actions.set("path", value)` (inside a repetition `actions.set(\`todos.${index}.done\`, ...)`); named host actions are called as `actions.group.name()`; repetition is `data.items.map((item, index) => ...)`; visibility bindings are `cond && <El/>`; an empty state is `data.items.length > 0 ? <ul>…</ul> : <p>…</p>`. Corpus `.jsx` files are data and are never compiled.

## A2UI

Version and catalog: the specification page https://a2ui.org/specification/v0.9-a2ui/ (messages carry `"version": "v0.9"`) and its basic catalog `https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json`, read from the `a2ui-project/a2ui` repository (`specification/v0_9/catalogs/basic/catalog.json` on main) on 2026-10-03. A v0.9.1 revision of the spec exists; it is not used. Each file holds `createSurface` and `updateComponents`; the data model lives in `data.json` and would travel in `updateDataModel` (left out for every format alike). Where the spec prose shows `checks` as flat `{call, args, message}`, the catalog schema defines `{condition, message}`; the catalog form is used. These renditions are hand-made; `weft a2ui` writes the same format from any screen and `weft import-a2ui` reads them back, with the loss tables of SPEC section 9 (the tests import every rendition here and export every screen against the A2UI v0.9 JSON Schemas, commit `4787774` of `a2ui-project/a2ui`, 2026-10-05).

Where A2UI's basic catalog cannot express something native to the screen, the closest construct is used and the loss is accepted:

| Screen need | A2UI stand-in or loss |
| --- | --- |
| form and its `submit` event, `submit` button | `Column`; the form's submit action sits on the Button |
| list `empty` slot | a `Text` after the `List`, always shown: A2UI has no conditional rendering |
| switch | `CheckBox` |
| select, radio group | `ChoicePicker` (`mutuallyExclusive`; `filterable` for select); its value must be a string array, so a scalar path like `/plan` is bound as is |
| table, columns, sort state | header `Row` of borderless Buttons or Text plus a templated `List` of `Row`s; sort state is lost |
| alert, tone | `Card`; tone is lost |
| menu, menu item | `List` of borderless `Button`s |
| link | borderless `Button` (`openUrl` function for an href) |
| section | `Card` with `accessibility.label` |
| dialog | `Modal`: the trigger renders in place, there is no `open` binding and no `close` event |
| button variant `danger`, state `busy` | lost; disabled maps to a failing `checks` condition |
| `hidden`, `state`, tokens (`gap`), field `type` email/search, `placeholder`, `on-change` | no equivalent, dropped |
| screen `label` | no equivalent, dropped |
| heading level 6, dynamic options | not used by the corpus |

## Expectation vocabulary (`tasks.json`)

An edit task has `expect` (a list of assertions) and optionally `remove` (nodes the edit may delete or rename). A question has `answer` (accepted final answers; the model replies `ANSWER: <answer>`).

An assertion is either `{ "absent": NodeSpec }` or `{ "has": NodeSpec, ...relations }`. Relations apply to the same matched node: `after`, `before` (document order against the first node matching the reference), `inside`, `not_inside` (ancestor matches the reference), `count` (exact number of matches instead of "at least one"). A reference is a NodeSpec or a plain string meaning `{ "name": ... }`.

A NodeSpec lists the fields a node must have; unlisted fields are ignored:

| Field | Meaning |
| --- | --- |
| `kind` | Weft component kind: `heading`, `text`, `button`, `link`, `field`, `checkbox`, `switch`, `radio-group`, `radio`, `select`, `option`, `list`, `table`, `column`, `tabs`, `tab`, `dialog`, `alert`, `menu`, `menu-item`, `section`, `image`, `form` |
| `name` | Accessible name or visible text (case-insensitive, whitespace-collapsed) |
| `nameBind` | The path supplying the name or text, e.g. `$.billing.plan`; `$*.field` is a field of the current repetition item |
| `bind` | The node's data binding: field value, checkbox state, image source, link target |
| `disabled`, `hidden` | `true`, or a path; a leading `!` negates it |
| `variant`, `type`, `required`, `level`, `value` | Button variant, field type, required flag, heading level, radio/option value |
| `on` | `{ "press": "action.name" }`: events that must fire these actions |
| `each` | The node is inside a repetition of this array path |

Checkers parse each format into one neutral tree (`bench/src/neutral.ts`) and evaluate the assertions on it. For A2UI, a `kind` also matches the closest basic-catalog construct: switch matches CheckBox, select and radio-group match ChoicePicker, radio matches a picker option, menu matches List, menu-item matches a Button, and alert and section match Card. After an edit the output must also parse and be valid for its format, and every name or bound text of the original screen must survive unless the task lists it in `remove`.
