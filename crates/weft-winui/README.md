# weft-winui

Weft to WinUI 3 (Windows App SDK) and back (SPEC §9, "To WinUI" and "From WinUI").

- `generate` turns a strictly valid screen into a XAML `Page` bound with `{x:Bind}` to a generated C# view model.
- `helpers` and `generate_tokens` write `Weft.cs` and `WeftTokens.xaml`, which every page of a project shares.
- `import_winui` reads a generated page back from its source comment.
- `read_winui` (feature `import`) reads any other XAML element by element and returns `{ document, losses, diagnostics }`.

What `generate` writes reads back to the same document both ways: through the comment, and from the XAML alone with no losses. The C# is derived from the XAML and is never read.

## Use

```console
$ weft winui screens/login.weft                        # prints the page's three files
$ weft winui screens/login.weft --out-dir windows      # writes windows/LoginPage.xaml, …
$ weft winui screens/login.weft --namespace Acme.Screens --name SignIn
$ weft winui-tokens --out-dir windows                  # writes WeftTokens.xaml and Weft.cs
$ weft import-winui windows/LoginPage.xaml > login.weft
```

- **Catalog and tokens.** The commands take the catalog from `--catalog`, then the project (`weft.json`), then the core catalog. Tokens come from `--tokens` (a token file or a resolver), then the project, then `packages/catalog/tokens/default.tokens.json`.
- **Names.** The page class is `<namespace>.<Name>Page`: `Name` from `--name`, else the screen id in PascalCase (`login-form` → `LoginForm`); the namespace from `--namespace`, then `export.winui.namespace`, then `WeftScreens`.
- **Output directory.** It comes from `--out-dir`, then the project's `export.winui.outDir` or `import.winui.outDir`, then standard output, where each file is headed by `==> <file> <==`.
- **Context.** The source comment keeps the screen's `<context>` unless `--context strip` or `export.winui.context` says otherwise; `weft import-winui --context drop` or `import.winui.context` keeps it out of the document read back.
- **Exit codes.** A screen that is not strictly valid exits 1, and so does one the generator cannot express yet: each such element is reported with its path. `weft import-winui` accepts only a generated page whose comment regenerates it; anything else exits 1.

As a library:

```rust
let page = weft_winui::generate(&document, &GenerateOptions { catalog: &catalog, tokens: &tokens, name: None, namespace: "WeftScreens", shared_tokens: true, appearance: None })?;
let weft_cs = weft_winui::helpers("WeftScreens")?;
let (weft_tokens, skipped) = weft_winui::generate_tokens(&tokens);
let document = weft_winui::import_winui(&page.xaml, &ImportOptions { catalog: &catalog, tokens: &tokens })?;
let result = weft_winui::read_winui(&xaml, &ImportOptions { catalog: &catalog, tokens: &tokens });
```

The crate is pure Rust (XML through `roxmltree`) and builds for WebAssembly. The `import` feature only adds the reader's tables.

## The generated files

| File | What it holds |
| --- | --- |
| `<Name>Page.xaml` | The source comment (`<!-- weft:source winui name=… namespace=…` and the escaped markup), then a `Page` holding a `ScrollViewer` and the screen's `StackPanel`. Ids are `AutomationProperties.AutomationId`. |
| `<Name>Page.xaml.cs` | The page class: `ViewModel` and the constructor. |
| `<Name>ViewModel.cs` | Plain C# with `INotifyPropertyChanged`: one property per data path, a class per object and list item, an `ICommand` per action, and `Perform`, which raises `Performed` with a `WeftEvent(Action, Id, Item)`. |
| `Weft.cs` | Written once per namespace: the binding and converter functions, the inert `Props` markers, the `On` events, `Choice.Value`, `Dialog.IsOpen`, the `Each` and `Slot` markers, `Tilt.Centered`, `ColumnsPanel` and `ColumnsLayout`. No NuGet package beyond the Windows App SDK. |
| `WeftTokens.xaml` | A `ResourceDictionary` merged into `App.xaml`: the dimension tokens (`x:Double`, and `Thickness` for padding), and the colours and materials (`SolidColorBrush`, `AcrylicBrush`) in `Light`, `Dark` and `HighContrast` theme dictionaries. With `shared_tokens: false` a page holds the tokens it uses in its own `Page.Resources` instead. |

## Mapping

SPEC §9 "To WinUI" is the full mapping and `docs/winui-design.md` the design behind it. In short:

| Weft | WinUI |
| --- | --- |
| `screen`, `form`, `section` | `StackPanel` with a landmark; a section is drawn as a card |
| `stack` | `StackPanel`, or a `Grid` of `Auto` and star tracks when `justify` leaves free space or a child has `grow` |
| `grid` | `Grid` of star columns, or `weft:ColumnsPanel` with `min-column-width`; an `ItemsRepeater` with a `weft:ColumnsLayout` when its only child is an `<each>` |
| `text`, `heading` | `TextBlock`, a heading with its type-ramp style and `HeadingLevel` |
| `button`, `link` | `Button`, `HyperlinkButton` |
| `field`, `checkbox`, `switch` | `TextBox` or `PasswordBox`, `CheckBox`, `ToggleSwitch` |
| `select`, `combobox`, `radio-group`, `segmented-control` | `ComboBox`, `RadioButtons`, `SelectorBar`, with `weft:Choice.Value` |
| `slider`, `stepper`, `date-picker`, `color-picker` | `Slider`, `NumberBox`, `CalendarDatePicker`/`TimePicker`, a `ColorPicker` flyout |
| `list`, `table`, `tabs`, `dialog`, `menu`, `alert` | `ListView`, panels marked as a table, `TabView`, `ContentDialog`, `DropDownButton`, `InfoBar` |
| `<each>` | `ItemsRepeater` with a typed `DataTemplate`, or a `ListView`'s `ItemsSource` |
| `material` | A `Border` with the token's `AcrylicBrush` around the stack or grid |
| `rotate-x`, `rotate-y`, `rotate-z`, `perspective` | A `CompositeTransform3D` centred by `weft:Tilt.Centered`, under a `Border` with a `PerspectiveTransform3D` |

Data paths are view-model properties, actions are commands, and what WinUI does not draw is an inert `weft:Props.<Prop>` marker the reader gives back.

## Hand-written XAML

`read_winui` parses XAML as XML and never compiles or runs it: a DTD is refused and the size, node count and depth are bounded. Each control becomes the kind the mapping writes for it, `{x:Bind}` paths become bindings, `weft:On.*` and command names become actions, `Weft.…` resources become tokens, a material `Border` and 3D transforms become `material` and the tilt, and `Grid` tracks become `justify` and `grow`. What has no Weft form is a loss with its XAML path (SPEC §9, "From WinUI"), never a silent drop.

## Tests

`cargo test -p weft-winui --all-features` compares every corpus screen and catalog example that generates with its golden files in `tests/fixtures/`, and reads each back both ways. The `PENDING` list in `tests/common/mod.rs` names the screens that do not generate yet. The `winui` CI job builds the goldens with `windows/WeftScreens.csproj` on Windows, so the XAML compiler checks every binding, event and attached property against the generated C#. Regenerate the goldens with `WEFT_UPDATE_FIXTURES=1`.
