# Design: Weft to WinUI 3 and back

Status: **approved.** The creator approved this design and accepted the recommended answer to every open question (the answers are recorded under **Open questions**). External facts are in `research.md` §23, with sources; facts that could not be checked against a primary source are marked **unverified** there and here, and the sub-task that first depends on one checks it in the Windows build (T105.7) before relying on it.

## Goal

Convert a Weft screen into a WinUI 3 page that builds in a Windows App SDK app, and read WinUI XAML back into Weft, the way T67 does for Slint and T34 for SwiftUI.

- **Weft → WinUI.** A screen becomes a XAML `Page` whose elements bind with `{x:Bind}` to a generated C# view model. Data the screen reads becomes view-model properties; each action an `on-<event>` names becomes a command on the view model.
- **WinUI → Weft.** The importer reads the `.xaml` file only. A file the generator wrote reads back exactly through its source comment (as T67). A hand-written or edited file is read element by element, with the losses SPEC §9 lists.
- **Checked** locally by golden files, XML checks and round trips, and in CI by a Windows job that `dotnet build`s a WinUI project holding every generated corpus screen (creator decision 3).

Not in T105: screenshots (a later, separate task), sample data in the view model, the design tools, the plugins and MCP (no native target has them), and the Slint-in-WinUI hosting of T68.

## Weft side

A new crate `crates/weft-winui`, shaped like `crates/weft-slint`:

| Item | Form |
| --- | --- |
| Generator | `generate(document, &GenerateOptions { catalog, tokens, name, namespace, shared_tokens, appearance }) -> Result<Generated, GenerateError>`, pure Rust, no Windows dependency. `Generated` holds three files (below). `GenerateError::Unsupported` lists every path it refuses, as T67 does. |
| Shared files | `generate_tokens(tokens, namespace)` writes `WeftTokens.xaml`; `helpers(namespace)` writes `Weft.cs`. A project writes each once. |
| Source-comment import | `import_winui(source) -> Result<Document, ImportError>`: reads the `weft:source` comment and accepts it only if generating from it gives the same XAML (below). Always built, no extra dependency beyond the XML reader. |
| Hand-written import | `read_winui(source, &ImportOptions { catalog, tokens }) -> ImportResult { document, losses, diagnostics }` behind the `import` feature, like `weft-slint`'s, using `weft-import` for the shared loss types and the lenient repair. |
| Input bounds | Both importers refuse input over 2 MB, more XML nodes than the bound and XML nested past 800 levels (counted before parsing, since roxmltree recurses per element); `read_winui` reports these as `W602` and cuts the elements past `MAX_NODES` or `MAX_DEPTH`, as the other importers do. |
| CLI | `weft winui`, `weft winui-tokens`, `weft import-winui` (T105.6). |
| Settings | `export.winui.*` and `import.winui.*` in `weft.json` (T105.6, SPEC §10.6). |

Golden files live in `crates/weft-winui/tests/fixtures/` and are rewritten with `WEFT_UPDATE_FIXTURES=1`, as in the other target crates. The goldens are also the input of the Windows build, so what CI compiles is exactly what the tests compare.

## WinUI side

For a screen named `Login` in namespace `Acme.Screens`, the generator writes:

| File | Content | Read back? |
| --- | --- | --- |
| `LoginPage.xaml` | `<Page x:Class="Acme.Screens.LoginPage">` with the source comment before the root; the tree; `{x:Bind ViewModel.…}` bindings | yes, the only file read |
| `LoginPage.xaml.cs` | `public sealed partial class LoginPage : Page` with `public LoginViewModel ViewModel { get; }`, the constructor calling `InitializeComponent()`, and nothing else | no |
| `LoginViewModel.cs` | `public sealed partial class LoginViewModel : INotifyPropertyChanged`: one property per data path the screen reads, one generated class per object or array item shape, `ObservableCollection<T>` for arrays, one `ICommand` per action, the `Performed` event and `Perform(action, id, item)` | no, derived |

Shared, once per project:

| File | Content |
| --- | --- |
| `WeftTokens.xaml` | A `ResourceDictionary` with every token WinUI has a form for (§ Tokens), merged into `App.xaml` |
| `Weft.cs` | The `Weft` helper namespace: the binding functions (`Not`, `Hide`, the date and colour converters), the inert marker properties (`Props`), the event properties (`On`), the choice and dialog properties, `ColumnsPanel` and `ColumnsLayout`. Plain C#, no NuGet dependency besides the Windows App SDK |

The page is a `Page` because a WinUI app navigates between pages in a `Frame`, and a page hosts in a window as its `Content` as well. The view model is a separate class, not the page, so the host can create, test and share it without UI.

Targets: Windows App SDK 2.5.1 (stable) and .NET 10 (LTS); C# only. The XAML uses no API newer than Windows App SDK 1.5 (`SelectorBar`), so it also builds on older 1.x releases, but only 2.5.1 is tested.

## Mapping of the core catalog

`AutomationId` below is `AutomationProperties.AutomationId`; every element with a Weft `id` gets it, verbatim, so UI Automation tests find the element by the Weft id and the importer reads the id back. `x:Name` is written only where the code-behind or a helper needs a field (none in the first slice). `label` becomes `AutomationProperties.Name` unless the row says the control shows it.

| Weft | WinUI 3 | Props |
| --- | --- | --- |
| `screen` | `Page` → `ScrollViewer` → `StackPanel` | `label` → `AutomationProperties.Name` and `AutomationProperties.LandmarkType="Main"` on the `StackPanel`; `state` → marker |
| `stack` | `StackPanel`; a `Grid` when `justify` is not `start` or a child has `grow` (§ Layout) | `direction` → `Orientation` (`column` → `Vertical`); `gap` → `Spacing`; `align` → children's alignment; `wrap` → marker (no wrapping `StackPanel`); `padding`, `max-width`, `material` (§ Layout, § Tokens) |
| `grid` | `Grid` with `columns` star columns, children placed by `Grid.Row`/`Grid.Column`; `weft:ColumnsPanel` with `min-column-width`; `ItemsRepeater` with `weft:ColumnsLayout` when its only child is an `<each>` | `gap` → `RowSpacing` and `ColumnSpacing` (panel: its `Gap`) |
| `section` | `StackPanel` drawn as a card (the theme's card background and stroke brushes, corner radius, padding) | `label` → `AutomationProperties.Name`, `LandmarkType="Custom"`, `LocalizedLandmarkType="region"`; the `header` slot is the first child |
| `heading` | `TextBlock` | `level` → `Style` from the type ramp (1 `TitleLargeTextBlockStyle`, 2 `TitleTextBlockStyle`, 3 `SubtitleTextBlockStyle`, 4–6 `BodyStrongTextBlockStyle`) and `AutomationProperties.HeadingLevel="Level<n>"` |
| `text` | `TextBlock` with `TextWrapping="Wrap"` | `tone` → `Foreground` from theme brushes (`muted` secondary text, `success`/`warning`/`danger` the system status brushes; names **unverified**) |
| `image` | `Image` | `src` → `Source` (SPEC §9 Trust: a literal of another scheme is a marker; a binding goes through `Weft.Image`, which loads only `http`, `https` and package paths); `label` → `AutomationProperties.Name` |
| `model` | `Image` of the `fallback` | `src` and `usdz` → marker (WinUI has no 3D viewer); a `kinds` loss on reading back without the comment |
| `link` | `HyperlinkButton` | `href` → `NavigateUri` only for `https`, `http`, `mailto` (otherwise a marker; a binding through `Weft.Link`, which applies the same rule); `on-press` → event |
| `button` | `Button` | `variant="primary"` → `Style="{StaticResource AccentButtonStyle}"` (**unverified** key); `danger` → marker; `disabled` → `IsEnabled` (negated); `submit` → presses the form's submit event; `state` → marker |
| `form` | `StackPanel` | `label` → `LandmarkType="Form"`; the `footer` slot is a row `StackPanel` at the end; Enter in a field submits through the `On.Submit` helper |
| `field` | `TextBox`; `PasswordBox` for `type="password"` | `label` → `Header`; `value` → `Text` (`Password`) two-way; `placeholder` → `PlaceholderText`; `type` → `InputScope` (`email` → `EmailSmtpAddress`, `number` → `Number`, `search` → `Search`), `multiline` → `AcceptsReturn="True"` and `TextWrapping="Wrap"`; `required` → `AutomationProperties.IsRequiredForForm`; `disabled` → `IsEnabled`; `error` → marker |
| `checkbox` | `CheckBox` | `label` → `Content`; `checked` → `IsChecked` two-way |
| `switch` | `ToggleSwitch` | `label` → `Header`; `checked` → `IsOn` two-way |
| `radio-group`, `radio` | `RadioButtons` with `RadioButton` items (`Tag` = `value`) | `label` → `Header`; `value` → `weft:Choice.Value` two-way (`RadioButtons` has no value property, only `SelectedIndex` and `SelectedItem`) |
| `segmented-control`, `segment` | `SelectorBar` with `SelectorBarItem` items (`Tag` = `value`) | `value` → `weft:Choice.Value`; `label` → `AutomationProperties.Name` (a selector bar has no header) |
| `select`, `option` | `ComboBox` with `ComboBoxItem` items (`Tag` = `value`, `Content` = text) | `label` → `Header`; `value` → `weft:Choice.Value` two-way (built: `SelectedValue` cannot tell a user's choice from a value the view model writes, which SPEC §9 requires for `change`) |
| `combobox` | `ComboBox IsEditable="True"` with `ComboBoxItem` items (`Content` and `Tag` = `value`, the text the box takes when an item is chosen; the option's text → `ToolTipService.ToolTip`) | `label` → `Header`; `value` → `weft:Choice.Value` two-way (the box's `Text`); `placeholder` → `PlaceholderText` |
| `slider` | `Slider` | `label` → `Header`; `min`/`max` → `Minimum`/`Maximum` (a `max` below `min` written as `min`); `step` → `StepFrequency`; `value` → `Value` two-way |
| `stepper` | `NumberBox SpinButtonPlacementMode="Inline"` | `label` → `Header`; `min`/`max` → `Minimum`/`Maximum`; `step` → `SmallChange`; `value` → `Value` two-way |
| `date-picker` | `CalendarDatePicker` (`date`), `TimePicker` (`time`), both in a row `StackPanel` (`datetime`; the row takes id, name and events, the date picker the `Header`) | `value` → `weft:Choice.Value` two-way, which converts with `Weft.Day`/`DayText`/`Clock`/`ClockText`, so the model keeps SPEC §5.1's text (built: a `BindBack` is a method that writes the source itself, so a static helper cannot be one); `min`/`max` → `MinDate`/`MaxDate` through `Weft.Earliest`/`Latest` for dates, a marker for times and for a literal not of the format |
| `color-picker` | `DropDownButton` showing a swatch (`Weft.Swatch`) and the label, with a `Flyout` holding a `ColorPicker IsAlphaEnabled="False"` | `value` → `weft:Choice.Value` two-way on the button, converted by `Weft.Color`/`Hex` (`#rrggbb`) |
| `list`, `item` | `ListView SelectionMode="None"` with `ListViewItem` children, or `ItemsSource` for one `<each>` | `ordered` → marker; the `empty` slot is the list's `Footer`, shown by `Visibility` through `Weft.Empty` when there is nothing to show; `item` `on-press` → event (`IsItemClickEnabled`) |
| `table`, `column`, `row`, `cell` | `StackPanel` of a header `Grid` (one star column per `column`, each a `Button` for `press`) and one `Grid` per row; an `<each>` of rows is an `ItemsRepeater` | `sort` and `selected` → markers; `empty` as in `list`. `TableView` exists only in the experimental channel (§ Open questions 10) |
| `tabs`, `tab` | `TabView` with `TabViewItem IsClosable="False"`, `IsAddTabButtonVisible="False"` | `label` → `Header`; `selected` → `weft:Choice.Value` two-way against each item's `Tag` (the tab id) |
| `dialog` | `ContentDialog` | `open` → `weft:Dialog.IsOpen` two-way: the helper shows the dialog when it turns true (once the page is loaded, so the dialog has its `XamlRoot`), hides it when false, and on a user dismissal writes `false` and fires `close`; `label` → `AutomationProperties.Name` (no `Title`); the `actions` slot is a row at the end of the content (the dialog's own buttons stay unused); `modal` → marker (a `ContentDialog` is always modal) |
| `alert` | `InfoBar IsOpen="True" IsClosable="False"` | `tone` → `Severity` (`info` → `Informational`, `success` → `Success`, `warning` → `Warning`, `danger` → `Error`); text → `Message`, mixed content → `Content` |
| `menu`, `menu-item` | `DropDownButton` with a `MenuFlyout` of `MenuFlyoutItem` | `label` → the button's `Content` and `AutomationProperties.Name` (shown, unlike on the web); `menu-item` `disabled` → `IsEnabled`; `on-press` → event |
| `<each>` | `ItemsRepeater` with an inline `DataTemplate x:DataType="local:<Item>"`, `StackLayout` matching the parent stack's orientation and spacing | the `each` id and `as` → markers `weft:Each.Id`, `weft:Each.As` on the repeater |
| `<each>` among options or menu items | The `ComboBox` or `MenuFlyout` itself, with `weft:Each.Source` (the list), `weft:Each.At` (the static items before it, left out when 0) and a `weft:Each.Template` holding a `DataTemplate x:DataType="local:<Item>"` of one `ComboBoxItem` or `MenuFlyoutItem` (T105.16) | the `each` id and `as` → `weft:Each.Id`, `weft:Each.As` beside them; at run time `Each` loads the template per item, sets the item as its `DataContext`, inserts it after `At` static items, refills on a list change and shows the choice's value again |
| `<slot>` | The slot's elements placed where the row of the owning kind says | — |
| `<use>` | Expanded before generating (as every target) | the source comment keeps the `<use>` |

Universal attributes:

| Weft | WinUI 3 |
| --- | --- |
| `id` | `AutomationProperties.AutomationId` |
| `hidden` | `Visibility="Collapsed"`; bound: `Visibility="{x:Bind weft:Weft.Hide(ViewModel.x), Mode=OneWay}"`, negated `{!$.x}`: `{x:Bind ViewModel.x, Mode=OneWay}` (x:Bind converts `bool` to `Visibility` itself). Collapsed elements leave the UI Automation tree, as SPEC requires |
| `state` | marker `weft:Props.State` |
| `rotate-x`, `rotate-y`, `rotate-z`, `perspective` | `Transform3D`: a `CompositeTransform3D` with `RotationX`, `RotationY`, `RotationZ` the negated angles (WinUI turns counter-clockwise, CSS clockwise), centred by `weft:Tilt.Centered` from `Weft.cs`, which sets `CenterX` and `CenterY` to half the element's size whenever it changes (the centre is in pixels). An element has one `Transform3D`, so a `perspective` is a `Border` around it whose `Transform3D` is a `PerspectiveTransform3D` (`Depth` = `perspective`); a perspective on a parent projects its children (research §23). The order of the three rotations inside one transform is **unverified**. A tilt on a `screen` or on an item a control holds itself (`option`, `item`, `row`, `tab`, …) has no place for a `Border` and is refused for now |
| `grow` | star size in the stack's `Grid` (§ Layout) |
| `on-<event>` | `weft:On.<Event>` (§ Events) |

A kind of the project's catalog extension is refused (`Unsupported`) in T105 (open question 6).

## Markers

What WinUI cannot draw but a Weft screen states (`state`, `tone` of a button, `error`, `required` where no automation property fits, `ordered`, `sort`, `selected`, `wrap`, `modal`, the 3D model paths, a time picker's bounds) is kept as an inert attached property of the `Weft.Props` class in `Weft.cs`: `weft:Props.Error="{x:Bind ViewModel.emailError, Mode=OneWay}"`. A marker does nothing at run time; it builds, it reads back, and an app may style on it. This is SwiftUI's inert marker modifier (SPEC §9) in XAML form. The importer turns each marker back into its prop, so these props are lost from what the user sees, not from the round trip.

## Data and bindings

- **Property names are the Weft path names, verbatim** (open question 2). `{$.user.email}` is `{x:Bind ViewModel.user.email, Mode=OneWay}`; the view model has a property `user` of a generated class with a property `email`. A Weft name is `[A-Za-z_][A-Za-z0-9_]*`, which is a C# identifier, so nothing is renamed and the importer reads the path straight back, with one exception below. A name that equals a member the generator writes (`PropertyChanged`, `Perform`, `Performed`, the `Weft`-prefixed members) is refused.
- **C# keywords take a `_` suffix (T105.17).** A Weft name that is a C# reserved keyword, alone or followed by any number of `_`, is the member with one more `_`: `{$.account.public}` is `ViewModel.account.public_` (property `public_`, backing field `Weft_public_`), and a real `public_` is `public__`. The importer strips exactly one `_` from a member that is a keyword followed by one or more `_`, so every name has a member of its own and the round trip is exact; a bare keyword, which XAML that builds cannot hold, reads as itself. Only the C# member changes: the data path, the `WeftPath` an event reports and the class names (`<Name>ClassItem`) keep the Weft name. The C# way, the verbatim identifier `@public`, does not work in x:Bind: in the open-source XAML compiler (microsoft/microsoft-ui-xaml `main` @ 866ba81a, checked 2026-10-17) the `'@' Identifier_or_keyword` branch of `src/XamlCompiler/BuildTasks/BindingPath/CSharpIdentifier.g4` is commented out, `BindingPath.g4` accepts no `@`, and `PropertyStepCodeGenerator` in `src/XamlCompiler/BuildTasks/CodeGenerators/BindPathStepCodeGenerators.cs` writes a property name into the generated `.g.cs` unescaped, so a bare keyword fails with CS1055 and CS0106 (`class` and `event` in T105.7's run 38656182916). The keywords are the 77 of the first table of the C# reference, <https://learn.microsoft.com/en-us/dotnet/csharp/language-reference/keywords/> (page updated 2026-10-15, checked 2026-10-17): `abstract` `as` `base` `bool` `break` `byte` `case` `catch` `char` `checked` `class` `const` `continue` `decimal` `default` `delegate` `do` `double` `else` `enum` `event` `explicit` `extern` `false` `finally` `fixed` `float` `for` `foreach` `goto` `if` `implicit` `in` `int` `interface` `internal` `is` `lock` `long` `namespace` `new` `null` `object` `operator` `out` `override` `params` `private` `protected` `public` `readonly` `ref` `return` `sbyte` `sealed` `short` `sizeof` `stackalloc` `static` `string` `struct` `switch` `this` `throw` `true` `try` `typeof` `uint` `ulong` `unchecked` `unsafe` `ushort` `using` `virtual` `void` `volatile` `while` (`KEYWORDS` in `crates/weft-winui/src/data.rs`). Contextual keywords (`value`, `var`, `field`, `record`, …) are identifiers wherever a member is, so they stay ordinary names.
- **Integer indexes** are x:Bind indexers: `{$.items.0.name}` is `ViewModel.items[0].name` (x:Bind needs `IList<T>`, which `ObservableCollection<T>` is).
- **`hidden` through a function** returns a `Visibility` itself, because x:Bind's built-in `bool` to `Visibility` conversion applies to property bindings only.
- **Types** come from the props that read each path, as in SwiftUI (SPEC §9): text props `string`, booleans `bool`, the number controls `double`, arrays `ObservableCollection<T>` of a generated item class. A path read with two types is refused with both paths. The XAML does not depend on the types, so the importer needs only the XAML.
- **Mode.** x:Bind defaults to `OneTime`, so every binding of data states `Mode=OneWay`, and writable props `Mode=TwoWay`. A `TextBox` writes on every keystroke (`UpdateSourceTrigger=PropertyChanged`; its default is `LostFocus`), as the web target does on `input`.
- **Negation and truthiness** use function bindings, since x:Bind has no operators: `{!$.x}` read as a boolean is `{x:Bind weft:Weft.Not(ViewModel.x), Mode=OneWay}`. `Not` and `Truthy` take `object` and apply SPEC §2.1's falsy rule (`false`, `0`, empty string, null, missing). `disabled` is `IsEnabled` with the opposite sense, so `disabled="{$.busy}"` is `IsEnabled="{x:Bind weft:Weft.Not(ViewModel.busy), Mode=OneWay}"` and `disabled="{!$.ready}"` is `IsEnabled="{x:Bind weft:Weft.Truthy(ViewModel.ready), Mode=OneWay}"`. A negated binding is never two-way (SPEC §5.1).
- **Inside `<each>`** the template's root is the item: `{$todo.title}` is `{x:Bind title, Mode=OneWay}`. Each generated item class also has `WeftRoot` (the view model, for `{$.…}` read inside the loop: `{x:Bind WeftRoot.filter}`), `WeftParent` (the enclosing loop's item, for nested loops) and `WeftPath` (the item's data path, for events). The view model sets them when it fills an array.

## Events and commands

Every action the screen names is a command property of the view model, named from the action: `.` becomes `_`, and `Command` is appended (`todo.add` → `todo_addCommand`). Action names contain no `_` (SPEC §2.2), so the name reads back exactly.

Every `on-<event>` is an attached property of `Weft.On` bound to that command: `weft:On.Press="{x:Bind ViewModel.todo_addCommand}"`, `weft:On.Change`, `weft:On.Submit`, `weft:On.Close`. The helper subscribes to the control's own event (`Click`, `TextChanged`, `Toggled`, `SelectionChanged`, `ValueChanged`, `ItemClick`, …) and executes the command with a `WeftEvent(action, id, item)`: the id from `AutomationId`, the item from the template item's `WeftPath`, so the host gets what SPEC §2.2 says a host receives. Inside a template the binding goes through the item: `{x:Bind WeftRoot.todo_toggleCommand}`. Every command calls `Perform`, which raises the view model's `Performed` event; the host handles it, or overrides `partial void OnPerformed(WeftEvent e)` (open question 1).

The native `Command` property of a button is not used for Weft events, because it carries neither the element id nor the loop item, and only buttons and menu items have it. The importer still reads a hand-written `Command="{x:Bind ViewModel.xCommand}"` or `Click="{x:Bind ViewModel.x}"` as `on-press="x"` when the name maps back.

A `button submit="true"` gets `weft:On.Submit` bound to the enclosing form's submit command (the event carries the form's id), next to its own `weft:On.Press` if it has one; the helper fires `press` first and `submit` second, once per press (SPEC §5.1). Enter in a field of the form fires the same `submit`.

## Layout vocabulary

The vocabulary of `docs/layout-design.md` maps onto WinUI's panels:

| Weft | WinUI 3 |
| --- | --- |
| `stack` with `justify="start"` and no `grow` child | `StackPanel` with `Orientation` and `Spacing` |
| `justify` `center`, `end`, `space-between` | A `Grid`: one `Auto` track per child, `Spacing` as `ColumnSpacing` (`RowSpacing` for a column), and star tracks for the free space: `center` a star track at each end, `end` one at the start, `space-between` one between each pair. A single child with `space-between` sits at the start (SPEC §5.1), so that stack stays a `StackPanel` with the `weft:Props.Justify` marker. Spacer tracks are empty, so the gap is still the least space |
| `grow="true"` | The child's track is `*` instead of `Auto`; with any growing child the stack has no spacer tracks (SPEC §5.1: `justify` then has no effect, and is kept as the `weft:Props.Justify` marker). A row whose every child grows is refused: star columns alone read back as a `grid` |
| `align` on a row | each child's `VerticalAlignment` (`start` → `Top`, `center`, `end` → `Bottom`, `stretch`); the default `center` is written out so the importer does not read it back as `align` |
| `align` on a column | each child's `HorizontalAlignment` (`start` → `Left`, `center`, `end` → `Right`, `stretch`) |
| `padding` | `Padding` (a `Thickness` token resource) on the `StackPanel` or `Grid`; inside a `material` surface it is the `Border`'s `Padding` |
| `max-width` | `MaxWidth` (the token's `x:Double`), stretching below it as every panel does; where the element sits in a wider parent is the parent's `align`, as in SPEC |
| `min-column-width` | `weft:ColumnsPanel` (static children; T105.5) or `weft:ColumnsLayout` (one `<each>`; T105.18): about 60 lines each in `Weft.cs`, showing `columns` columns while each is at least the token wide and fewer, down to one, below that, children in order |
| `wrap` | marker |

`<each>` is transparent in Weft, so its items are the parent's children. XAML panels cannot mix static children with a bound collection, so an `<each>` becomes a nested `ItemsRepeater` with the parent's orientation and spacing. That draws the same in a `start`-justified stack, a list and a table. In a `grid`, or in a stack that becomes a `Grid` (justify or grow), an `<each>` beside other children cannot share the tracks, so the generator refuses it (`Unsupported` with the path). No corpus screen does it. The same holds for any parent that is not a panel (`row`, `tabs`, `radio-group`, `segmented-control`) and for a list whose `<each>` sits beside static items or repeats more than one `item`: a `ListView` takes either children or an `ItemsSource`, and its template draws one item. A `select`, `combobox` or `menu` cannot hold a repeater either, and a `ComboBox` takes either items or an `ItemsSource` (`example-option` mixes an `<each>` with a static option) while a `MenuFlyout` has no `ItemsSource` at all (research.md §23). So an `<each>` of one `option` or `menu-item` there keeps its typed template on the `ComboBox` or `MenuFlyout` as `weft:Each.Template`, and the `Each` helper of `Weft.cs` loads it once per item, the item as the element's `DataContext`: compiled bindings in a template update from the root's `DataContextChanged`, as in an items control. One such `<each>` per parent, of one item, is drawn; more is refused. A template is typed by the item class, so its paths are the item's members, `WeftParent` an enclosing loop's item and `WeftRoot` the view model, which every command in it is read from.

## Tokens

| Token type | XAML resource | Used as |
| --- | --- | --- |
| dimension | `<x:Double x:Key="Weft.space.md">12</x:Double>`, and for a token used as padding also `<Thickness x:Key="Weft.space.md.thickness">12</Thickness>` | `{StaticResource …}` on `Spacing`, `RowSpacing`, `ColumnSpacing`, `MaxWidth`, `Padding` |
| colour | `<Color>` and `<SolidColorBrush>` in `ThemeDictionaries` `Light`, `Dark` and `HighContrast` | `{ThemeResource …}` |
| material | `AcrylicBrush` (`TintColor`, `TintOpacity`, `FallbackColor`) in `ThemeDictionaries`; in `HighContrast` a `SolidColorBrush` of the fallback | `Background="{ThemeResource Weft.material.glass}"` on a `Border` around the stack or grid |
| typography, duration, cubic Bézier | not written: no core prop of a WinUI control takes them | — |

- Keys are `Weft.` followed by the token path. A dimension token's path cannot have children (a DTCG token is not a group), so the `.thickness` suffix cannot collide. The Windows build compiles keys with dots and with the `$root` of a colour group (`Weft.color.blue.$root`).
- Dimensions are effective pixels: `px` as is, `rem` × 16, as the other targets.
- `{StaticResource}` for dimensions, because they do not change with the theme; `{ThemeResource}` for colours and materials, which follow the system theme at run time.
- **Light and dark.** With an appearance (SPEC §10.3), a colour whose `dark` value differs from its `light` one gets both dictionaries; otherwise `Light` and `Dark` hold the same value. `HighContrast` holds the light colours and each material's fallback colour as a plain brush; the text tones use the system's brushes, so they follow the high-contrast theme.
- **Shared or per-page** (open question 3; T105.18): with `export.winui.sharedTokens` (default `true`) a page references `WeftTokens.xaml` merged in `App.xaml`; with `false` the page's `Page.Resources` holds only the tokens it uses, so a single page builds on its own.
- A blur radius, and the glass refraction of a material, have no `AcrylicBrush` form: a `tokens` loss. The text tones use the system's theme brushes, not tokens.

## Slots, light and dark, accessibility

- **Slots.** `section.header` is the first child of the section's `StackPanel`, `form.footer` the row at its end, `dialog.actions` the row at the end of the dialog content, `list.empty` and `table.empty` an element after the items whose `Visibility` follows the empty rule (SPEC §5.1) through `weft:Weft.Empty(…)`. The slot name reads back from a marker `weft:Slot.Name` on that element, because nothing else in the XAML names it.
- **Light and dark** follow the system theme through `{ThemeResource}` and the WinUI controls' own theme resources. The generator sets no `RequestedTheme`.
- **Accessibility** goes through `AutomationProperties`: `AutomationId`, `Name`, `HeadingLevel`, `LandmarkType` and `LocalizedLandmarkType`, `IsRequiredForForm`. The controls give their own control types (button, check box, combo box, slider, spinner, tab, …). What has no UI Automation pattern (a table's `table`/`row`/`cell` structure through grids, list `ordered`) is a loss to assistive technology, listed in SPEC §9.

## Source comment and round trip

The XAML file starts with the XML declaration, then the source comment, then the `Page`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- weft:source winui name=Login namespace=Acme.Screens
<screen id="login" weft="0.2">
  …
</screen>
-->
<Page x:Class="Acme.Screens.LoginPage" …>
```

- XML forbids `--` anywhere in a comment and a `-` just before its end (W3C XML 1.0 §2.5). The comment escapes with `\`: `\` → `\\`, every `-` that follows a `-` → `\-`, a final `-` → `\-`, and `@` → `@\` as SPEC §9 requires of every source comment. The existing provenance helper already escapes the HTML comment end; T105.1 extends it with the stricter XML case instead of writing a second escaper.
- **Exact read-back (T105.1).** `import_winui` finds the comment, parses its markup, regenerates the XAML from it with the `name` and `namespace` in the comment and the shared-tokens mode the file shows, and accepts the comment's screen only when both XAML files are equal as XML: the same elements, the same attributes in any order, the same non-blank text, comments other than the source comment and whitespace ignored. In the per-page token mode the `Weft.*` entries of `Page.Resources` are compared by key and type, not value, because the values come from the project's tokens and are not part of the screen. The C# files are never read: they are derived from the screen.
- **Context.** The source comment carries the context block unless `export.winui.context` is `strip`; `import.winui.context` `drop` discards it on the way back (SPEC §10.6), as for the other targets. No readable context comments above elements in T105.
- **Hand-written or edited XAML (T105.8, T105.9).** When the comment is missing or does not match, `read_winui` reads the elements: the controls of the mapping table become their kinds, `AutomationId` the ids, the markers their props, `{x:Bind ViewModel.…}` paths the bindings, `weft:On.*` and the command names the actions, `{StaticResource Weft.…}` and `{ThemeResource Weft.…}` the token references, `Grid` star tracks back into `justify` and `grow`. What does not map is a loss of the SPEC §9 kinds (`ids`, `bindings`, `actions`, `tokens`, `layout`, `repetition`, `slots`, `hidden`, `props`, `values`, `names`, `kinds`, `text`, `structure`, `context`), with the XAML path, never a silent drop. An element without an `AutomationId` gets a generated id and an `ids` loss, as in the other readers.

The round trip is tested both ways on every corpus screen and catalog example: `import_winui(generate(x)) == x`, and `read_winui(generate(x))` with the source comment removed gives `x` with only the losses the table lists.

## XML parser

`rust.md` lists no XML crate (its parsing section has `html5ever`, `oxc_parser` and `tree-sitter`). The choice is **`roxmltree` 0.21**:

- it is already in `Cargo.lock` (0.21.1, through `usvg` and `fontique`), so pinning `0.21` adds no new crate to the lock file;
- pure Rust with no C dependency, so the importer also builds for `wasm32` if the importer is ever exposed there;
- a read-only tree with namespace resolution, which XAML needs (`x:`, `d:`, `mc:`, `using:` namespaces and attached properties `AutomationProperties.Name`), and the text position of every node, for diagnostics;
- safe defaults for untrusted input: a DTD is an error unless `allow_dtd` is set (so no entity expansion), and `nodes_limit` bounds the tree, which maps onto `W602`;
- MIT or Apache-2.0, released 2025-10-12.

`quick-xml` 0.42 (2026-08-22) is the alternative: a streaming reader and writer, more active. The importer wants a tree with namespaces resolved and positions, which `quick-xml` leaves to the caller, and the generator writes its XAML as text itself (as `weft-slint` writes Slint), so `quick-xml`'s writer is not needed. `xml-rs` is also in the lock file but is a streaming reader with the same gap. `roxmltree` needs a row in `rust.md`, which lives outside this repository; the creator adds it or permits it (T105.8 flags it).

## CI: the Windows build

| Item | Choice |
| --- | --- |
| Where | A new job `winui` in `.github/workflows/ci.yml`, beside `gate` |
| Runner | `windows-latest` (Windows Server 2025 image) |
| Steps | `actions/checkout` (pinned by commit SHA, as the other jobs); `actions/setup-dotnet` v6 (pinned by SHA) with `dotnet-version: 10.0.402`; `dotnet build crates/weft-winui/tests/build/WeftScreens.csproj -c Release -p:Platform=x64` |
| Project | A WinUI 3 class library: `UseWinUI` `true`, `OutputType` `Library`, `TargetFramework` `net10.0-windows10.0.<sdk>` (the Windows SDK build is taken from the current WinUI template when the job is written, **unverified** here), `Microsoft.WindowsAppSDK` 2.5.1, `TreatWarningsAsErrors`. It includes the golden `.xaml`, `.xaml.cs` and `.cs` files of `tests/fixtures/` and the shared `WeftTokens.xaml` and `Weft.cs` (the library has no `App.xaml`, so `WeftTokens.xaml` is merged by a small resource file of the project), so every page goes through the XAML compiler and the C# compiler |
| What it proves | Every generated page, view model and helper compiles: x:Bind paths resolve against the view model's types, function bindings and attached properties exist, resource keys parse. It does not run the pages |
| No Rust on Windows | The goldens are generated and checked on the macOS gate (`cargo nextest`, `WEFT_UPDATE_FIXTURES=1` to rewrite). A Rust test asserts that every corpus screen and every catalog example has a golden, so the Windows build covers the whole catalog |
| Required? | Not required at first, required once it has passed for a while (open question 7) |

The XAML compiler runs only on Windows (**unverified**; it ships as a Windows executable in the Windows App SDK package), which is why the macOS gate cannot replace this job. Screenshots of the pages are a later task: they need an app, a window and a desktop session on the runner.

## Losses

Weft → WinUI (shown, but kept in the source comment and in markers): `wrap` (no wrapping stack), `button` `danger`, `field` `error`, `list` `ordered`, `column` `sort`, `row` `selected`, `dialog` `modal="false"`, a time picker's `min`/`max`, `model` (its still is drawn), a material's blur, a `menu` label that the web does not show is shown, a `section` drawn as a card, a table's cell structure for assistive technology, an `href` with a scheme other than `https`, `http` or `mailto` (not navigable). Refused: a catalog extension kind, an `<each>` beside other children in a grid or in a justified or growing stack, a second `<each>` in a `select`, `combobox` or `menu`, or one there of anything but one `option` or `menu-item`, a data name that is a generated member, a path read with two types.

WinUI → Weft (hand-written XAML): controls outside the table (`kinds`), layout panels other than `StackPanel` and `Grid` (`structure`), bindings other than `{x:Bind ViewModel.path}` with the helper functions above (`bindings`; `{Binding}` is read as the same path with a `bindings` loss for the mode), event handlers in code-behind (`actions`), literal colours and sizes (`tokens`, `values`), `Margin`, `Width`, `Height` and other sizes (`layout`), styles and templates other than the type ramp (`props`).

## Implementation outline

Each sub-task is at most 500 lines of code plus tests (AGENTS.md), and each updates SPEC §9 for what it adds before the code (spec first).

| Task | Scope | Code lines (estimate) |
| --- | --- | --- |
| T105.1 | Crate, names and data typing, XAML writer, first slice (`screen`, `stack` without justify or grow, `text`, `heading`, `button`, `field`, `checkbox`, `switch`, `form`), the source comment and `import_winui`, goldens | 480 |
| T105.2 | The view model and page code-behind (properties, item classes, commands, `Perform`), `Weft.cs` core (`On.*`, `Not`, `Truthy`, `Hide`, `Props`) | 450 |
| T105.3 | The value and choice controls: `select`, `combobox`, `radio-group`, `segmented-control`, `slider`, `stepper`, `date-picker`, `color-picker`, `link`, `image`, `model`, `alert`, `menu`, and their helpers | 480 |
| T105.4 | Containers: `list`, `table`, `tabs`, `dialog`, `section`, `grid` without `min-column-width`, the `header` and `actions` slots | 250 |
| T105.14 | Repetition: `<each>` as an `ItemsRepeater`, the `empty` slots | 240 |
| T105.5 | Layout vocabulary (`justify`, `grow`, `align`, `padding`, `max-width`, `min-column-width` with `ColumnsPanel`) | 230 |
| T105.15 | Colour tokens and theme dictionaries, material, 3D tilt, and reading them back | 500 |
| T105.18 | Per-page tokens (`export.winui.sharedTokens`), `ColumnsLayout` for a repeated `min-column-width` grid | 300 |
| T105.6 | CLI `weft winui`, `weft winui-tokens`, `weft import-winui`; `weft.json` settings, SPEC §10.6 rows, schema | 250 |
| T105.7 | The Windows CI job and build project; the golden-completeness test | 120 |
| T105.8 | `read_winui`: kinds, ids, props, markers, structure, slots, with losses and diagnostics | 480 |
| T105.9 | `read_winui`: bindings, function bindings, events and commands, tokens, layout back into `justify`/`grow`/`align` | 450 |
| T105.10 | Closing: SPEC §9 "From WinUI" and loss table complete, AGENT-SPEC check, crate README, the corpus-wide round-trip test | 60 |

Order: T105.1 → T105.2 → T105.7 (the build exists before most controls, so each later slice is compiled as it lands) → T105.3, T105.4, T105.5 (independent of each other) → T105.6 (any time after T105.2) → T105.8 → T105.9 → T105.10.

## Alternatives considered

| Alternative | Why not |
| --- | --- |
| `{Binding}` with a `DataContext` instead of `{x:Bind}` | Reflection at run time, errors only at run time; x:Bind is compiled and checked by the CI build (creator decision 2) |
| CommunityToolkit.Mvvm source generators (`[ObservableProperty]`, `[RelayCommand]`) | Shorter C#, but a NuGet dependency in every app and a second code generator between Weft and the build; the generated C# is not read back, so its length matters little (open question 1) |
| PascalCase view-model properties | Idiomatic C#, but `name` and `Name` collide, and the XAML alone no longer gives the Weft path back (open question 2) |
| Native `Command`/`CommandParameter` on buttons | Carries no element id and no loop item; only buttons and menu items have it |
| `UniformGridLayout` for `min-column-width` | Sizes from the first item when no minimum is set and has no fixed column count; it also needs an `ItemsRepeater`, so static children cannot use it |
| `TableView` for `table` | Only in the experimental channel (2.5.4-experimental) |
| Generating a `UserControl` instead of a `Page` | A page is what a WinUI app navigates to; a page also sits in a window's content, so a `Page` covers both |
| Reading the C# view model back | The XAML holds every path and action name, verbatim; reading C# would need a C# parser for no gain (creator decision 2) |
| `quick-xml` | See § XML parser |

## Open questions

All answered: the creator accepted the recommendation of each question below.


1. **View-model base.** Plain generated C# implementing `INotifyPropertyChanged` and `ICommand`, no NuGet dependency, or the CommunityToolkit.Mvvm 8.4.2 source generators? **Recommendation:** plain generated C#; the app adds the toolkit if it wants it. Also: the host hears actions through a `Performed` event and a `partial void OnPerformed(WeftEvent)` hook; **recommendation:** both, the event for hosts that compose, the partial method for hosts that subclass nothing.
2. **Property names.** Verbatim Weft names (`ViewModel.user.email`) or PascalCase (`ViewModel.User.Email`) with refusal on collisions? **Recommendation:** verbatim: no renaming, no collisions, and the XAML reads back without the C#.
3. **Tokens shared or per page.** **Recommendation:** shared `WeftTokens.xaml` by default (`export.winui.sharedTokens` `true`, as SwiftUI), per page when `false` or without a project.
4. **Actions inside `<each>`.** **Recommendation:** each generated item class carries `WeftRoot`, and templates bind `{x:Bind WeftRoot.<action>Command}`; the alternative, a literal action name resolved by walking up the visual tree, is not compile-checked.
5. **`min-column-width`.** **Recommendation:** the generated `ColumnsPanel` and `ColumnsLayout` in `Weft.cs` (about 120 lines together), not `UniformGridLayout`, which cannot hold a fixed column count or take static children.
6. **Catalog-extension kinds.** Refuse them, or generate `local:<Kind>Control` user controls the app writes (as SwiftUI calls `<Kind>View`)? **Recommendation:** refuse in T105, a follow-up task for user controls.
7. **The Windows job as a required check.** **Recommendation:** not required until it has passed ten runs in a row on `main`, then required, because a new runner image or SDK can break it for reasons outside the change.
8. **Importer in WebAssembly and MCP.** **Recommendation:** not in T105; no native target has those surfaces yet, and `roxmltree` keeps the option open.
9. **`menu` label.** WinUI has no always-open menu outside a flyout, so the menu becomes a drop-down button showing its label. **Recommendation:** accept that as a listed loss.
10. **`table`.** A grid of grids now, or wait for `TableView` to reach the stable channel? **Recommendation:** grids now, `TableView` in a follow-up when it is stable.
11. **`rust.md`.** `roxmltree` needs a row in the shared `rust.md`, outside this repository. **Recommendation:** the creator permits the row, added with T105.8.
