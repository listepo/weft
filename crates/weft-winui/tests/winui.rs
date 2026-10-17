//! Weft → WinUI over the corpus screens and catalog examples the generator covers so far, a
//! fixture of every prop it draws and one of the data shapes a view model holds: the XAML, the
//! code-behind, the view model and the shared `Weft.cs` match their reviewed golden files, and
//! what the generator cannot express is refused with its path. `WEFT_UPDATE_FIXTURES=1` rewrites
//! the golden files.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{
    KEYWORDS_BODY, NAMESPACE, PENDING, controls, covered, document, here, options, screen, screens,
    setup,
};
use weft_winui::{
    GenerateError, GenerateOptions, Generated, TOKENS_FILE, generate, generate_tokens, helpers,
};

#[test]
fn every_screen_but_the_pending_ones_generates() {
    let (catalog, tokens) = setup();
    let (mut generated, mut refused) = (vec![], vec![]);
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(path).unwrap(), &catalog, &tokens);
        match generate(&doc, &options(&catalog, &tokens, None)) {
            Ok(_) => generated.push(name),
            Err(GenerateError::Unsupported(problems)) => {
                assert!(!problems.is_empty(), "{name}");
                refused.push(name);
            }
            Err(e) => panic!("{name}: {e}"),
        }
    }
    assert_eq!(refused, PENDING);
    assert!(!generated.is_empty());
}

/// The Windows build (T105.7) compiles every `.xaml` and `.cs` file of `tests/fixtures/`, so the
/// set must be exactly the goldens of the screens that generate and the shared files: a screen
/// without its goldens would go unbuilt, a stale golden would be built for nothing.
#[test]
fn every_screen_that_generates_has_its_golden_files_and_no_other() {
    let mut expected: Vec<String> = covered()
        .iter()
        .flat_map(|(_, _, generated)| generated.files().map(|(file, _)| file))
        .chain(["Weft.cs".to_owned(), TOKENS_FILE.to_owned()])
        .collect();
    expected.sort();
    let mut found: Vec<String> = std::fs::read_dir(here("tests/fixtures"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        // The read-back's hand-written input, not generator output; the build leaves it out.
        .filter(|f| (f.ends_with(".xaml") || f.ends_with(".cs")) && f != "handwritten.xaml")
        .collect();
    found.sort();
    assert_eq!(found, expected);
}

fn golden(file: &str, text: &str) {
    let golden = here(&format!("tests/fixtures/{file}"));
    if std::env::var_os("WEFT_UPDATE_FIXTURES").is_some() {
        std::fs::write(&golden, text).unwrap();
    }
    let expected = std::fs::read_to_string(&golden).unwrap_or_default();
    assert_eq!(text, expected, "{file}");
}

/// Every `{` of a C# file is closed in order, outside string literals: a cheap guard where the
/// Windows build (T105.7) cannot run.
fn balanced(file: &str, source: &str) {
    let mut depth = 0i32;
    let mut quoted = false;
    for c in source.chars() {
        match c {
            '"' => quoted = !quoted,
            '{' if !quoted => depth += 1,
            '}' if !quoted => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0, "{file}");
    }
    assert_eq!((depth, quoted), (0, false), "{file}");
}

#[test]
fn generated_files_match_the_golden_files() {
    for (name, _, generated) in covered() {
        let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
        assert_eq!(xml.root_element().tag_name().name(), "Page", "{name}");
        for (file, text) in generated.files() {
            if file.ends_with(".cs") {
                balanced(&file, text);
            }
            golden(&file, text);
        }
    }
    let shared = helpers(NAMESPACE).unwrap();
    balanced("Weft.cs", &shared);
    golden("Weft.cs", &shared);
    let (_, tokens) = setup();
    golden(TOKENS_FILE, &generate_tokens(&tokens, None).0);
}

fn data() -> Generated {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("tests/fixtures/data.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    generate(&doc, &options(&catalog, &tokens, None)).unwrap()
}

/// The body of class `name` in a generated C# file.
fn class<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("public sealed partial class {name} "))
        .unwrap_or_else(|| panic!("{name}"));
    let end = source[start..].find("\n}\n").unwrap();
    &source[start..start + end]
}

#[test]
fn paths_become_properties_objects_and_lists() {
    let generated = data();
    let vm = &generated.view_model_source;
    assert_eq!(generated.view_model, "DataViewModel");
    let root = class(vm, "DataViewModel");
    for line in [
        "public string title",
        "public ObservableCollection<DataTodosItem> todos { get; } = new();",
        "public ObservableCollection<string> labels { get; } = new();",
        "public ObservableCollection<bool> flags { get; } = new();",
        "public DataUserData user { get; } = new();",
        "public string kind",
        "public DataAlertData alert { get; } = new();",
        // Read only through `Weft.Not`, a flag.
        "public bool busy",
        "Weft.Place(user.pins, this, null, \"$.user.pins\");",
    ] {
        assert!(root.contains(line), "{line}\n{root}");
    }
    let todo = class(vm, "DataTodosItem");
    for line in [
        "INotifyPropertyChanged, IWeftItem",
        "public DataViewModel? WeftRoot { get; private set; }",
        "public object? WeftParent { get; private set; }",
        "public string WeftPath { get; private set; }",
        "public bool done",
        "public DataTodosOwnerData owner { get; } = new();",
        "Weft.Place(tags, WeftRoot, this, WeftPath + \".tags\");",
    ] {
        assert!(todo.contains(line), "{line}\n{todo}");
    }
    let tag = class(vm, "DataTodosTagsItem");
    assert!(tag.contains("public DataTodosItem? WeftParent"), "{tag}");
    assert!(tag.contains("public bool shown"), "{tag}");
    let away = class(vm, "DataUserData");
    assert!(away.contains("public bool away"), "{away}");
    assert!(!away.contains("WeftRoot"), "{away}");
    for xaml in [
        "Text=\"{x:Bind ViewModel.todos[0].title, Mode=OneWay}\"",
        "Visibility=\"{x:Bind ViewModel.todos[0].tags[0].shown, Mode=OneWay}\"",
        "Text=\"{x:Bind ViewModel.kind, Mode=TwoWay, UpdateSourceTrigger=PropertyChanged}\"",
    ] {
        assert!(generated.xaml.contains(xaml), "{xaml}");
    }
}

#[test]
fn each_action_is_one_command_that_performs_it() {
    let vm = data().view_model_source;
    let root = class(&vm, "DataViewModel");
    assert_eq!(
        root.matches("new WeftCommand(\"todo.add\", Perform)")
            .count(),
        1
    );
    assert!(root.contains("public ICommand todo_toggleCommand { get; }"));
    assert!(root.contains("public event EventHandler<WeftEvent>? Performed;"));
    assert!(
        root.contains("public void Perform(string action, string? id = null, string? item = null)")
    );
    assert!(root.contains("partial void OnPerformed(WeftEvent e);"));
}

#[test]
fn the_code_behind_owns_the_view_model() {
    let generated = data();
    let files = generated.files();
    let names: Vec<_> = files.iter().map(|(f, _)| f.as_str()).collect();
    assert_eq!(
        names,
        ["DataPage.xaml", "DataPage.xaml.cs", "DataViewModel.cs"]
    );
    let code = &generated.code_behind;
    assert!(code.contains("namespace WeftScreens;"));
    assert!(code.contains("public sealed partial class DataPage : Page"));
    assert!(code.contains("public DataViewModel ViewModel { get; } = new();"));
    assert!(code.contains("InitializeComponent();"));
}

#[test]
fn helpers_take_the_namespace() {
    let shared = helpers("Acme.Screens").unwrap();
    assert!(shared.contains("\nnamespace Acme.Screens;\n"));
    assert!(!shared.contains("WeftNamespace"));
    for function in [
        "Uri? Link(object? value)",
        "ImageSource? Image(object? value)",
        "DateTimeOffset? Day(string? text)",
        "string DayText(DateTimeOffset day)",
        "TimeSpan? Clock(string? text)",
        "string ClockText(TimeSpan time)",
        "WinColor Color(string? text)",
        "string Hex(WinColor color)",
        // x:Bind passes a quoted literal as a string and converts nothing: with an `object`
        // parameter the XAML compiler fails (WMC9999) on `Weft.Earliest('2026-01-01')`.
        "DateTimeOffset Earliest(string? value)",
        "DateTimeOffset Latest(string? value)",
        "class Choice",
        "class Dialog",
        "class ColumnsPanel : Panel",
        // SPEC §9 Trust: a bound address opens or loads only these schemes.
        "uri.Scheme is \"http\" or \"https\" or \"mailto\"",
        "\"ms-appx:///\"",
        // Enter in a single-line field submits its form.
        "e.Key == VirtualKey.Enter && field",
    ] {
        assert!(shared.contains(function), "{function}");
    }
    for name in ["Not", "Truthy", "Hide"] {
        let function = format!(" {name}(object? value) =>");
        assert!(shared.contains(&function), "{name}");
    }
    for event in ["Press", "Change", "Submit", "Close"] {
        assert!(
            shared.contains(&format!("{event}Property = Event(\"{event}\")")),
            "{event}"
        );
    }
    assert!(matches!(
        helpers("Acme Screens"),
        Err(GenerateError::Namespace(_))
    ));
}

#[test]
fn the_page_is_named_from_the_option_or_the_screen_id() {
    let (catalog, tokens) = setup();
    let doc = controls(&catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    assert_eq!(generated.page, "ControlsPage");
    assert!(
        generated
            .xaml
            .contains("x:Class=\"WeftScreens.ControlsPage\"")
    );
    let named = generate(&doc, &options(&catalog, &tokens, Some("sign-in"))).unwrap();
    assert_eq!(named.page, "SignInPage");
    assert!(
        named
            .xaml
            .contains("<!-- weft:source winui name=SignIn namespace=WeftScreens\n")
    );
}

#[test]
fn hostile_text_stays_text() {
    let (catalog, tokens) = setup();
    let markup =
        screen(r#"<text id="t">{x:Bind ViewModel.secret} -- --> &lt;!-- "q" &amp;</text>"#);
    let doc = document(&markup, &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
    let text = xml
        .descendants()
        .find(|n| n.has_tag_name("TextBlock"))
        .and_then(|n| n.attribute("Text"))
        .unwrap();
    assert_eq!(text, r#"{}{x:Bind ViewModel.secret} -- --> <!-- "q" &"#);
    let comments: Vec<_> = xml.descendants().filter(|n| n.is_comment()).collect();
    assert_eq!(comments.len(), 1);
}

/// The paths of the problems `generate` reports for `markup`.
fn refused(markup: &str) -> Vec<String> {
    let (catalog, tokens) = setup();
    let doc = document(markup, &catalog, &tokens);
    match generate(&doc, &options(&catalog, &tokens, None)) {
        Err(GenerateError::Unsupported(problems)) => problems.into_iter().map(|p| p.path).collect(),
        other => panic!("{markup}: {other:?}"),
    }
}

#[test]
fn what_the_slice_cannot_draw_is_refused_with_its_path() {
    let cases = [
        (
            r#"<date-picker id="d" label="D" type="{$.type}"/>"#,
            "date-picker#d",
        ),
        (r#"<alert id="a" tone="{$.tone}">A</alert>"#, "alert#a"),
        // A choice or a menu holds one template, of one item.
        (
            r#"<select id="sel" label="S" value="{$.v}"><each id="e" as="o" in="{$.os}"><option id="o" value="{$o.v}">O</option></each><each id="f" as="p" in="{$.ps}"><option id="p" value="{$p.v}">P</option></each></select>"#,
            "each#f",
        ),
        (
            r#"<menu id="m" label="M"><each id="e" as="a" in="{$.as}"><menu-item id="i" text="{$a.t}"/><menu-item id="j" text="{$a.u}"/></each></menu>"#,
            "each#e",
        ),
        (
            r#"<radio-group id="rg" label="R"><each id="e" as="o" in="{$.os}"><radio id="o" value="{$o.v}">O</radio></each></radio-group>"#,
            "each#e",
        ),
        // Star columns alone are a `grid`'s tracks.
        (
            r#"<stack id="r" direction="row"><text id="a" grow="true">A</text></stack>"#,
            "stack#r",
        ),
        // A control holds its items as its own type, so no tilt wrapper fits around one.
        (
            r#"<select id="se" label="S"><option id="o" value="a" rotate-y="10">A</option></select>"#,
            "option#o",
        ),
        (
            r#"<stack id="r"><each id="e" as="o" in="{$.os}"><text id="t" grow="true" text="{$o.v}"/></each></stack>"#,
            "each#e",
        ),
        // A repeater cannot share a justified stack's tracks.
        (
            r#"<stack id="r" justify="end"><each id="e" as="o" in="{$.os}"><text id="t" text="{$o.v}"/></each></stack>"#,
            "each#e",
        ),
        (r#"<heading id="h" level="{$.n}">A</heading>"#, "heading#h"),
        // Each column is a `ColumnDefinition`, so their number is a bounded literal.
        (
            r#"<grid id="g" columns="{$.n}"><text id="a">A</text></grid>"#,
            "grid#g",
        ),
        (
            r#"<grid id="g" columns="101"><text id="a">A</text></grid>"#,
            "grid#g",
        ),
        // A repeater cannot share a grid's tracks, or sit among a list's static items or columns.
        (
            r#"<grid id="g" columns="2"><each id="e" as="x" in="{$.xs}"><text id="t" text="{$x.a}"/></each></grid>"#,
            "each#e",
        ),
        (
            r#"<list id="l" label="L"><item id="i">A</item><each id="e" as="x" in="{$.xs}"><item id="j" text="{$x.a}"/></each></list>"#,
            "each#e",
        ),
        (
            r#"<table id="tb" label="T"><each id="e" as="x" in="{$.xs}"><column id="c" text="{$x.a}"/></each></table>"#,
            "each#e",
        ),
        // A loop's item is an object, and `WeftParent` reaches only the item its list is under.
        (
            r#"<stack id="st"><each id="e" as="x" in="{$.xs}"><text id="t" text="{$x}"/></each></stack>"#,
            "text#t",
        ),
        (
            r#"<stack id="st"><each id="e" as="x" in="{$.xs}"><text id="t" text="{$x.0}"/></each></stack>"#,
            "$x.0",
        ),
        (
            r#"<stack id="st"><each id="e" as="x" in="{$.xs}"><each id="f" as="y" in="{$.ys}"><text id="t" text="{$x.a}"/></each></each></stack>"#,
            "text#t",
        ),
        (r#"<button id="b" variant="{$.v}">A</button>"#, "button#b"),
        (r#"<text id="t" tone="{$.tone}">A</text>"#, "text#t"),
        (r#"<field id="f" label="F" type="{$.type}"/>"#, "field#f"),
        (r#"<text id="t" text="{$.0.name}"/>"#, "$.0.name"),
        (r#"<text id="t" text="{$.a.ToString}"/>"#, "$.a.ToString"),
        (r#"<text id="t" text="{$.Performed}"/>"#, "$.Performed"),
        (
            r#"<text id="t" text="{$.user.WeftPath}"/>"#,
            "$.user.WeftPath",
        ),
        (
            r#"<stack id="r" padding="{$.p}"><text id="a">A</text></stack>"#,
            "stack#r",
        ),
        (
            r#"<stack id="r" max-width="{$.w}"><text id="a">A</text></stack>"#,
            "stack#r",
        ),
        (
            r#"<grid id="g" columns="2" min-column-width="{$.w}"><text id="a">A</text></grid>"#,
            "grid#g",
        ),
        (
            r#"<stack id="r" gap="{$.g}"><text id="a">A</text></stack>"#,
            "stack#r",
        ),
        // A list's template draws one item.
        (
            r#"<list id="l" label="L"><each id="e" as="x" in="{$.xs}"><item id="i" text="{$x.a}"/><item id="j" text="{$x.b}"/></each></list>"#,
            "each#e",
        ),
        // One `Count` decides whether the slot shows.
        (
            r#"<table id="tb" label="T"><column id="c">C</column><each id="e" as="x" in="{$.xs}"><row id="r"><cell id="c1" text="{$x.a}"/></row></each><each id="f" as="y" in="{$.ys}"><row id="r2"><cell id="c2" text="{$y.a}"/></row></each><slot name="empty"><text id="none">None</text></slot></table>"#,
            "table#tb",
        ),
    ];
    for (body, path) in cases {
        assert_eq!(refused(&screen(body)), [path], "{body}");
    }
    let two_types =
        screen(r#"<checkbox id="c" checked="{$.x}" label="C"/><text id="t" text="{$.x}"/>"#);
    assert_eq!(refused(&two_types), ["text#t"]);
    // `hidden="{!$.x}"` binds `Visibility` to the path itself, so the path is a flag.
    let shown_when = screen(r#"<text id="t" hidden="{!$.x}" text="{$.x}"/>"#);
    assert_eq!(refused(&shown_when), ["text#t"]);
    let object = screen(r#"<text id="a" text="{$.user}"/><text id="b" text="{$.user.name}"/>"#);
    assert_eq!(refused(&object), ["text#a"]);
    let command =
        screen(r#"<button id="b" on-press="go">Go</button><text id="t" text="{$.goCommand}"/>"#);
    assert_eq!(refused(&command), ["text#t"]);
}

fn values() -> Generated {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("tests/fixtures/values.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    generate(&doc, &options(&catalog, &tokens, None)).unwrap()
}

/// The element of `xaml` whose `AutomationId` is `id`.
fn by_id<'a>(xml: &'a roxmltree::Document<'a>, id: &str) -> roxmltree::Node<'a, 'a> {
    xml.descendants()
        .find(|n| n.attribute("AutomationProperties.AutomationId") == Some(id))
        .unwrap_or_else(|| panic!("{id}"))
}

#[test]
fn value_controls_follow_the_spec() {
    let generated = values();
    let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
    // `weft:` attributes are in the namespace of the page's helpers.
    let at = |id: &str, attr: &str| {
        let node = by_id(&xml, id);
        match attr.strip_prefix("weft:") {
            Some(name) => node.attribute(("using:WeftScreens", name)),
            None => node.attribute(attr),
        }
        .map(str::to_owned)
    };
    // Every choice shows its text through `Choice.Value`, two-way when bound.
    for (id, element) in [
        ("size", "ComboBox"),
        ("city", "ComboBox"),
        ("view", "SelectorBar"),
        ("day", "CalendarDatePicker"),
        ("alarm", "TimePicker"),
        ("accent", "DropDownButton"),
    ] {
        assert_eq!(by_id(&xml, id).tag_name().name(), element, "{id}");
        let value = at(id, "weft:Choice.Value").unwrap();
        assert!(value.ends_with("Mode=TwoWay}"), "{id}: {value}");
    }
    // A combobox's item shows the value its box takes, a select's the option's text.
    assert_eq!(at("city-lisbon", "Content").as_deref(), Some("Lisbon"));
    assert_eq!(
        at("city-lisbon", "ToolTipService.ToolTip").as_deref(),
        Some("Portugal")
    );
    assert_eq!(at("size-s", "Content").as_deref(), Some("Small"));
    assert_eq!(at("size-s", "Tag").as_deref(), Some("s"));
    // Literal bounds as SPEC §5.1 reads them; a bound `min` leaves a literal `max` alone. The
    // reversed bounds are not in the fixture, which reads back to the source exactly.
    let (catalog, tokens) = setup();
    let reversed = screen(
        r#"<slider id="flipped" label="F" max="5" min="10" step="0" value="7"/><stepper id="back" label="B" max="2" min="8" step="-1" value="3"/>"#,
    );
    let reversed = document(&reversed, &catalog, &tokens);
    let reversed = generate(&reversed, &options(&catalog, &tokens, None)).unwrap();
    let reversed = roxmltree::Document::parse(&reversed.xaml).unwrap();
    for (id, min, max, step) in [
        ("flipped", "10", "10", "1"),
        ("back", "8", "8", "1"),
        ("above", "{x:Bind ViewModel.bottom, Mode=OneWay}", "-5", ""),
    ] {
        let xml = if id == "above" { &xml } else { &reversed };
        let at = |attr: &str| by_id(xml, id).attribute(attr).map(str::to_owned);
        assert_eq!(at("Minimum").as_deref(), Some(min), "{id}");
        assert_eq!(at("Maximum").as_deref(), Some(max), "{id}");
        let step = (!step.is_empty()).then(|| step.to_owned());
        assert_eq!(at("StepFrequency").or(at("SmallChange")), step, "{id}");
    }
    let vm = &generated.view_model_source;
    assert!(vm.contains("public double level"), "{vm}");
    assert!(vm.contains("private double Weft_count = 0;"), "{vm}");
    // Dates: a literal of the format through `Weft.Earliest`, any other bound a marker.
    let earliest = "{x:Bind weft:Weft.Earliest('2026-01-01')}";
    assert_eq!(at("day", "MinDate").as_deref(), Some(earliest));
    assert_eq!(at("bad-day", "MinDate"), None);
    assert_eq!(
        at("bad-day", "weft:Props.Max").as_deref(),
        Some("2026-02-30")
    );
    assert_eq!(at("alarm", "weft:Props.Max").as_deref(), Some("18:00"));
    let meeting = by_id(&xml, "meeting");
    let parts: Vec<_> = meeting
        .children()
        .filter(|n| n.is_element())
        .map(|n| n.tag_name().name())
        .collect();
    assert_eq!(parts, ["CalendarDatePicker", "TimePicker"]);
    // Colours: a literal swatch is lowercase `#rrggbb`, black when it is not one.
    let swatch = |id: &str| {
        let border = by_id(&xml, id)
            .descendants()
            .find(|n| n.has_tag_name("Border"))
            .unwrap();
        border.attribute("Background").unwrap().to_owned()
    };
    assert_eq!(swatch("plain"), "#d946ef");
    assert_eq!(swatch("broken"), "#000000");
    // SPEC §9 Trust: only `http`, `https` and `mailto` open, and a hostile image loads nothing.
    assert_eq!(
        at("mail", "NavigateUri").as_deref(),
        Some("mailto:help@example.com")
    );
    for id in ["relative", "script"] {
        assert_eq!(at(id, "NavigateUri"), None, "{id}");
        assert!(at(id, "weft:Props.Href").is_some(), "{id}");
    }
    assert_eq!(at("hostile", "Source"), None);
    assert_eq!(at("chair", "Source").as_deref(), Some("assets/chair.png"));
    // An alert's text is its message; other content is a panel.
    assert_eq!(at("info", "Message").as_deref(), Some("Plain notice"));
    assert_eq!(at("info", "Severity"), None);
    assert_eq!(at("bad", "Severity").as_deref(), Some("Error"));
    assert!(
        by_id(&xml, "warn")
            .children()
            .any(|n| n.has_tag_name("StackPanel"))
    );
}

#[test]
fn a_path_has_one_csharp_type() {
    let item_twice = screen(
        r#"<checkbox id="c" checked="{$.todos.0.done}" label="C"/><text id="t" text="{$.todos.1.done}"/>"#,
    );
    assert_eq!(refused(&item_twice), ["text#t"]);
    let list_and_object =
        screen(r#"<text id="a" text="{$.todos.0}"/><text id="b" text="{$.todos.count}"/>"#);
    assert_eq!(refused(&list_and_object), ["text#a"]);
    let list_and_text =
        screen(r#"<text id="a" text="{$.todos}"/><text id="b" text="{$.todos.0.title}"/>"#);
    assert_eq!(refused(&list_and_text), ["text#a"]);
    let lists_of_lists = screen(r#"<text id="a" text="{$.grid.0.0}"/>"#);
    assert_eq!(refused(&lists_of_lists), ["text#a"]);
}

#[test]
fn a_name_csharp_cannot_hold_is_refused() {
    // `user_name` and `user.name` are both the class `SUserNameData`.
    let one_class = screen(
        r#"<text id="a" text="{$.user_name.first}"/><text id="b" text="{$.user.name.first}"/>"#,
    );
    assert_eq!(refused(&one_class), ["$.user.name"]);
    // A member cannot share its class's name.
    assert_eq!(
        refused(&screen(r#"<text id="a" text="{$.SViewModel}"/>"#)),
        ["$.SViewModel"]
    );
    let member = screen(r#"<text id="a" text="{$.todos.0.STodosItem}"/>"#);
    assert_eq!(refused(&member), ["$.todos.0.STodosItem"]);
    for name in [
        "Perform",
        "Performed",
        "OnPerformed",
        "Equals",
        "GetType",
        "WeftRoot",
    ] {
        let body = format!(r#"<text id="a" text="{{$.{name}}}"/>"#);
        assert_eq!(refused(&screen(&body)), [format!("$.{name}")], "{name}");
    }
}

#[test]
fn a_keyword_takes_a_suffix_on_its_csharp_member() {
    let (catalog, tokens) = setup();
    let doc = document(&screen(KEYWORDS_BODY), &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let vm = &generated.view_model_source;
    let root = class(vm, "SViewModel");
    for line in [
        "public bool public_\n",
        "set => WeftSet(ref Weft_public_, value);",
        // A name that already ends like a suffixed keyword takes one more `_`.
        "public string public__\n",
        "private string Weft_public__ = \"\";",
        // A contextual keyword is an ordinary name.
        "public string value\n",
        "public ObservableCollection<SClassItem> class_ { get; } = new();",
        "class_.CollectionChanged += (_, _) => WeftPlaceLists();",
        // The item's path is the Weft path, which events report.
        "Weft.Place(class_, this, null, \"$.class\");",
    ] {
        assert!(root.contains(line), "{line}\n{root}");
    }
    let item = class(vm, "SClassItem");
    assert!(item.contains("public string event_\n"), "{item}");
    for xaml in [
        "IsOn=\"{x:Bind ViewModel.public_, Mode=TwoWay}\"",
        "Text=\"{x:Bind ViewModel.public__, Mode=OneWay}\"",
        "Text=\"{x:Bind ViewModel.value, Mode=OneWay}\"",
        "ItemsSource=\"{x:Bind ViewModel.class_, Mode=OneWay}\"",
        "Text=\"{x:Bind event_, Mode=OneWay}\"",
    ] {
        assert!(generated.xaml.contains(xaml), "{xaml}\n{}", generated.xaml);
    }
}

#[test]
fn a_namespace_must_be_a_csharp_namespace() {
    let (catalog, tokens) = setup();
    let doc = document(&screen(""), &catalog, &tokens);
    for bad in ["", "Acme.", "Acme Screens", "1Acme", "Acme.-x"] {
        let options = GenerateOptions {
            namespace: bad,
            ..options(&catalog, &tokens, None)
        };
        assert!(
            matches!(generate(&doc, &options), Err(GenerateError::Namespace(_))),
            "{bad}"
        );
    }
}

#[test]
fn an_each_repeats_in_its_stacks_flow() {
    let (catalog, tokens) = setup();
    let markup = screen(
        r#"<stack id="tags" direction="row" gap="{token.space.sm}"><text id="label">Tags:</text><each id="e" as="tag" in="{$.tags}"><text id="name" text="{$tag.name}"/><text id="count" text="{$tag.count}"/></each></stack>"#,
    );
    let doc = document(&markup, &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
    let flow = |name: &str| {
        let n = xml
            .descendants()
            .find(|n| n.has_tag_name(name) && n.attribute("Orientation").is_some())
            .unwrap_or_else(|| panic!("no {name} with the row's flow"));
        (n.attribute("Orientation"), n.attribute("Spacing"))
    };
    let row = (Some("Horizontal"), Some("{StaticResource Weft.space.sm}"));
    assert_eq!(flow("StackLayout"), row);
    let panels = xml
        .descendants()
        .filter(|n| {
            n.has_tag_name("StackPanel") && n.attribute("Orientation") == Some("Horizontal")
        })
        .count();
    // The stack itself and the template's panel of the item's two texts.
    assert_eq!(panels, 2);
}

#[test]
fn a_screen_strict_validation_rejects_is_not_generated() {
    let (catalog, tokens) = setup();
    let markup = screen(r#"<text id="t" bogus="1">A</text>"#);
    let parsed = weft_core::parse(
        &markup,
        &weft_core::ParseOptions {
            catalog: Some(&catalog),
            mode: weft_core::Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    let doc = parsed.document.unwrap();
    let result = generate(&doc, &options(&catalog, &tokens, None));
    assert!(
        matches!(result, Err(GenerateError::Invalid(_))),
        "{result:?}"
    );
}

#[test]
fn a_material_is_a_border_and_a_tilt_turns_it_under_its_perspective() {
    let (catalog, tokens) = setup();
    let markup = screen(concat!(
        r#"<stack id="card" material="{token.material.glass}" padding="{token.space.sm}" "#,
        r#"hidden="{$.gone}" perspective="800" rotate-x="-4" rotate-y="12"><text id="t">T</text></stack>"#,
    ));
    let doc = document(&markup, &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
    let card = by_id(&xml, "card");
    let surface = card.parent_element().unwrap();
    let view = surface.parent_element().unwrap();
    // The surface is the stack's box: its background and padding, turned as one.
    assert_eq!(
        surface.attribute("Background"),
        Some("{ThemeResource Weft.material.glass}")
    );
    assert_eq!(
        surface.attribute("Padding"),
        Some("{StaticResource Weft.space.sm.thickness}")
    );
    assert_eq!(card.attribute("Padding"), None);
    assert_eq!(surface.attribute("Tilt.Centered"), Some("True"));
    let turn = surface
        .descendants()
        .find(|n| n.has_tag_name("CompositeTransform3D"))
        .unwrap();
    // CSS turns clockwise, WinUI counter-clockwise.
    assert_eq!(turn.attribute("RotationX"), Some("4"));
    assert_eq!(turn.attribute("RotationY"), Some("-12"));
    assert_eq!(turn.attribute("RotationZ"), None);
    // The perspective is an ancestor's, and the outermost wrapper hides the whole.
    assert_eq!(view.tag_name().name(), "Border");
    let depth = view
        .descendants()
        .find(|n| n.has_tag_name("PerspectiveTransform3D"))
        .and_then(|n| n.attribute("Depth"));
    assert_eq!(depth, Some("800"));
    assert!(view.attribute("Visibility").is_some(), "{}", generated.xaml);
    assert_eq!(surface.attribute("Visibility"), None);
    assert_eq!(card.attribute("Visibility"), None);
}

/// The `x:Key`s of the page's own resources, in order, each with its element.
fn own_keys(xaml: &str) -> Vec<String> {
    let xml = roxmltree::Document::parse(xaml).unwrap();
    let Some(resources) = xml.descendants().find(|n| n.has_tag_name("Page.Resources")) else {
        return vec![];
    };
    let key = ("http://schemas.microsoft.com/winfx/2006/xaml", "Key");
    resources
        .descendants()
        .filter_map(|n| Some(format!("{} {}", n.tag_name().name(), n.attribute(key)?)))
        .collect()
}

#[test]
fn a_page_with_its_own_tokens_holds_only_the_ones_it_uses() {
    let (catalog, tokens) = setup();
    let markup = screen(concat!(
        r#"<stack id="card" gap="{token.space.md}" material="{token.material.glass}">"#,
        r#"<text id="t">T</text></stack>"#,
    ));
    let doc = document(&markup, &catalog, &tokens);
    let own = |appearance| GenerateOptions {
        shared_tokens: false,
        appearance,
        ..options(&catalog, &tokens, None)
    };
    let generated = generate(&doc, &own(None)).unwrap();
    assert_eq!(
        own_keys(&generated.xaml),
        [
            "ResourceDictionary Light",
            "AcrylicBrush Weft.material.glass",
            "ResourceDictionary Dark",
            "AcrylicBrush Weft.material.glass",
            "ResourceDictionary HighContrast",
            "SolidColorBrush Weft.material.glass",
            "Double Weft.space.md",
            "Thickness Weft.space.md.thickness",
        ]
    );
    // The shared form references the same resources and holds none.
    let shared = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    assert!(own_keys(&shared.xaml).is_empty());
    let strip = |x: &str| x.split("<Page.Resources>").next().unwrap().to_owned();
    assert_eq!(
        strip(&generated.xaml),
        strip(&shared.xaml).split("<ScrollViewer>").next().unwrap()
    );
    // A page that uses no token needs no resources.
    let plain = document(&screen(r#"<text id="t">T</text>"#), &catalog, &tokens);
    let generated = generate(&plain, &own(None)).unwrap();
    assert!(!generated.xaml.contains("Page.Resources"));
    // The dark values of an appearance go to the `Dark` dictionary.
    let mut dark = tokens.clone();
    dark["material.glass"].value = weft_core::parse_json(
        r##"{ "colorSpace": "srgb", "components": [0, 0, 0], "alpha": 0.5 }"##,
    )
    .unwrap();
    let appearance = weft_catalog::Appearance {
        modifier: "theme",
        light_context: "light",
        dark_context: "dark",
        light: &tokens,
        dark: &dark,
    };
    let generated = generate(&doc, &own(Some(appearance))).unwrap();
    assert!(
        generated.xaml.contains("TintColor=\"#FF000000\""),
        "{}",
        generated.xaml
    );
}

#[test]
fn an_own_token_with_no_winui_form_is_refused() {
    let (catalog, mut tokens) = setup();
    tokens["space.md"].value = weft_core::parse_json(r#"{ "value": 1, "unit": "em" }"#).unwrap();
    let doc = document(
        &screen(r#"<stack id="card" gap="{token.space.md}"><text id="t">T</text></stack>"#),
        &catalog,
        &tokens,
    );
    let own = GenerateOptions {
        shared_tokens: false,
        ..options(&catalog, &tokens, None)
    };
    let Err(GenerateError::Unsupported(problems)) = generate(&doc, &own) else {
        panic!("an `em` dimension was written");
    };
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].path, "stack#card");
    assert!(problems[0].message.contains("space.md"), "{}", problems[0]);
    // The shared dictionary leaves it out and says so; the page only references it.
    assert!(generate(&doc, &options(&catalog, &tokens, None)).is_ok());
}

#[test]
fn an_each_alone_in_a_reflowing_grid_is_a_columns_layout() {
    let (catalog, tokens) = setup();
    let grid = |body: &str| {
        screen(&format!(
            r#"<grid id="g" columns="2" min-column-width="{{token.size.sm}}" padding="{{token.space.sm}}">{body}</grid>"#
        ))
    };
    let one = r#"<each id="e" as="p" in="{$.photos}"><text id="c" text="{$p.caption}"/></each>"#;
    let doc = document(&grid(one), &catalog, &tokens);
    let generated = generate(&doc, &options(&catalog, &tokens, None)).unwrap();
    let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
    let repeater = by_id(&xml, "g");
    assert_eq!(repeater.tag_name().name(), "ItemsRepeater");
    // A repeater has no padding: the layout keeps it around the columns.
    assert_eq!(repeater.attribute("Padding"), None);
    let layout = repeater
        .descendants()
        .find(|n| n.tag_name().name() == "ColumnsLayout")
        .unwrap();
    assert_eq!(layout.attribute("Columns"), Some("2"));
    assert_eq!(
        layout.attribute("Padding"),
        Some("{StaticResource Weft.space.sm.thickness}")
    );
    // A cell is one element, and a repeater cannot share the columns with static cells.
    for body in [
        r#"<each id="e" as="p" in="{$.photos}"><text id="c" text="{$p.caption}"/><text id="d">D</text></each>"#,
        &format!(r#"<text id="first">First</text>{one}"#),
    ] {
        let doc = document(&grid(body), &catalog, &tokens);
        let Err(GenerateError::Unsupported(problems)) =
            generate(&doc, &options(&catalog, &tokens, None))
        else {
            panic!("{body}");
        };
        assert_eq!(problems[0].path, "each#e", "{body}");
    }
}
