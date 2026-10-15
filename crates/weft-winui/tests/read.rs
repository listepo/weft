//! WinUI → Weft for XAML `import_winui` rejects: a generated page without its source comment reads
//! back to its document with no loss, each binding, event, token and layout form of the mapping
//! reads back, a hand-written page reports each loss at its path,
//! and hostile or oversized input is refused with `W601` or cut with `W602`. The results always
//! validate in lenient mode. `WEFT_UPDATE_FIXTURES=1` rewrites the `.read.*` goldens.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{covered, here, setup};
use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{
    Catalog, Child, Code, Mode, Node, Severity, ValidateOptions, Value, serialize,
    validate_document,
};
use weft_winui::{
    ImportOptions, ImportResult, LossKind, MAX_SOURCE_LENGTH, MAX_XML_DEPTH, read_winui,
};

fn read(source: &str, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> ImportResult {
    read_winui(source, &ImportOptions { catalog, tokens })
}

/// The page without its `weft:source` comment, as a hand edit that drops it leaves it.
fn bare(xaml: &str) -> String {
    let start = xaml.find("<!-- weft:source").unwrap();
    let end = xaml.find("-->\n<Page").unwrap() + "-->\n".len();
    format!("{}{}", &xaml[..start], &xaml[end..])
}

fn losses(result: &ImportResult) -> String {
    result
        .losses
        .iter()
        .map(|l| format!("{:?} {}: {}\n", l.kind, l.path, l.note))
        .collect()
}

fn lenient(result: &ImportResult, catalog: &Catalog) {
    let diagnostics = validate_document(
        &result.document,
        &ValidateOptions {
            catalog: Some(catalog),
            mode: Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Compares `actual` with the golden file `name`, or rewrites it under `WEFT_UPDATE_FIXTURES`.
fn golden(name: &str, actual: &str) {
    let path = here(&format!("tests/fixtures/{name}"));
    if std::env::var_os("WEFT_UPDATE_FIXTURES").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(actual, expected, "{name}");
}

/// Removes the props the generator leaves out because they hold their default.
fn without_defaults(node: &mut Node) {
    let defaults = [
        ("tone", "default"),
        ("justify", "start"),
        ("variant", "secondary"),
    ];
    node.props.retain(|prop, value| {
        !defaults
            .iter()
            .any(|(p, d)| p == prop && *value == Value::String((*d).into()))
    });
    let slots = node.slots.values_mut().flatten();
    for child in node.children.iter_mut().chain(slots) {
        if let Child::Node(n) = child {
            without_defaults(n);
        }
    }
}

#[test]
fn generated_pages_read_back_without_their_comment() {
    let (catalog, tokens) = setup();
    for (name, doc, generated) in covered() {
        let result = read(&bare(&generated.xaml), &catalog, &tokens);
        assert!(
            result.diagnostics.is_empty(),
            "{name}: {:?}",
            result.diagnostics
        );
        assert_eq!(losses(&result), "", "{name}");
        lenient(&result, &catalog);
        let mut want = weft_core::expand(&doc, &catalog).document;
        want.weft = result.document.weft.clone();
        without_defaults(&mut want.root);
        assert_eq!(serialize(&result.document), serialize(&want), "{name}");
        if name == "controls" {
            golden("controls.read.weft", &serialize(&result.document));
            golden("controls.read.losses", &losses(&result));
        }
    }
}

const PAGE: &str = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" xmlns:weft="using:Shop" xmlns:sys="clr-namespace:System"><StackPanel AutomationProperties.AutomationId="s">"#;

/// The screen and losses read from `body` inside a page's screen panel.
fn read_body(body: &str) -> (String, String) {
    let (catalog, tokens) = setup();
    let result = read(
        &format!("{PAGE}{body}</StackPanel></Page>"),
        &catalog,
        &tokens,
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lenient(&result, &catalog);
    let markup = serialize(&result.document);
    let inner = markup
        .lines()
        .filter(|l| l.starts_with("  "))
        .map(|l| format!("{}\n", &l[2..]))
        .collect();
    (inner, losses(&result))
}

#[test]
fn x_bind_paths_functions_and_modes_are_bindings() {
    let (doc, lost) = read_body(concat!(
        r#"<TextBlock AutomationProperties.AutomationId="a" Text="{x:Bind ViewModel.user.name, Mode=OneWay}" Visibility="{x:Bind weft:Weft.Hide(ViewModel.gone), Mode=OneWay}" />"#,
        r#"<TextBox AutomationProperties.AutomationId="b" Header="B" Text="{x:Bind Path=ViewModel.items[2].title, Mode=TwoWay, UpdateSourceTrigger=PropertyChanged}" IsEnabled="{x:Bind weft:Weft.Not(ViewModel.busy), Mode=OneWay}" />"#,
        r#"<StackPanel AutomationProperties.AutomationId="w" Orientation="Horizontal" weft:Props.Wrap="{x:Bind weft:Weft.Not(ViewModel.tight), Mode=OneWay}" />"#,
        r#"<PasswordBox AutomationProperties.AutomationId="c" Header="C" Password="{x:Bind ViewModel.secret, Mode=TwoWay}" IsEnabled="{x:Bind weft:Weft.Truthy(ViewModel.ready), Mode=OneWay}" />"#,
        r#"<CheckBox AutomationProperties.AutomationId="d" Content="D" IsChecked="{x:Bind WeftRoot.flag, Mode=TwoWay}" IsEnabled="{x:Bind ViewModel.ready, Mode=OneWay}" Visibility="{x:Bind ViewModel.shown, Mode=OneWay}" />"#,
        r#"<Button AutomationProperties.AutomationId="e" Content="{x:Bind ViewModel.label, Mode=OneTime}" weft:Props.State="{x:Bind ViewModel.state}" />"#,
    ));
    assert_eq!(
        doc,
        concat!(
            r#"<text id="a" hidden="{$.gone}" text="{$.user.name}"/>"#,
            "\n",
            r#"<field id="b" disabled="{$.busy}" label="B" value="{$.items.2.title}"/>"#,
            "\n",
            r#"<stack id="w" direction="row" wrap="{!$.tight}"/>"#,
            "\n",
            r#"<field id="c" disabled="{!$.ready}" label="C" type="password" value="{$.secret}"/>"#,
            "\n",
            r#"<checkbox id="d" checked="{$.flag}" disabled="{!$.ready}" hidden="{!$.shown}" label="D"/>"#,
            "\n",
            r#"<button id="e" state="{$.state}" text="{$.label}"/>"#,
            "\n",
        )
    );
    // x:Bind reads a value once unless a mode says otherwise; the binding is kept all the same.
    assert_eq!(
        lost,
        concat!(
            "Bindings /screen#s/button#e: `Content` on `Button`: without `Mode=OneWay` x:Bind reads the value once; it is read as a live binding\n",
            "Bindings /screen#s/button#e: `Props.State` on `Button`: without `Mode=OneWay` x:Bind reads the value once; it is read as a live binding\n",
        )
    );
}

#[test]
fn bindings_with_no_weft_form_are_losses() {
    let (doc, lost) = read_body(concat!(
        r#"<TextBlock AutomationProperties.AutomationId="a" Text="{Binding Name}" />"#,
        r#"<TextBlock AutomationProperties.AutomationId="b" Text="{x:Bind title, Mode=OneWay}" />"#,
        r#"<TextBlock AutomationProperties.AutomationId="c" Text="{x:Bind ViewModel.name, Mode=OneWay, Converter={StaticResource Upper}}" />"#,
        r#"<TextBlock AutomationProperties.AutomationId="d" Text="{x:Bind sys:Weft.Not(ViewModel.x), Mode=OneWay}" />"#,
        r#"<TextBlock AutomationProperties.AutomationId="i" Text="{x:Bind weft:Weft.Not(ViewModel.x), Mode=OneWay}" />"#,
        r#"<TextBlock AutomationProperties.AutomationId="e" Text="{x:Bind ViewModel.a[0]b, Mode=OneWay}" />"#,
        r#"<Button AutomationProperties.AutomationId="f" Content="F" IsEnabled="{x:Bind weft:Weft.Hide(ViewModel.x), Mode=OneWay}" />"#,
        r#"<StackPanel AutomationProperties.AutomationId="g" Orientation="{x:Bind ViewModel.o, Mode=OneWay}" />"#,
        r#"<CheckBox AutomationProperties.AutomationId="h" Content="H" weft:Props.Shade="{x:Bind ViewModel.shade, Mode=OneWay}" />"#,
    ));
    assert_eq!(
        doc,
        concat!(
            r#"<text id="a"/>"#,
            "\n",
            r#"<text id="b"/>"#,
            "\n",
            r#"<text id="c"/>"#,
            "\n",
            r#"<text id="d"/>"#,
            "\n",
            r#"<text id="i"/>"#,
            "\n",
            r#"<text id="e"/>"#,
            "\n",
            r#"<button id="f">F</button>"#,
            "\n",
            r#"<stack id="g"/>"#,
            "\n",
            r#"<checkbox id="h" label="H"/>"#,
            "\n",
        )
    );
    assert_eq!(
        lost,
        concat!(
            "Bindings /screen#s/text#a: `Text` on `TextBlock`: a binding\n",
            "Bindings /screen#s/text#b: `Text` on `TextBlock`: `title` is not a path of the page's view model\n",
            "Bindings /screen#s/text#c: `Text` on `TextBlock`: `Converter` changes what the binding reads\n",
            "Bindings /screen#s/text#d: `Text` on `TextBlock`: `sys:Weft.Not` is not a `Weft` helper\n",
            "Bindings /screen#s/text#i: `Text`: `text` takes no negated binding\n",
            "Bindings /screen#s/text#e: `Text` on `TextBlock`: `ViewModel.a[0]b` is not a Weft data path\n",
            "Bindings /screen#s/button#f: `IsEnabled` on `Button`: a binding with no Weft form\n",
            "Bindings /screen#s/stack#g: `Orientation` on `StackPanel`: a binding with no Weft form\n",
            "Bindings /screen#s/checkbox#h: `Props.Shade`: a `checkbox` has no `shade`\n",
        )
    );
}

#[test]
fn on_helpers_commands_and_bound_events_are_actions() {
    let (doc, lost) = read_body(concat!(
        r#"<StackPanel AutomationProperties.AutomationId="f" AutomationProperties.LandmarkType="Form" weft:On.Submit="{x:Bind ViewModel.login_sendCommand}">"#,
        r#"<TextBox AutomationProperties.AutomationId="name" Header="Name" TextChanged="{x:Bind ViewModel.form_edit}" />"#,
        r#"<Button AutomationProperties.AutomationId="go" Content="Go" weft:On.Submit="{x:Bind ViewModel.login_sendCommand}" weft:On.Press="{x:Bind ViewModel.login_trackCommand}" />"#,
        r#"</StackPanel>"#,
        r#"<Button AutomationProperties.AutomationId="clear" Content="Clear" Command="{x:Bind ViewModel.cart_clearCommand}" />"#,
        r#"<Button AutomationProperties.AutomationId="pay" Content="Pay" Click="{x:Bind ViewModel.order_pay}" />"#,
        r#"<CheckBox AutomationProperties.AutomationId="terms" Content="Terms" Click="{x:Bind ViewModel.terms_toggle}" />"#,
        r#"<ToggleSwitch AutomationProperties.AutomationId="push" Header="Push" Toggled="{x:Bind ViewModel.prefs_save}" />"#,
    ));
    assert_eq!(
        doc,
        concat!(
            r#"<form id="f" on-submit="login.send">"#,
            "\n",
            r#"  <field id="name" label="Name" on-change="form.edit"/>"#,
            "\n",
            r#"  <button id="go" submit="true" on-press="login.track">Go</button>"#,
            "\n",
            r#"</form>"#,
            "\n",
            r#"<button id="clear" on-press="cart.clear">Clear</button>"#,
            "\n",
            r#"<button id="pay" on-press="order.pay">Pay</button>"#,
            "\n",
            r#"<checkbox id="terms" label="Terms" on-change="terms.toggle"/>"#,
            "\n",
            r#"<switch id="push" label="Push" on-change="prefs.save"/>"#,
            "\n",
        )
    );
    assert_eq!(lost, "");
}

#[test]
fn events_that_name_no_weft_action_are_losses() {
    let (doc, lost) = read_body(concat!(
        r#"<Button AutomationProperties.AutomationId="a" Content="A" Click="OnA" />"#,
        r#"<Button AutomationProperties.AutomationId="b" Content="B" weft:On.Change="{x:Bind ViewModel.b_setCommand}" />"#,
        r#"<Button AutomationProperties.AutomationId="c" Content="C" Command="{x:Bind ViewModel.Pay}" />"#,
        r#"<Button AutomationProperties.AutomationId="d" Content="D" weft:On.Press="{x:Bind ViewModel.d_oneCommand}" Command="{x:Bind ViewModel.d_twoCommand}" />"#,
        r#"<Button AutomationProperties.AutomationId="e" Content="E" Command="{x:Bind weft:Weft.Not(ViewModel.eCommand)}" />"#,
    ));
    assert_eq!(
        doc,
        concat!(
            r#"<button id="a">A</button>"#,
            "\n",
            r#"<button id="b">B</button>"#,
            "\n",
            r#"<button id="c">C</button>"#,
            "\n",
            r#"<button id="d" on-press="d.one">D</button>"#,
            "\n",
            r#"<button id="e">E</button>"#,
            "\n",
        )
    );
    assert_eq!(
        lost,
        concat!(
            "Actions /screen#s/button#a: `Click` on `Button`: an event handler\n",
            "Actions /screen#s/button#b: `On.Change` on `Button`: a `button` has no such event\n",
            "Actions /screen#s/button#c: `Command` on `Button`: not a command of the view model named after a Weft action\n",
            "Actions /screen#s/button#d: `Command` on `Button`: `on-press` already calls `d.one`\n",
            "Actions /screen#s/button#e: `Command` on `Button`: not a command of the view model named after a Weft action\n",
        )
    );
}

#[test]
fn weft_resources_are_tokens() {
    let (doc, lost) = read_body(concat!(
        r#"<StackPanel AutomationProperties.AutomationId="a" Spacing="{StaticResource Weft.space.md}" Padding="{StaticResource Weft.space.lg.thickness}" MaxWidth="{StaticResource Weft.size.lg}" HorizontalAlignment="Stretch" />"#,
        r#"<StackPanel AutomationProperties.AutomationId="b" Spacing="{StaticResource Weft.space.huge}" Padding="{StaticResource Weft.space.lg}" MaxWidth="{ThemeResource Weft.color.red}" />"#,
        r#"<StackPanel AutomationProperties.AutomationId="c" Spacing="12" Padding="{StaticResource Gap}" />"#,
        r#"<Grid AutomationProperties.AutomationId="d" RowSpacing="{StaticResource Weft.space.sm}" ColumnSpacing="{StaticResource Weft.space.md}"><Grid.ColumnDefinitions><ColumnDefinition /><ColumnDefinition /></Grid.ColumnDefinitions></Grid>"#,
    ));
    assert_eq!(
        doc,
        concat!(
            r#"<stack id="a" gap="{token.space.md}" max-width="{token.size.lg}" padding="{token.space.lg}"/>"#,
            "\n",
            r#"<stack id="b"/>"#,
            "\n",
            r#"<stack id="c"/>"#,
            "\n",
            r#"<grid id="d" columns="2" gap="{token.space.sm}"/>"#,
            "\n",
        )
    );
    assert_eq!(
        lost,
        concat!(
            "Tokens /screen#s/stack#b: `Spacing`: `space.huge` is not a token of the project\n",
            "Tokens /screen#s/stack#b: `Padding` on `StackPanel`: a token reference\n",
            "Tokens /screen#s/stack#b: `MaxWidth`: the color token `color.red` does not fit `max-width`\n",
            "Layout /screen#s/stack#c: `Spacing` on `StackPanel`: a layout value\n",
            "Props /screen#s/stack#c: `Padding` on `StackPanel`: a markup extension\n",
            "Tokens /screen#s/grid#d: `ColumnSpacing` disagrees with the `gap` already read\n",
        )
    );
}

/// A `Grid` with `tracks` on one axis around `cells`.
fn tracks(axis: &str, tracks: &[&str], cells: &str) -> String {
    let (defs, def, size) = match axis {
        "row" => ("ColumnDefinitions", "ColumnDefinition", "Width"),
        _ => ("RowDefinitions", "RowDefinition", "Height"),
    };
    let tracks: String = tracks
        .iter()
        .map(|t| format!(r#"<{def} {size}="{t}" />"#))
        .collect();
    format!(
        r#"<Grid AutomationProperties.AutomationId="g"><Grid.{defs}>{tracks}</Grid.{defs}>{cells}</Grid>"#
    )
}

fn button(id: &str, place: &str) -> String {
    format!(r#"<Button AutomationProperties.AutomationId="{id}" Content="{id}" {place} />"#)
}

#[test]
fn grid_tracks_are_justify_and_grow() {
    let a = |t: usize| button("a", &format!(r#"Grid.Column="{t}""#));
    let b = |t: usize| button("b", &format!(r#"Grid.Column="{t}""#));
    for (spec, cells, want) in [
        (vec!["Auto", "Auto"], a(0) + &b(1), r#"direction="row""#),
        (
            vec!["*", "Auto", "Auto"],
            b(2) + &a(1),
            r#"direction="row" justify="end""#,
        ),
        (
            vec!["*", "Auto", "Auto", "*"],
            a(1) + &b(2),
            r#"direction="row" justify="center""#,
        ),
        (
            vec!["Auto", "*", "Auto"],
            a(0) + &b(2),
            r#"direction="row" justify="space-between""#,
        ),
    ] {
        let (doc, lost) = read_body(&tracks("row", &spec, &cells));
        assert_eq!(
            doc,
            format!(
                "<stack id=\"g\" {want}>\n  <button id=\"a\">a</button>\n  <button id=\"b\">b</button>\n</stack>\n"
            ),
            "{spec:?}"
        );
        assert_eq!(lost, "", "{spec:?}");
    }
    // A star track holds a growing child; rows are a column.
    let cells = button("a", "") + &button("b", r#"Grid.Row="1""#);
    let (doc, lost) = read_body(&tracks("column", &["Auto", "*"], &cells));
    assert_eq!(
        doc,
        "<stack id=\"g\" direction=\"column\">\n  <button id=\"a\">a</button>\n  <button id=\"b\" grow=\"true\">b</button>\n</stack>\n"
    );
    assert_eq!(lost, "");
    // Free space beside a growing child, or an empty `Auto` track, is no Weft layout.
    for spec in [vec!["*", "*", "Auto"], vec!["Auto", "Auto", "Auto"]] {
        let cells = button("a", r#"Grid.Column="1""#) + &button("b", r#"Grid.Column="2""#);
        let (_, lost) = read_body(&tracks("row", &spec, &cells));
        assert!(
            lost.starts_with("Layout /screen#s/stack#g: the tracks"),
            "{spec:?}: {lost}"
        );
    }
    // All-star columns, a shared track or a second row: a `grid`.
    for (spec, cells) in [
        (vec!["*", "*"], a(0) + &b(1)),
        (vec!["Auto", "*"], a(0) + &b(0)),
        (vec!["Auto", "*"], a(0) + &button("b", r#"Grid.Row="1""#)),
    ] {
        let (doc, _) = read_body(&tracks("row", &spec, &cells));
        assert!(
            doc.starts_with("<grid id=\"g\" columns=\"2\">"),
            "{spec:?}: {doc}"
        );
    }
}

#[test]
fn a_grid_filled_in_order_reads_as_its_columns_alone() {
    let rows = |n: usize| "<RowDefinition Height=\"Auto\" />".repeat(n);
    let grid = |id: &str, columns: &str, rows: &str, cells: &str| {
        format!(
            r#"<Grid AutomationProperties.AutomationId="{id}"><Grid.ColumnDefinitions>{columns}</Grid.ColumnDefinitions><Grid.RowDefinitions>{rows}</Grid.RowDefinitions>{cells}</Grid>"#
        )
    };
    let star = "<ColumnDefinition Width=\"*\" /><ColumnDefinition />";
    let filled = grid(
        "f",
        star,
        &rows(2),
        concat!(
            r#"<TextBlock AutomationProperties.AutomationId="a" Text="A" />"#,
            r#"<TextBlock AutomationProperties.AutomationId="b" Text="B" Grid.Column="1" />"#,
            r#"<TextBlock AutomationProperties.AutomationId="c" Text="C" Grid.Row="1" />"#,
        ),
    );
    // Out of fill order, a wide column and a spare row each keep the tracks a loss.
    let swapped = grid(
        "o",
        star,
        &rows(1),
        concat!(
            r#"<TextBlock AutomationProperties.AutomationId="d" Text="D" Grid.Column="1" />"#,
            r#"<TextBlock AutomationProperties.AutomationId="e" Text="E" />"#,
        ),
    );
    let wide = grid(
        "w",
        "<ColumnDefinition Width=\"2*\" />",
        &rows(1),
        r#"<TextBlock AutomationProperties.AutomationId="g" Text="G" />"#,
    );
    let spare = grid(
        "p",
        star,
        &rows(2),
        r#"<TextBlock AutomationProperties.AutomationId="h" Text="H" />"#,
    );
    let (doc, lost) = read_body(&format!("{filled}{swapped}{wide}{spare}"));
    assert!(doc.starts_with(r#"<grid id="f" columns="2">"#), "{doc}");
    assert!(!lost.contains("grid#f"), "{lost}");
    for id in ["o", "w", "p"] {
        let tracks = format!("/grid#{id}: `Grid.RowDefinitions`: the track sizes are not read");
        assert!(lost.contains(&tracks), "{id}: {lost}");
    }
    assert!(
        lost.contains("text#d: `Grid.Column` on `TextBlock`"),
        "{lost}"
    );
}

#[test]
fn shared_cross_axis_alignment_is_align() {
    let row = |v: &str, w: &str| {
        format!(
            r#"<StackPanel AutomationProperties.AutomationId="r" Orientation="Horizontal"><Button AutomationProperties.AutomationId="a" Content="a" VerticalAlignment="{v}" /><Button AutomationProperties.AutomationId="b" Content="b" VerticalAlignment="{w}" /></StackPanel>"#
        )
    };
    let start = r#"<stack id="r" align="start" direction="row">"#;
    let plain = r#"<stack id="r" direction="row">"#;
    for (v, w, head) in [
        ("Top", "Top", start),
        ("Center", "Center", plain),
        (
            "Stretch",
            "Stretch",
            r#"<stack id="r" align="stretch" direction="row">"#,
        ),
    ] {
        let (doc, lost) = read_body(&row(v, w));
        assert!(doc.starts_with(head), "{v}: {doc}");
        assert_eq!(lost, "", "{v}");
    }
    // Children that disagree keep their own alignment, which is layout.
    let (doc, lost) = read_body(&row("Top", "Bottom"));
    assert!(doc.starts_with(plain), "{doc}");
    assert_eq!(
        lost,
        concat!(
            "Layout /screen#s/stack#r/button#a: `VerticalAlignment` on `Button`: a layout value\n",
            "Layout /screen#s/stack#r/button#b: `VerticalAlignment` on `Button`: a layout value\n",
        )
    );
    // A column reads `HorizontalAlignment`; a grid stack reads its children's too.
    let column = r#"<StackPanel AutomationProperties.AutomationId="c"><TextBlock AutomationProperties.AutomationId="t" Text="t" HorizontalAlignment="Right" /></StackPanel>"#;
    let (doc, lost) = read_body(column);
    assert!(doc.starts_with(r#"<stack id="c" align="end">"#), "{doc}");
    assert_eq!(lost, "");
    let cells = button("a", r#"Grid.Column="1" VerticalAlignment="Bottom""#);
    let (doc, lost) = read_body(&tracks("row", &["*", "Auto"], &cells));
    assert!(
        doc.starts_with(r#"<stack id="g" align="end" direction="row" justify="end">"#),
        "{doc}"
    );
    assert_eq!(lost, "");
}

#[test]
fn a_hand_written_page_reports_each_loss_at_its_path() {
    let (catalog, tokens) = setup();
    let source = std::fs::read_to_string(here("tests/fixtures/handwritten.xaml")).unwrap();
    let result = read(&source, &catalog, &tokens);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    lenient(&result, &catalog);
    golden("handwritten.read.weft", &serialize(&result.document));
    golden("handwritten.read.losses", &losses(&result));
    for kind in [
        LossKind::Ids,
        LossKind::Bindings,
        LossKind::Actions,
        LossKind::Tokens,
        LossKind::Layout,
        LossKind::Repetition,
        LossKind::Slots,
        LossKind::Props,
        LossKind::Values,
        LossKind::Kinds,
        LossKind::Text,
        LossKind::Structure,
    ] {
        assert!(result.losses.iter().any(|l| l.kind == kind), "{kind:?}");
    }
}

#[test]
fn ids_come_from_the_page_and_never_collide() {
    let (catalog, tokens) = setup();
    let page = |body: &str| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"><StackPanel AutomationProperties.AutomationId="s">{body}</StackPanel></Page>"#
        )
    };
    // A generated id avoids an id a later element states, which keeps it.
    let result = read(
        &page(r#"<Button Content="Go" /><Button x:Name="button-go" Content="Go" />"#),
        &catalog,
        &tokens,
    );
    let ids: Vec<_> = result
        .document
        .root
        .children
        .iter()
        .map(|c| match c {
            Child::Node(n) => n.id.clone().unwrap(),
            Child::Text(_) => panic!("text"),
        })
        .collect();
    assert_eq!(ids, ["button-go-2", "button-go"]);
    assert_eq!(result.document.root.id.as_deref(), Some("s"));
    let generated: Vec<_> = result
        .losses
        .iter()
        .map(|l| (l.kind, &l.path[..]))
        .collect();
    assert_eq!(generated, [(LossKind::Ids, "/screen#s/button#button-go-2")]);
    // A loose panel is the screen; a horizontal one sits in a generated screen.
    let loose = r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Orientation="Horizontal" />"#;
    let result = read(loose, &catalog, &tokens);
    assert_eq!(
        serialize(&result.document),
        "<screen id=\"screen-1\" weft=\"0.3\">\n  <stack id=\"stack-1\" direction=\"row\"/>\n</screen>\n"
    );
}

fn diagnostic(result: &ImportResult) -> Code {
    assert_eq!(result.diagnostics.len(), 1, "{:?}", result.diagnostics);
    result.diagnostics[0].code
}

#[test]
fn unreadable_input_is_w601() {
    let (catalog, tokens) = setup();
    let ns = r#"xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation""#;
    let dtd = format!(
        "<!DOCTYPE Page [<!ENTITY lol \"lol\">]>\n<Page {ns}><TextBlock Text=\"&lol;\" /></Page>"
    );
    for source in [
        dtd.as_str(),
        "<Page",
        "not xml",
        "<html><body>Hi</body></html>",
    ] {
        let result = read(source, &catalog, &tokens);
        assert_eq!(diagnostic(&result), Code::W601, "{source}");
        assert_eq!(result.document.root.children, vec![]);
    }
}

#[test]
fn oversized_input_is_cut_with_w602() {
    let (catalog, tokens) = setup();
    let page = |body: String| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><StackPanel>{body}</StackPanel></Page>"#
        )
    };
    // Too long, or too many XML nodes: nothing is read.
    let long = page(" ".repeat(MAX_SOURCE_LENGTH));
    let result = read(&long, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children, vec![]);
    let nodes = page("<!---->".repeat(weft_import::MAX_NODES * 8));
    assert!(nodes.len() < MAX_SOURCE_LENGTH);
    assert_eq!(diagnostic(&read(&nodes, &catalog, &tokens)), Code::W602);
    // Too many elements, or nested too deep: what fits is read.
    let many = page("<TextBlock/>".repeat(weft_import::MAX_NODES + 5));
    let result = read(&many, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children.len(), weft_import::MAX_NODES);
}

#[test]
fn deep_input_is_cut_with_w602() {
    let (catalog, tokens) = setup();
    let page = |body: String| {
        format!(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"><StackPanel>{body}</StackPanel></Page>"#
        )
    };
    // `Page` and the outer panel, then as deep as the parser is given: read, and cut.
    let depth = MAX_XML_DEPTH - 2;
    let deep = page(format!(
        "{}{}",
        "<StackPanel>".repeat(depth),
        "</StackPanel>".repeat(depth)
    ));
    let result = read(&deep, &catalog, &tokens);
    assert_eq!(diagnostic(&result), Code::W602);
    assert_eq!(result.document.root.children.len(), 1);
    // Validating 200 levels takes more than a debug test thread's stack.
    weft_import::on_reader_stack("lenient", || lenient(&result, &catalog)).unwrap();
    // Deeper than the parser is given: refused before parsing, wherever the nesting hides.
    for (open, close) in [
        ("<StackPanel>", "</StackPanel>"),
        (
            "<a:b xmlns:a='x' c='>'><!-- <d> --><![CDATA[<e>]]><f/>",
            "</a:b>",
        ),
    ] {
        let deep = page(format!(
            "{}{}",
            open.repeat(MAX_XML_DEPTH),
            close.repeat(MAX_XML_DEPTH)
        ));
        let result = read(&deep, &catalog, &tokens);
        assert_eq!(diagnostic(&result), Code::W602);
        assert_eq!(result.document.root.children, vec![]);
    }
}
