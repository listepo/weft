//! `version` on a screen or a fragment (SPEC §3): a literal, canonical, and set by `set-version`.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use weft_core::{ApplyOptions, ParseOptions, parse, serialize, stringify};

mod common;

fn codes(markup: &str) -> Vec<&'static str> {
    common::markup_codes(markup)
}

#[test]
fn version_is_lifted_and_sorted_with_the_other_attributes() {
    let markup = r#"<screen weft="0.3" id="cart" version="1.4.0" label="Cart"><text id="t">Hi</text></screen>"#;
    let result = parse(markup, &ParseOptions::default());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let document = result.document.unwrap();
    assert_eq!(document.version.as_deref(), Some("1.4.0"));
    assert!(document.root.props.get("version").is_none());
    let json = stringify(&document);
    assert!(json.starts_with("{\n  \"weft\": \"0.3\",\n  \"version\": \"1.4.0\",\n"));
    assert_eq!(
        serialize(&document).lines().next().unwrap(),
        r#"<screen id="cart" label="Cart" version="1.4.0" weft="0.3">"#
    );
}

#[test]
fn a_binding_or_a_short_version_is_w230() {
    assert!(codes(r#"<screen id="s" label="S" version="{$.x}" weft="0.3"/>"#).contains(&"W230"));
    assert!(codes(r#"<screen id="s" label="S" version="1.2" weft="0.3"/>"#).contains(&"W230"));
    assert!(
        codes(r#"<screen id="s" label="S" version="1.2.3-alpha" weft="0.3"/>"#).contains(&"W230")
    );
    assert!(
        !codes(
            r#"<screen id="s" label="S" version="1.2.3" weft="0.3"><text id="t">Hi</text></screen>"#
        )
        .contains(&"W230")
    );
    assert!(
        !codes(r#"<fragment version="1.0.0" weft="0.3"><text id="t">Hi</text></fragment>"#)
            .contains(&"W230")
    );
}

#[test]
fn set_version_stores_a_literal_and_drops_a_bad_one() {
    let catalog = common::catalog();
    let base =
        common::document(r#"<screen id="s" label="S" weft="0.3"><text id="t">Hi</text></screen>"#);
    let applied = |value: serde_json::Value| {
        weft_core::apply_patches(
            &base,
            &json!([{ "op": "set-version", "value": value }]),
            &ApplyOptions::new(&catalog),
        )
    };
    let ok = applied(json!("1.2.3"));
    assert!(ok.diagnostics.is_empty(), "{:?}", ok.diagnostics);
    assert_eq!(
        ok.document.as_ref().unwrap().version.as_deref(),
        Some("1.2.3")
    );
    let removed = weft_core::apply_patches(
        ok.document.as_ref().unwrap(),
        &json!([{ "op": "set-version", "value": null }]),
        &ApplyOptions::new(&catalog),
    );
    assert!(removed.document.unwrap().version.is_none());
    let bad = applied(json!("1.2"));
    assert!(bad.document.is_none());
    assert!(common::codes(&bad.diagnostics).contains(&"W230"));
}
