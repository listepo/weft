//! Claims about fragments (SPEC §10.7): a `<fragment>` is checked on its own with its parameters
//! in scope, a `<use>` is checked against the parameters, and patches stay out of fragments.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, document, markup_codes, screen};
use serde_json::json;
use weft_core::{ApplyOptions, Catalog, Fragment, ParseOptions, Value, apply_patches, parse};

fn fragment(body: &str) -> Vec<&'static str> {
    markup_codes(&format!("<fragment weft=\"0.2\">{body}</fragment>"))
}

const HEADER: &str = r#"<use id="h" fragment="page-header" title="T""#;

const SPEC_FRAGMENT: &str = r#"<fragment label="Page header" weft="0.2">
  <param name="title" required="true" type="string"/>
  <param default="muted" name="tone" type="enum" values="default muted"/>
  <param name="back" type="action"/>
  <param allowed-children="button link" name="actions" type="slot"/>
  <stack id="bar" direction="row">
    <button id="back" on-press="{$back}">Back</button>
    <heading id="title" level="1" text="{$title}"/>
    <text id="note" tone="{$tone}">Signed in</text>
    <outlet name="actions"/>
  </stack>
</fragment>"#;

#[test]
fn the_spec_fragment_checks_cleanly_and_reads_as_canonical_json() {
    assert_eq!(markup_codes(SPEC_FRAGMENT), Vec::<&str>::new());
    let root = serde_json::to_value(document(SPEC_FRAGMENT).root).unwrap();
    assert_eq!(
        root["children"][0],
        json!({"kind": "param", "props": {"name": "title", "required": true, "type": "string"}})
    );
    assert_eq!(root["children"][1]["props"]["default"], json!("muted"));
    let bar = &root["children"][4];
    assert_eq!(bar["children"][0]["on"], json!({"press": "{$back}"}));
    assert_eq!(
        bar["children"][1]["props"]["text"],
        json!({"bind": "$title"})
    );
    assert_eq!(
        bar["children"][3],
        json!({"kind": "outlet", "props": {"name": "actions"}})
    );
}

#[test]
fn a_fragment_body_reads_its_parameters_by_type() {
    let p = r#"<param name="n" type="number"/><param name="s" type="string"/><param name="e" type="enum" values="muted loud"/><param name="t" type="token" token-type="color"/><param name="b" type="boolean"/><param name="go" type="action"/><param name="more" type="slot"/>"#;
    let cases: &[(&str, &[&str])] = &[
        (r#"<text id="a" text="{$s}"/>"#, &[]),
        (r#"<text id="a" text="{$e}"/>"#, &[]),
        (r#"<text id="a" text="{$n}"/>"#, &["W204"]),
        (r#"<text id="a" tone="{$e}"/>"#, &["W203"]),
        (r#"<stack id="a" gap="{$t}"/>"#, &["W307"]),
        (r#"<button id="a" disabled="{!$b}">B</button>"#, &[]),
        (r#"<text id="a" text="{!$s}"/>"#, &["W218"]),
        (r#"<text id="a" text="{$s.x}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$go}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$more}"/>"#, &["W807"]),
        (r#"<text id="a" text="{$nope}"/>"#, &["W305"]),
        (r#"<button id="a" on-press="{$go}">B</button>"#, &[]),
        (r#"<button id="a" on-press="{$s}">B</button>"#, &["W807"]),
        (r#"<button id="a" on-press="{$gone}">B</button>"#, &["W305"]),
        (r#"<stack id="a"><outlet name="more"/></stack>"#, &[]),
        (
            r#"<stack id="a"><outlet name="more"/><outlet name="more"/></stack>"#,
            &["W804"],
        ),
        (r#"<stack id="a"><outlet name="go"/></stack>"#, &["W804"]),
        (
            r#"<stack id="a"><param name="x" type="string"/></stack>"#,
            &["W803"],
        ),
        (
            r#"<button id="a" submit="true" grow="true">B</button>"#,
            &[],
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(fragment(&format!("{p}{body}")), *expected, "{body}");
    }
}

#[test]
fn declarations_are_checked() {
    let cases: &[(&str, &[&str])] = &[
        (
            r#"<param name="a" type="string"/><text id="t">T</text>"#,
            &[],
        ),
        (
            r#"<text id="t">T</text><param name="a" type="string"/>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string"/><param name="a" type="number"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="id" type="string"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string" values="x y"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="number" default="x"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="number" min="1" default="0"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="enum"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="string" required="true" default="x"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (
            r#"<param name="a" type="slot" content="text"/><text id="t">T</text>"#,
            &["W803"],
        ),
        (r#"<param name="a" type="string"/>"#, &["W803"]),
        (r#"<text id="t">T</text><text id="t">U</text>"#, &["W301"]),
    ];
    for (body, expected) in cases {
        assert_eq!(fragment(body), *expected, "{body}");
    }
    assert_eq!(
        markup_codes(&screen(r#"<stack id="a"><outlet name="x"/></stack>"#)),
        ["W804"]
    );
}

#[test]
fn the_fixture_fragment_checks_cleanly_and_has_a_signature() {
    let fixture = catalog();
    let header = &fixture.fragments["page-header"];
    let markup = weft_core::serialize(&header.document);
    assert_eq!(markup_codes(&markup), Vec::<&str>::new());
    let props = header.signature.props.as_ref().unwrap();
    assert_eq!(
        props.keys().collect::<Vec<_>>(),
        ["fragment", "title", "tone"]
    );
    assert_eq!(
        header.signature.events.as_deref(),
        Some(&["back".to_owned()][..])
    );
    assert!(header.signature.slot("actions").is_some());
}

#[test]
fn use_literals_are_typed_by_the_parameters() {
    let doc = document(&screen(
        r#"<use id="h" fragment="page-header" title="7" tone="muted"/>"#,
    ));
    let Some(weft_core::Child::Node(node)) = doc.root.children.first() else {
        panic!("no use");
    };
    assert_eq!(node.props["title"], Value::String("7".into()));
    let raw = parse(
        &screen(r#"<use id="h" title="7"/>"#),
        &ParseOptions::default(),
    );
    assert!(raw.document.is_some());
}

#[test]
fn a_use_answers_to_its_fragment() {
    let cases: &[(&str, &[&str])] = &[
        (r#"<use id="h" fragment="page-header"/>"#, &["W205"]),
        (
            r#"<use id="h" fragment="page-headr" title="T"/>"#,
            &["W801"],
        ),
        (r#"<use id="h" title="T"/>"#, &["W205"]),
        (&format!("{HEADER} tone=\"loud\"/>"), &["W203"]),
        (&format!("{HEADER} hidden=\"true\"/>"), &["W802"]),
        (&format!("{HEADER} on-close=\"a.b\"/>"), &["W802"]),
        (&format!("{HEADER} on-back=\"a.b\"/>"), &[]),
        (
            &format!("{HEADER}><slot name=\"extra\"><text id=\"x\">X</text></slot></use>"),
            &["W802"],
        ),
        (
            &format!("{HEADER}><slot name=\"actions\"><text id=\"x\">X</text></slot></use>"),
            &["W302"],
        ),
        (
            &format!("{HEADER}><text id=\"x\">X</text></use>"),
            &["W304"],
        ),
        (&format!("{HEADER} x-acme-note=\"n\"/>"), &[]),
        (
            r#"<use id="title" fragment="page-header" title="T"/><text id="bar">B</text>"#,
            &[],
        ),
    ];
    for (markup, expected) in cases {
        assert_eq!(markup_codes(&screen(markup)), *expected, "{markup}");
    }
}

#[test]
fn what_a_use_places_answers_to_where_it_stands() {
    // `page-header` places a <stack>, which a <select> does not take.
    let markup = format!("<select id=\"s2\" label=\"L\">{HEADER}/></select>");
    assert_eq!(markup_codes(&screen(&markup)), ["W302"]);
    let mut catalog = catalog();
    let item = r#"<fragment weft="0.2"><item id="i">I</item></fragment>"#;
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let parsed = parse(item, &options).document.unwrap();
    catalog
        .fragments
        .insert("row".into(), Fragment::new(parsed));
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let placed = |inner: &str| codes(&parse(&screen(inner), &options).diagnostics);
    assert_eq!(
        placed(r#"<list id="l"><use id="u" fragment="row"/></list>"#),
        Vec::<&str>::new()
    );
    assert_eq!(
        placed(r#"<stack id="l"><use id="u" fragment="row"/></stack>"#),
        ["W303"]
    );
}

#[test]
fn patches_set_parameters_and_never_reach_into_a_fragment() {
    let catalog = catalog();
    let base = document(&screen(&format!("{HEADER}/>")));
    let options = ApplyOptions::new(&catalog);
    let set = |prop: &str, value: serde_json::Value| {
        let patch = json!([{"op": "set", "id": "h", "prop": prop, "value": value}]);
        codes(&apply_patches(&base, &patch, &options).diagnostics)
    };
    assert_eq!(set("title", json!("New")), Vec::<&str>::new());
    assert_eq!(set("title", json!(3)), ["W204"]);
    assert_eq!(set("on-back", json!("nav.back")), Vec::<&str>::new());
    let insert = json!([{"op": "insert", "parent": "h", "slot": "actions", "markup": "<button id=\"x\">X</button>"}]);
    assert_eq!(
        codes(&apply_patches(&base, &insert, &options).diagnostics),
        Vec::<&str>::new()
    );
    let wrong = json!([{"op": "insert", "parent": "h", "slot": "nope", "markup": "<button id=\"x\">X</button>"}]);
    assert_eq!(
        codes(&apply_patches(&base, &wrong, &options).diagnostics),
        ["W504"]
    );
    for id in ["title", "h/title"] {
        let remove = json!([{"op": "remove", "id": id}]);
        let result = apply_patches(&base, &remove, &options);
        assert_eq!(codes(&result.diagnostics), ["W502"]);
        assert!(
            result.diagnostics[0]
                .hint
                .as_deref()
                .unwrap()
                .contains("page-header")
        );
    }
}

#[test]
fn a_catalog_with_fragments_round_trips_through_json() {
    let catalog = catalog();
    let text = serde_json::to_string(&catalog).unwrap();
    let back: Catalog = serde_json::from_str(&text).unwrap();
    assert_eq!(back, catalog);
    let broken = json!({"weft": "0.2", "name": "c", "version": "1", "components": {}, "fragments": {"x": {"weft": "0.2"}}});
    assert!(serde_json::from_value::<Catalog>(broken).is_err());
}
