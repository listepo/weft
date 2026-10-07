# weft-slint

Weft to Slint and back (SPEC §9, "To Slint" and "From Slint").

- `generate` turns a strictly valid screen into one `.slint` file for Slint 1.x and its `std-widgets.slint`.
- `import_slint` reads a file that `generate` printed back into the same Weft document.

```rust
let slint = weft_slint::generate(&document, &GenerateOptions { catalog: &catalog, tokens: &tokens, name: None })?;
let document = weft_slint::import_slint(&slint, &ImportOptions { catalog: &catalog, tokens: &tokens })?;
```

The crate is pure Rust over `weft-core` and `weft-catalog`. The Slint compiler (`slint-interpreter`) is a dev-dependency: the tests compile every generated file with it.

## The generated file

| Part | Slint |
| --- | --- |
| Source comment | `// weft:source slint` and the canonical markup, one line per comment line |
| Imports | Only the std widgets the screen uses |
| Component | `export component <Name>Screen inherits Window`, `title` from the screen's `label` |
| Data | One `in-out property` per root data path a binding reads: `$.email` → `email`, `$.user.email` → `user-email` |
| Actions | `callback perform(string, string)`: the action name and the id of the element |
| Body | A `VerticalBox` named after the screen id, holding the elements |

A host sets and reads the data through the properties (`set_email`, `get_email` in Rust) and handles every event in `on_perform`.

## Mapping

| Weft | Slint |
| --- | --- |
| `screen` | `Window` component with a `VerticalBox` (`alignment: start`) |
| `stack` | `VerticalLayout` or `HorizontalLayout` (`direction`); a `gap` token → `spacing` in px (`rem` × 16) |
| `grid` | `GridLayout`; child `n` gets `row: n / columns` and `col: n % columns` |
| `section` | `GroupBox` (`title` from `label`) around a `VerticalBox`, the `header` slot first |
| `form` | `VerticalBox`; the `footer` slot in a `HorizontalBox` after the content |
| `heading` | `Text` with the browser's default size for the `level` and weight 700 |
| `text` | `Text` |
| `link` | `TouchArea` around a `Text` in `Palette.accent-background` |
| `button` | `Button`; `primary: true` for `variant="primary"` |
| `field` | a caption `Text` above a `LineEdit` (`input-type` `password` or `number`, `placeholder-text`); `TextEdit` for `multiline` |
| `checkbox`, `switch` | `CheckBox`, `Switch`; `text` from `label` |
| `select`, `option` | caption `Text` above a `ComboBox`: the option texts are the `model` (a bound option text is an expression), `value` picks `current-index`, and a selection writes the option's `value` back |
| `slider` | caption `Text` above a `Slider` (`float`): `minimum`, `maximum`, `value` |
| `stepper` | caption `Text` above a `SpinBox` (`int`) |
| `each` | `for <as> in root.<model>`. The model is `[{ field: type }]`, or `[string]` for the options of a `select` or `combobox`. `$.players.0.name` reads `root.players[0].name` |
| `list`, `item` | `ListView` when the list's only element is one `<each>`; otherwise `VerticalLayout`. An item is `Text`, `TouchArea` (`on-press`) or `VerticalLayout` |
| `table`, `column`, `row`, `cell` | `StandardTableView`. Columns carry `title` and `sort-order`. Literal rows are `rows`; a repeated row's cells stay in the comment |
| `tabs`, `tab` | `TabWidget` and `Tab`. `selected` is `current-index` |
| `dialog` | `PopupWindow`, shown and closed from `open` |
| `menu`, `menu-item` | `MenuBar`, `Menu`, `MenuItem` |
| `image`, `model` | `Image` with `accessible-label`. Paths are not `@image-url` |
| `alert` | `VerticalLayout` of its text and children |
| `date-picker`, `color-picker` | caption `Text` above a `LineEdit` holding the string `value` |
| `radio-group`, `radio`, `segmented-control`, `segment` | `RadioGroup` and `RadioButton` (`orientation: horizontal` for a segmented control) |
| `combobox` | `ComboBox`, as a `select`. Options from `<each>` are a `[string]` model |

| Weft | Slint |
| --- | --- |
| id | element id (`email := LineEdit`) |
| writable prop bound to a path | two-way binding, `text <=> root.email` |
| other binding | expression; a boolean read of text is `root.x != ""`, of a number `root.x != 0` |
| `disabled` | `enabled`, the binding read negated |
| `hidden` | `visible`, the binding read negated |
| `label` on `button` and `heading` | `accessible-label` |
| `on-press` | `clicked => { root.perform("action", "id"); }` |
| `on-change` | `edited` (fields, `SpinBox`), `toggled` (`CheckBox`, `Switch`), `selected` (`ComboBox`), `changed` (`Slider`) |
| `on-submit` | runs from a `submit="true"` button's `clicked` and from Enter in a `LineEdit` of the form (`accepted`) |

## What does not map

- **Kept only in the source comment:** `state`, `required`, `error`, the field types `email` and `search`, `variant="danger"`, `tone`, `stack.align` and `wrap`, `step`, `href`, `material`, the 3D tilt. Tokens become their px values.
- **Refused** (`GenerateError::Unsupported` with the element or binding path): a kind outside the table above; an array index that is not into a repeated model; ids or property names Slint reads as one (`a-b` and `a_b`), the reserved ids (`root`, `self`, `parent`, `true`, `false`, and `Palette`, `InputType`, `LayoutAlignment`, which the generated code names), and a data path whose `-data` form is still a `Window` property name; a non-token `gap`, a bound heading `level`, a bound `variant`, and inputs of different types writing one path.
- **The importer** reads only files the generator printed: an edited file, one generated with other tokens, or hand-written Slint is refused (`ImportError::Edited`, `NoSource`). Reading such files with a loss table is T67.3.
