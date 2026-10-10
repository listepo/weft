//! Claims about fragments (SPEC §10.7): a `<fragment>` is checked on its own with its parameters
//! in scope, a `<use>` is checked against the parameters, and patches stay out of fragments.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, document, markup_codes, screen};
use serde_json::json;
use weft_core::{
    ApplyOptions, Catalog, Fragment, ParseOptions, Value, apply_patches, expand, explain, parse,
    serialize,
};

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

const PRICE_ROW: &str = r#"<fragment label="Price row" weft="0.2">
  <param name="label" required="true" type="string"/>
  <param name="amount" required="true" type="string"/>
  <param default="normal" name="emphasis" type="enum" values="normal total" variant="true"/>
  <variant when="normal">
    <stack id="row" direction="row">
      <text id="name" text="{$label}"/>
      <text id="value" text="{$amount}"/>
    </stack>
  </variant>
  <variant when="total">
    <stack id="row" direction="row">
      <heading id="name" level="3" text="{$label}"/>
      <heading id="value" level="3" text="{$amount}"/>
    </stack>
  </variant>
</fragment>
"#;

fn with_fragment(name: &str, markup: &str) -> Catalog {
    let mut catalog = catalog();
    let document = document(markup);
    catalog
        .fragments
        .insert(name.to_owned(), Fragment::new(document));
    catalog
}

#[test]
fn a_variant_fragment_is_canonical_and_the_parameter_is_a_literal() {
    assert_eq!(markup_codes(PRICE_ROW), Vec::<&str>::new());
    let doc = document(PRICE_ROW);
    assert_eq!(serialize(&doc), PRICE_ROW);
    let root = serde_json::to_value(&doc.root).unwrap();
    assert_eq!(root["children"][2]["props"]["variant"], json!(true));
    assert_eq!(root["children"][3]["kind"], json!("variant"));
    assert_eq!(root["children"][3]["props"]["when"], json!("normal"));
    assert!(root["children"][3].get("id").is_none());
    let signature = Fragment::new(doc).signature;
    let emphasis = &signature.props.as_ref().unwrap()["emphasis"];
    assert_eq!(emphasis.bindable, Some(false));
    assert!(
        signature.props.as_ref().unwrap()["label"]
            .bindable
            .is_none()
    );
}

#[test]
fn variant_declarations_are_w809_and_ids_are_per_variant() {
    let cases: &[(&str, &[&str])] = &[
        (
            r#"<param name="a" type="string" variant="true"/><text id="t">T</text>"#,
            &["W809"],
        ),
        (
            r#"<param name="a" type="enum" values="a b" variant="true"/><param name="b" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text></variant><variant when="b"><text id="t">T</text></variant>"#,
            &["W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text></variant>"#,
            &["W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text></variant><variant when="a"><text id="u">U</text></variant>"#,
            &["W809", "W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="a b"><text id="t">T</text></variant>"#,
            &[],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="nope"><text id="t">T</text></variant><variant when="a"><text id="u">U</text></variant>"#,
            &["W809", "W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a" variant="true"/><stack id="t"/>"#,
            &["W809", "W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a" variant="true"/><variant id="v" when="a"><text id="t">T</text></variant>"#,
            &["W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a" variant="true"/><variant when="a"/>"#,
            &["W809"],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text></variant><variant when="b"><text id="t">U</text></variant>"#,
            &[],
        ),
        (
            r#"<param default="a" name="a" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text><text id="t">U</text></variant><variant when="b"><text id="u">U</text></variant>"#,
            &["W301"],
        ),
        (
            r#"<variant when="a"><text id="t">T</text></variant>"#,
            &["W809"],
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(fragment(body), *expected, "{body}");
    }
    assert_eq!(
        markup_codes(&screen(
            r#"<stack id="s"><variant when="a"><text id="t">T</text></variant></stack>"#
        )),
        ["W809"]
    );
    // A reference resolves inside its own variant. The other variant's ids do not count.
    assert_eq!(
        fragment(
            r#"<param default="a" name="k" type="enum" values="a b" variant="true"/><variant when="a"><tabs id="tabs" selected="one"><tab id="one" label="One"/></tabs></variant><variant when="b"><tabs id="tabs" selected="one"><tab id="two" label="Two"/></tabs></variant>"#,
        ),
        ["W309"]
    );
}

#[test]
fn a_use_places_and_explains_only_the_chosen_variant() {
    let catalog = with_fragment("price-row", PRICE_ROW);
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let codes_of = |markup: &str| codes(&parse(&screen(markup), &options).diagnostics);
    assert_eq!(
        codes_of(r#"<use id="s" fragment="price-row" label="Subtotal" amount="1"/>"#),
        Vec::<&str>::new()
    );
    assert_eq!(
        codes_of(r#"<use id="s" fragment="price-row" label="Total" amount="1" emphasis="{$.x}"/>"#),
        ["W217"]
    );
    let required = r#"<fragment weft="0.2"><param name="k" required="true" type="enum" values="a b" variant="true"/><variant when="a"><text id="t">T</text></variant><variant when="b"><text id="t">T</text></variant></fragment>"#;
    let required_catalog = with_fragment("need", required);
    let options = ParseOptions {
        catalog: Some(&required_catalog),
        ..ParseOptions::default()
    };
    assert_eq!(
        codes(&parse(&screen(r#"<use id="s" fragment="need"/>"#), &options).diagnostics),
        ["W205"]
    );

    let placed = with_fragment(
        "row",
        r#"<fragment weft="0.2"><param default="row" name="kind" type="enum" values="row item" variant="true"/><variant when="row"><stack id="s"><text id="t">T</text></stack></variant><variant when="item"><item id="i">I</item></variant></fragment>"#,
    );
    let options = ParseOptions {
        catalog: Some(&placed),
        ..ParseOptions::default()
    };
    let placed_codes = |markup: &str| codes(&parse(&screen(markup), &options).diagnostics);
    assert_eq!(
        placed_codes(r#"<list id="l"><use id="u" fragment="row" kind="item"/></list>"#),
        Vec::<&str>::new()
    );
    assert_eq!(
        placed_codes(r#"<stack id="l"><use id="u" fragment="row" kind="item"/></stack>"#),
        ["W303"]
    );
    assert_eq!(
        placed_codes(r#"<list id="l"><use id="u" fragment="row"/></list>"#),
        ["W302"]
    );

    let screen = screen(
        r#"<use id="subtotal" fragment="price-row" label="Subtotal" amount="1"/><use id="total" fragment="price-row" label="Total" amount="2" emphasis="total"/>"#,
    );
    let document = parse(&screen, &options_of(&catalog)).document.unwrap();
    let result = expand(&document, &catalog);
    assert_eq!(codes(&result.diagnostics), Vec::<&str>::new());
    let root = serde_json::to_value(&result.document.root).unwrap();
    assert_eq!(root["children"][0]["kind"], json!("stack"));
    assert_eq!(root["children"][0]["id"], json!("subtotal/row"));
    assert_eq!(root["children"][0]["children"][0]["kind"], json!("text"));
    assert_eq!(root["children"][1]["children"][0]["kind"], json!("heading"));
    assert_eq!(
        root["children"][1]["children"][1]["id"],
        json!("total/value")
    );
    let lines: Vec<String> = explain(&document, Some(&catalog))
        .iter()
        .map(|line| line.to_string())
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("uses the \"normal\" variant of price-row, the default")),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("uses the \"total\" variant of price-row")),
        "{lines:?}"
    );
}

fn options_of(catalog: &Catalog) -> ParseOptions<'_> {
    ParseOptions {
        catalog: Some(catalog),
        ..ParseOptions::default()
    }
}

#[test]
fn a_variant_without_an_outlet_drops_that_slot() {
    let markup = r#"<fragment weft="0.2"><param name="note" type="slot"/><param default="normal" name="emphasis" type="enum" values="normal total" variant="true"/><variant when="normal"><stack id="row"><outlet name="note"/></stack></variant><variant when="total"><text id="value" text="{$emphasis}"/></variant></fragment>"#;
    let catalog = with_fragment("row", markup);
    let options = options_of(&catalog);
    let screen = screen(
        r#"<use id="t" fragment="row" emphasis="total"><slot name="note"><text id="n">N</text></slot></use>"#,
    );
    let document = parse(&screen, &options).document.unwrap();
    assert_eq!(
        codes(&parse(&screen, &options).diagnostics),
        Vec::<&str>::new()
    );
    let result = expand(&document, &catalog);
    let root = serde_json::to_value(&result.document.root).unwrap();
    assert_eq!(root["children"][0]["kind"], json!("text"));
    assert_eq!(root["children"][0]["props"]["text"], json!("total"));
    assert_eq!(root["children"].as_array().unwrap().len(), 1);
    let lines: Vec<String> = explain(&document, Some(&catalog))
        .iter()
        .map(|line| line.to_string())
        .collect();
    assert!(
        lines.iter().any(|l| l.contains("drops the \"note\" slot")),
        "{lines:?}"
    );
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
