# weft-swiftui

Weft to SwiftUI and back (SPEC §9, "To SwiftUI" and "From SwiftUI").

- `generate` turns a strictly valid screen into one Swift file for iOS 17 and macOS 14.
- `import_swiftui` reads Swift source back into a Weft document and returns `{ document, losses, diagnostics }`.

What `generate` prints reads back to the same document with no losses. Other SwiftUI imports view by view, and each part Weft cannot hold becomes a loss.

## Use

```console
$ weft swiftui screens/login.weft > Login.swift
$ weft swiftui screens/login.weft --out-dir ios        # writes ios/login.swift
$ weft import-swiftui Login.swift > login.weft          # losses go to stderr
```

- **Catalog and tokens.** Both commands take the catalog from `--catalog`, then the project (`weft.json`), then the core catalog. `weft swiftui` takes tokens from `--tokens`, then the project, then `packages/catalog/tokens/default.tokens.json`.
- **Output directory.** It comes from `--out-dir`, then the project's `export.swiftui.outDir` or `import.swiftui.outDir`, then standard output.
- **Exit codes.** A screen that is not strictly valid exits 1, and so does one the generator cannot express (`x-` content, a kind outside the core catalog, a token that is not a px or rem dimension). For the importer, losses are not failures: it exits 1 only on an error diagnostic.

As a library:

```rust
let swift = weft_swiftui::generate(&document, &GenerateOptions { catalog: &catalog, tokens: &tokens, name: None })?;
let result = weft_swiftui::import_swiftui(&swift, &ImportOptions { catalog: &catalog });
```

The default `import` feature brings in tree-sitter and its Swift grammar. Both are C, and they do not build for `wasm32-unknown-unknown`. For a WebAssembly build, use `default-features = false`: that leaves the generator alone, which is pure Rust.

## The generated file

A file for a screen whose id is `login` contains these parts, in order:

| Part | Swift |
| --- | --- |
| Data model | `@Observable final class LoginModel`: one property per root data path the bindings read. Each type is inferred from the props that read it: `String`, `Bool`, `Int`, `Double`, a nested struct, or an array of one. |
| Actions | `enum LoginAction: String, CaseIterable`: one case per action, with the Weft action name as the raw value. |
| Events | `struct LoginEvent { action, id, item }`: what the handler receives. Inside `<each>`, `item` is the data path of the list item (`$.todos.2`). |
| Theme | `struct LoginTheme`: the tokens the screen references, as nested `CGFloat` values (px, or rem × 16). |
| View | `struct LoginScreen: View` with `@Bindable var model`, `var theme`, and `var perform: (LoginEvent) -> Void`. |
| Preview | `#Preview` with an empty model. |
| Helpers | `fileprivate` helpers named `weft…`. They carry what SwiftUI has no form for, so the importer can read it back. |

## Mapping

| Weft | SwiftUI |
| --- | --- |
| `screen` | `VStack(alignment: .leading)`, the body of the view |
| `stack` | `VStack` or `HStack` (`direction`); `align` gives `alignment:`, a `gap` token gives `spacing:` |
| `grid` | `LazyVGrid` with `columns` flexible `GridItem`s |
| `section` | `Section`, with the `header` slot as its header |
| `heading` | `Text` with a font for the `level` and `.accessibilityHeading(.hN)` |
| `text` | `Text`; `tone` gives `.foregroundStyle` |
| `image` | `AsyncImage(url:)`; only `http`, `https`, `mailto` and relative URLs load |
| `link` | `Button` that calls `openLink` (the same URL rule) |
| `button` | `Button` that sends the action; `variant` gives `.buttonStyle`, and `danger` gives `role: .destructive` |
| `form` | `Form`; `on-submit` gives `.onSubmit`, and the `footer` slot gives a section footer |
| `field` | `TextField` or `SecureField` (`type="password"`) bound to the model; `type` sets the keyboard on iOS |
| `checkbox` | `Toggle` with a checkbox style |
| `switch` | `Toggle` with `.toggleStyle(.switch)` |
| `radio-group` | `Picker` with `.pickerStyle(.inline)` |
| `select` | `Picker` |
| `radio`, `option` | `Text(…).tag(value)` inside the picker |
| `list` | `List`; the `empty` slot goes in an `.overlay` shown when the list has nothing to show |
| `item` | `HStack` |
| `table` | `Grid(alignment: .leading)`, with the columns as its first `GridRow` |
| `column` | `Text` in `.headline` |
| `row` | `GridRow` |
| `cell` | `HStack` |
| `tabs` | `TabView(selection:)`, bound when `selected` is a binding |
| `tab` | A page with `.tabItem { Text(label) }` and `.tag(id)` |
| `dialog` | `.sheet(isPresented:)` on an empty anchor; `on-dismiss` gives `onDismiss:`, and `modal="false"` gives `.presentationBackgroundInteraction` |
| `alert` | `GroupBox` tinted by `tone` |
| `menu` | `Menu` with the label as its title |
| `menu-item` | `Button` inside the menu |
| `<each>` | `ForEach(Array(items.enumerated()), id: \.offset)` |
| `id` | `.accessibilityIdentifier(id)` |
| `label` | The control's title, else `.accessibilityLabel` (not on `stack`, `grid` or `text`, which have no name) |
| `hidden` | `.weftHidden(condition)`, which removes the view while the condition holds |
| `disabled` | `.disabled` |
| `on-*` | `send(.case, id)`, which calls `perform` with a `LoginEvent` |
| Binding `{$.a.b}` | `model.a.b`; a writable prop gets `$model.a.b` |
| Negated binding `{!$.a}` | `!` applied to the value read as a flag (SPEC §2.1: an empty array is still true) |
| Token `{token.space.md}` | `theme.space.md` |

A prop SwiftUI has no form for keeps its value as `.weftProp("name", value)`, which does nothing at run time. This covers `required`, `sort`, `state`, `error`, `ordered`, `wrap` and similar props, and values a generated form would not keep exactly (an explicit default, for example), so the importer can read them back.

## Import

The importer reads the first view that no other view in the file uses, and inlines the views it uses from the same file. Source the generator printed comes back unchanged. Other source imports with these rules:

- **Containers.** `VStack`, `HStack` and `LazyHStack` become `stack`. `LazyVGrid` and `LazyHGrid` become `grid`. `List` becomes `list`, `Form` becomes `form`, and `TabView` becomes `tabs`.
- **Controls.** `TextField`, `SecureField` and `TextEditor` become `field`. `Toggle` becomes `switch`, or `checkbox` with a checkbox style. `Picker` becomes `select`, or `radio-group` when its style is inline, radio-group or segmented.
- **Overlays.** `.sheet` and `.alert` become a `dialog`.
- **Repetition and conditions.** `ForEach` over a data path becomes `<each>`. An `if` on a data path becomes `hidden`.
- **Actions.** `send(.case)` and `perform(.case)` keep the action, and so does a call such as `onSave()`.
- **Data paths.** The view's first object property, such as the `@Bindable` model, is `$`. Later object properties are named parts of `$`. A view property that holds a value is its own root: one with a literal initial value, a scalar or array type, or `@Binding`, `@AppStorage` or `@SceneStorage`. A property whose name ends in `theme` holds tokens.

Losses use the SPEC §9 kinds. `tests/fixtures/import/Settings.swift` is a hand-written screen, and its document and every loss are pinned next to it.

| Loss | When |
| --- | --- |
| `ids` | A view has no valid, unique `.accessibilityIdentifier`, so an id is generated from the kind and the text. |
| `bindings` | An expression is not a data path, or text interpolates a value. |
| `actions` | A handler runs code instead of naming an action, or names one on a kind without that event. |
| `tokens` | Spacing is not a theme token. |
| `layout` | A modifier has no Weft prop (`padding`, `frame`, `font`, `tint`, `navigationTitle`, …), or a navigation container is unwrapped. |
| `repetition` | A `ForEach` runs over something other than a data path. |
| `hidden` | An `if` has a condition that is not a data path. |
| `slots` | A section footer has no slot. |
| `props` | A prop value is invalid for the catalog. |
| `values`, `names` | A required prop or label gets a stand-in. |
| `kinds` | A view has no Weft kind (a shape, a custom view from another file). |
| `text` | Text has no place in the content model, or reads like a reference. |
| `structure` | The body is not one stack, content cannot be placed, or the source has syntax errors. |

The source is untrusted: it is parsed, never compiled or run. It is limited to `MAX_SOURCE_LENGTH` bytes, `MAX_NODES` elements and `MAX_DEPTH` levels of nesting; beyond them the importer stops and reports `W602`. A source with no view gives `W601`. The returned document always validates in lenient mode.

## Tests

- **`swift`.** It checks that generation is deterministic. It also typechecks every corpus screen, catalog example and fixture with `xcrun swiftc -typecheck -swift-version 6`, for iOS 17 and for macOS 14, together with the hand-written sample. Without Xcode this test is skipped and says so.
- **`roundtrip`.** For every screen, Weft → SwiftUI → Weft gives byte-identical output with no losses.
- **`import`.** It pins the hand-written sample, and a property test checks that any source imports, without panicking, to a document that is valid in lenient mode.
