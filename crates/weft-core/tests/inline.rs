//! Inline fragments (SPEC §10.7): a `<fragment name>` on one screen, resolved before the project's.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{catalog, document, markup_codes, parse_lenient, screen};
use serde_json::json;
use weft_core::{ApplyOptions, Code, Mode, apply_patches, expand, serialize};

fn row_screen(uses: &str) -> String {
    format!(
        "<screen id=\"cart\" weft=\"0.2\"><fragment name=\"price-row\"><param name=\"label\" type=\"string\"/><stack id=\"row\"><text id=\"name\" text=\"{{$label}}\"/></stack></fragment>{uses}</screen>"
    )
}

#[test]
fn a_screen_keeps_an_inline_fragment_and_places_it() {
    let markup = row_screen(r#"<use id="sub" fragment="price-row" label="Sub"/>"#);
    let doc = document(&markup);
    assert!(doc.fragments.contains_key("price-row"));
    assert!(
        doc.root
            .children
            .iter()
            .all(|c| c.as_node().unwrap().kind != "fragment")
    );
    let text = serialize(&doc);
    assert!(
        text.find("<fragment ").unwrap() < text.find("<use ").unwrap(),
        "{text}"
    );
    let expanded = expand(&doc, &catalog()).document;
    let again = serialize(&expanded);
    assert!(again.contains(r#"id="sub/row""#), "{again}");
    assert!(again.contains("Sub"), "{again}");
    assert!(!again.contains("<fragment"), "{again}");
}

#[test]
fn inline_fragments_are_written_sorted_by_name() {
    let markup = r#"<screen id="s" weft="0.2"><fragment name="b"><text id="t">B</text></fragment><fragment name="a"><text id="t">A</text></fragment><text id="body">x</text></screen>"#;
    let text = serialize(&document(markup));
    assert!(
        text.find(r#"name="a""#).unwrap() < text.find(r#"name="b""#).unwrap(),
        "{text}"
    );
}

#[test]
fn an_inline_fragment_works_without_a_project() {
    let markup = row_screen(r#"<use id="sub" fragment="price-row" label="Sub"/>"#);
    let parsed = weft_core::parse(&markup, &weft_core::ParseOptions::default());
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    assert!(parsed.document.unwrap().fragments.contains_key("price-row"));
}

#[test]
fn a_use_prefers_the_inline_fragment() {
    let markup = row_screen(r#"<use id="sub" fragment="price-row" label="Sub"/>"#);
    let doc = document(&markup);
    let expanded = expand(&doc, &catalog()).document;
    // The fixture catalog has no price-row, so the inline body is what expanded.
    assert!(serialize(&expanded).contains(r#"id="sub/name""#));
}

#[test]
fn misplaced_fragments_are_left_out() {
    let after =
        screen(r#"<text id="t">x</text><fragment name="row"><text id="a">y</text></fragment>"#);
    assert_eq!(markup_codes(&after), ["W808"]);
    assert!(parse_lenient(&after).document.unwrap().fragments.is_empty());

    let nested =
        screen(r#"<stack id="s"><fragment name="row"><text id="a">y</text></fragment></stack>"#);
    assert_eq!(markup_codes(&nested), ["W808"]);

    let extra = screen(
        r#"<fragment id="f" name="row"><text id="a">y</text></fragment><text id="t">x</text>"#,
    );
    assert!(markup_codes(&extra).contains(&"W808"));

    let twice = screen(
        r#"<fragment name="row"><text id="a">y</text></fragment><fragment name="row"><text id="b">z</text></fragment><text id="t">x</text>"#,
    );
    let parsed = parse_lenient(&twice);
    assert!(parsed.diagnostics.iter().any(|d| d.code == Code::W808));
    assert_eq!(parsed.document.unwrap().fragments.len(), 1);
}

#[test]
fn a_project_name_and_a_fragment_file_cannot_hold_an_inline_fragment() {
    let taken = screen(
        r#"<fragment name="page-header"><text id="a">y</text></fragment><text id="t">x</text>"#,
    );
    let parsed = parse_lenient(&taken);
    assert!(parsed.diagnostics.iter().any(|d| d.code == Code::W808));
    assert!(parsed.document.unwrap().fragments.is_empty());

    let file = r#"<fragment weft="0.2"><fragment name="row"><text id="a">y</text></fragment><text id="t">x</text></fragment>"#;
    let parsed = parse_lenient(file);
    assert!(parsed.diagnostics.iter().any(|d| d.code == Code::W808));
    let doc = parsed.document.unwrap();
    assert!(doc.fragments.is_empty());
    assert!(
        doc.root
            .children
            .iter()
            .all(|c| c.as_node().is_none_or(|n| n.kind != "fragment"))
    );
}

#[test]
fn an_inline_fragment_may_use_another_and_a_cycle_is_w805() {
    let markup = r#"<screen id="s" weft="0.2"><fragment name="a"><use id="inner" fragment="b"/></fragment><fragment name="b"><text id="t">B</text></fragment><use id="top" fragment="a"/></screen>"#;
    let parsed = parse_lenient(markup);
    let doc = parsed.document.unwrap();
    assert!(
        parsed.diagnostics.is_empty(),
        "{:?}\n{}",
        parsed.diagnostics,
        serialize(&doc)
    );
    let expanded = expand(&doc, &catalog());
    assert!(
        expanded.diagnostics.is_empty(),
        "{:?}",
        expanded.diagnostics
    );
    assert!(serialize(&expanded.document).contains(r#"id="top/inner/t""#));

    let cycle = r#"<screen id="s" weft="0.2"><fragment name="a"><use id="x" fragment="b"/></fragment><fragment name="b"><use id="y" fragment="a"/></fragment><use id="top" fragment="a"/></screen>"#;
    assert!(
        parse_lenient(cycle)
            .diagnostics
            .iter()
            .any(|d| d.code == Code::W805)
    );
}

#[test]
fn patches_address_an_inline_fragment_and_add_or_remove_one() {
    let doc = document(&row_screen(
        r#"<use id="sub" fragment="price-row" label="Sub"/>"#,
    ));
    let catalog = catalog();
    let options = ApplyOptions::new(&catalog);
    let set = apply_patches(
        &doc,
        &json!([{"op": "set", "fragment": "price-row", "id": "name", "prop": "text", "value": "Hi"}]),
        &options,
    );
    assert!(set.document.is_some(), "{:?}", set.diagnostics);
    assert!(serialize(set.document.as_ref().unwrap()).contains(r#"text="Hi""#));

    let missing = apply_patches(
        &doc,
        &json!([{"op": "set", "fragment": "price-rw", "id": "name", "prop": "text", "value": "Hi"}]),
        &options,
    );
    assert_eq!(missing.diagnostics[0].code, Code::W502);
    assert!(
        missing.diagnostics[0]
            .hint
            .as_deref()
            .unwrap()
            .contains("price-row")
    );

    let added = apply_patches(
        &document(&screen(r#"<text id="t">x</text>"#)),
        &json!([{"op": "add-fragment", "markup": "<fragment name=\"row\"><text id=\"a\">y</text></fragment>"}]),
        &options,
    );
    assert!(
        added
            .document
            .as_ref()
            .unwrap()
            .fragments
            .contains_key("row"),
        "{:?}",
        added.diagnostics
    );

    let taken = apply_patches(
        &doc,
        &json!([{"op": "add-fragment", "markup": "<fragment name=\"price-row\"><text id=\"a\">y</text></fragment>"}]),
        &options,
    );
    assert_eq!(taken.diagnostics[0].code, Code::W513);

    let mut strict = options;
    strict.mode = Mode::Strict;
    let removed = apply_patches(
        &doc,
        &json!([{"op": "remove-fragment", "name": "price-row"}]),
        &strict,
    );
    assert!(removed.document.is_none());
    assert!(removed.diagnostics.iter().any(|d| d.code == Code::W801));
}

#[test]
fn context_for_names_the_use_not_an_id_inside_the_fragment() {
    let markup = r#"<screen id="s" weft="0.2"><context><entry id="n" by="agent" for="sub" kind="intent" name="m">The row.</entry></context><fragment name="price-row"><text id="name">x</text></fragment><use id="sub" fragment="price-row"/></screen>"#;
    assert!(parse_lenient(markup).diagnostics.is_empty());
    let inside = r#"<screen id="s" weft="0.2"><context><entry id="n" by="agent" for="name" kind="intent" name="m">Inside.</entry></context><fragment name="price-row"><text id="name">x</text></fragment><use id="sub" fragment="price-row"/></screen>"#;
    assert!(markup_codes(inside).contains(&"W309"));
}
