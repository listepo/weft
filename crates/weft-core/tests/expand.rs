//! Claims about expansion (SPEC §10.7): a use means its fragment's body with the use's values,
//! slot content and instance paths, evaluated in scope, and hostile fragments stop it.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, codes, screen};
use serde_json::{Value as Json, json};
use weft_core::{
    Catalog, Fragment, Mode, ParseOptions, ValidateOptions, expand, parse, validate_document,
};

fn with(fragments: &[(&str, &str)]) -> Catalog {
    let mut catalog = catalog();
    for (name, markup) in fragments {
        let parsed = parse(markup, &ParseOptions::default());
        let document = parsed.document.unwrap();
        catalog
            .fragments
            .insert((*name).into(), Fragment::new(document));
    }
    catalog
}

fn expanded(inner: &str, catalog: &Catalog) -> (Json, Vec<&'static str>) {
    let options = ParseOptions {
        catalog: Some(catalog),
        ..ParseOptions::default()
    };
    let document = parse(&screen(inner), &options).document.unwrap();
    let result = expand(&document, catalog);
    let root = serde_json::to_value(&result.document.root).unwrap();
    (root, codes(&result.diagnostics))
}

#[test]
fn a_use_becomes_its_body_with_values_slots_and_instance_paths() {
    let (root, problems) = expanded(
        r#"<use id="h" fragment="page-header" title="Cart" on-back="nav.back"><slot name="actions"><button id="clear" on-press="cart.clear">Clear</button></slot></use>"#,
        &catalog(),
    );
    assert_eq!(problems, Vec::<&str>::new());
    let bar = &root["children"][0];
    assert_eq!(bar["id"], json!("h/bar"));
    let kids = &bar["children"];
    assert_eq!(kids[0]["id"], json!("h/back"));
    assert_eq!(kids[0]["on"], json!({"press": "nav.back"}));
    assert_eq!(kids[1]["props"]["text"], json!("Cart"));
    // An absent optional parameter takes its default.
    assert_eq!(kids[2]["props"]["tone"], json!("muted"));
    // The slot content keeps its screen ids.
    assert_eq!(kids[3]["id"], json!("clear"));
    assert_eq!(kids.as_array().unwrap().len(), 4);
}

#[test]
fn reads_follow_negation_and_absent_values_leave_the_attribute_out() {
    let catalog = with(&[(
        "flag",
        r#"<fragment weft="0.2"><param name="busy" type="boolean"/><param name="note" type="string"/><text id="t" hidden="{!$busy}" text="{$note}">x</text></fragment>"#,
    )]);
    let text = |inner: &str| expanded(inner, &catalog).0["children"][0]["props"].clone();
    assert_eq!(
        text(r#"<use id="a" fragment="flag" busy="{!$.ready}"/>"#),
        json!({"hidden": {"bind": "$.ready"}})
    );
    assert_eq!(
        text(r#"<use id="a" fragment="flag" busy="true" note="{$.n}"/>"#),
        json!({"hidden": false, "text": {"bind": "$.n"}})
    );
}

#[test]
fn loop_variables_of_the_body_never_capture_use_site_names() {
    let catalog = with(&[(
        "rows",
        r#"<fragment weft="0.2"><param name="label" type="string"/><each in="{$.items}" as="line"><text id="t" text="{$line.name}" label="{$label}">x</text></each></fragment>"#,
    )]);
    let (root, problems) = expanded(
        r#"<each in="{$.lines}" as="line"><use id="r" fragment="rows" label="{$line.title}"/></each>"#,
        &catalog,
    );
    assert_eq!(problems, Vec::<&str>::new());
    let inner = &root["children"][0]["children"][0];
    assert_eq!(inner["props"]["as"], json!("line2"));
    let text = &inner["children"][0]["props"];
    assert_eq!(text["text"], json!({"bind": "$line2.name"}));
    assert_eq!(text["label"], json!({"bind": "$line.title"}));
}

#[test]
fn nested_uses_join_instance_paths_and_forward_actions() {
    let catalog = with(&[
        (
            "crumb",
            r#"<fragment weft="0.2"><param name="go" type="action"/><link id="home" on-press="{$go}">Home</link></fragment>"#,
        ),
        (
            "bar",
            r#"<fragment weft="0.2"><param name="back" type="action"/><stack id="s"><use id="crumbs" fragment="crumb" on-go="{$back}"/></stack></fragment>"#,
        ),
    ]);
    let (root, _) = expanded(
        r#"<use id="top" fragment="bar" on-back="nav.back"/>"#,
        &catalog,
    );
    let link = &root["children"][0]["children"][0];
    assert_eq!(link["id"], json!("top/crumbs/home"));
    assert_eq!(link["on"], json!({"press": "nav.back"}));
}

#[test]
fn a_cycle_expands_to_nothing_and_is_reported_by_validation() {
    let catalog = with(&[
        (
            "a",
            r#"<fragment weft="0.2"><use id="b" fragment="b"/></fragment>"#,
        ),
        (
            "b",
            r#"<fragment weft="0.2"><use id="a" fragment="a"/></fragment>"#,
        ),
    ]);
    let (root, problems) = expanded(r#"<use id="x" fragment="a"/>"#, &catalog);
    assert_eq!(problems, ["W805"]);
    assert_eq!(root.get("children"), None);
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let document = parse(&screen(r#"<use id="x" fragment="a"/>"#), &options)
        .document
        .unwrap();
    let validate = ValidateOptions {
        catalog: Some(&catalog),
        mode: Mode::Strict,
        ..ValidateOptions::default()
    };
    let found = validate_document(&document, &validate);
    assert_eq!(codes(&found), ["W805"]);
    assert_eq!(found[0].path, "/screen#root/use#x");
}

#[test]
fn exponential_fan_out_stops_at_the_limit() {
    let ten = |inner: &str| {
        let uses: String = (0..10)
            .map(|i| format!(r#"<use id="u{i}" fragment="{inner}"/>"#))
            .collect();
        format!(r#"<fragment weft="0.2"><stack id="s">{uses}</stack></fragment>"#)
    };
    let (b, c, d) = (ten("c"), ten("d"), ten("e"));
    let catalog = with(&[
        ("a", &ten("b")),
        ("b", &b),
        ("c", &c),
        ("d", &d),
        (
            "e",
            r#"<fragment weft="0.2"><text id="t">x</text></fragment>"#,
        ),
    ]);
    let (root, problems) = expanded(r#"<use id="x" fragment="a"/>"#, &catalog);
    assert_eq!(problems, ["W806"]);
    assert_eq!(root.get("children"), None);
}

#[test]
fn a_use_of_an_unknown_fragment_is_kept() {
    let (root, problems) = expanded(r#"<use id="x" fragment="nope"/>"#, &catalog());
    assert_eq!(problems, Vec::<&str>::new());
    assert_eq!(root["children"][0]["kind"], json!("use"));
}

#[test]
fn a_fragment_in_its_own_slot_is_no_cycle() {
    let catalog = with(&[(
        "box",
        r#"<fragment weft="0.2"><param name="body" type="slot"/><stack id="s"><outlet name="body"/></stack></fragment>"#,
    )]);
    let (root, problems) = expanded(
        r#"<use id="a" fragment="box"><slot name="body"><use id="b" fragment="box"><slot name="body"><text id="t">x</text></slot></use></slot></use>"#,
        &catalog,
    );
    assert_eq!(problems, Vec::<&str>::new());
    let outer = &root["children"][0];
    assert_eq!(outer["id"], json!("a/s"));
    assert_eq!(outer["children"][0]["id"], json!("b/s"));
    assert_eq!(outer["children"][0]["children"][0]["id"], json!("t"));
}
