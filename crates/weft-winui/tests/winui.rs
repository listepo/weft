//! Weft → WinUI over the corpus screens and catalog examples the generator covers so far, and a
//! fixture of every prop it draws: the XAML matches its reviewed golden file and is well-formed
//! XML, and what the slice cannot draw is refused with its path. `WEFT_UPDATE_FIXTURES=1`
//! rewrites the golden files.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{COVERED, controls, covered, document, here, options, screen, screens, setup};
use weft_winui::{GenerateError, GenerateOptions, generate};

#[test]
fn exactly_the_covered_screens_generate() {
    let (catalog, tokens) = setup();
    let mut generated = vec![];
    for (name, path) in screens() {
        let doc = document(&std::fs::read_to_string(path).unwrap(), &catalog, &tokens);
        match generate(&doc, &options(&catalog, &tokens, None)) {
            Ok(_) => generated.push(name),
            Err(GenerateError::Unsupported(problems)) => assert!(!problems.is_empty(), "{name}"),
            Err(e) => panic!("{name}: {e}"),
        }
    }
    let mut expected: Vec<&str> = COVERED.to_vec();
    expected.push("controls");
    assert_eq!(generated, expected);
}

#[test]
fn generated_xaml_matches_the_golden_files() {
    let update = std::env::var_os("WEFT_UPDATE_FIXTURES").is_some();
    for (name, _, generated) in covered() {
        let xml = roxmltree::Document::parse(&generated.xaml).unwrap();
        assert_eq!(xml.root_element().tag_name().name(), "Page", "{name}");
        let golden = here(&format!("tests/fixtures/{}.xaml", generated.page));
        if update {
            std::fs::write(&golden, &generated.xaml).unwrap();
        }
        let expected = std::fs::read_to_string(&golden).unwrap_or_default();
        assert_eq!(generated.xaml, expected, "{name}");
    }
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
        (r#"<link id="l" href="https://a.b">A</link>"#, "link#l"),
        (
            r#"<stack id="r" justify="center"><text id="a">A</text></stack>"#,
            "stack#r",
        ),
        (
            r#"<stack id="r" padding="{token.space.md}"><text id="a">A</text></stack>"#,
            "stack#r",
        ),
        (
            r#"<stack id="r"><text id="g" grow="true">A</text></stack>"#,
            "text#g",
        ),
        (r#"<heading id="h" level="{$.n}">A</heading>"#, "heading#h"),
        (r#"<button id="b" variant="{$.v}">A</button>"#, "button#b"),
        (r#"<text id="t" tone="{$.tone}">A</text>"#, "text#t"),
        (r#"<field id="f" label="F" type="{$.type}"/>"#, "field#f"),
        (
            r#"<text id="t" text="{$.items.0.name}"/>"#,
            "$.items.0.name",
        ),
        (r#"<text id="t" text="{$.Performed}"/>"#, "$.Performed"),
        (
            r#"<text id="t" text="{$.user.WeftPath}"/>"#,
            "$.user.WeftPath",
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
