//! The number, date and colour controls in SwiftUI: a slider's range never reverses (a
//! `ClosedRange` with its bounds the wrong way round traps at run time), a bound that comes from
//! data goes through a helper that applies the renderer's rules, and whole and fractional numbers
//! keep their own Swift types.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_swiftui::{GenerateOptions, ImportOptions, generate, import_swiftui};

fn swift(markup: &str, data: Option<&serde_json::Value>) -> String {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let document = common::parse_screen(markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
        shared_tokens: false,
        data,
        appearance: None,
    };
    generate(&document, &options).unwrap()
}

fn screen(body: &str) -> String {
    format!("<screen id=\"root\" label=\"Test\" weft=\"0.2\">\n  {body}\n</screen>\n")
}

#[test]
fn a_literal_range_is_never_reversed() {
    let out = swift(
        &screen("<slider id=\"s\" label=\"S\" max=\"5\" min=\"10\" value=\"7\"/>"),
        None,
    );
    assert!(out.contains("in: 10...10,"), "{out}");
    let out = swift(
        &screen("<stepper id=\"s\" label=\"S\" max=\"1\" min=\"3\" value=\"2\"/>"),
        None,
    );
    assert!(out.contains("in: 3...3"), "{out}");
}

#[test]
fn a_bound_or_missing_bound_goes_through_the_helpers() {
    let out = swift(
        &screen(
            "<slider id=\"s\" label=\"S\" max=\"{$.top}\" min=\"2\" step=\"0\" value=\"{$.at}\"/>",
        ),
        None,
    );
    assert!(out.contains("in: weftRange(2, model.top)"), "{out}");
    assert!(out.contains("step: weftStep(0)"), "{out}");
    let out = swift(
        &screen("<stepper id=\"s\" label=\"S\" min=\"4\" value=\"{$.n}\"/>"),
        None,
    );
    assert!(out.contains("in: weftBounds(4, nil)"), "{out}");
}

#[test]
fn a_fractional_number_is_a_double() {
    let data = serde_json::json!({ "zoom": 1.5 });
    let out = swift(
        &screen("<stepper id=\"z\" label=\"Zoom\" step=\"0.5\" value=\"{$.zoom}\"/>"),
        Some(&data),
    );
    assert!(out.contains("var zoom: Double = 0"), "{out}");
    assert!(out.contains("zoom: 1.5"), "{out}");
    assert!(out.contains("step: 0.5"), "{out}");
}

#[test]
fn a_path_cannot_be_a_number_and_text() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let markup = screen(
        "<stack id=\"a\"><slider id=\"s\" label=\"S\" value=\"{$.x}\"/><field id=\"f\" label=\"F\" value=\"{$.x}\"/></stack>",
    );
    let document = common::parse_screen(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
        shared_tokens: false,
        data: None,
        appearance: None,
    };
    let error = generate(&document, &options).unwrap_err().to_string();
    assert!(error.contains("number and as text"), "{error}");
}

#[test]
fn explicit_and_absent_values_read_back_apart() {
    for body in [
        "<slider id=\"s\" label=\"S\" value=\"0\"/>",
        "<slider id=\"s\" label=\"S\"/>",
        "<slider id=\"s\" label=\"S\" max=\"100\" min=\"0\" step=\"1\" value=\"3\"/>",
        "<stepper id=\"s\" label=\"S\" step=\"1\" value=\"0\"/>",
        "<date-picker id=\"s\" label=\"S\" type=\"date\" value=\"2026-02-03\"/>",
        "<date-picker id=\"s\" label=\"S\" value=\"2026-02-03\"/>",
    ] {
        let catalog = common::catalog();
        let markup = screen(body);
        let source = swift(&markup, None);
        let result = import_swiftui(&source, &ImportOptions { catalog: &catalog });
        assert_eq!(weft_core::serialize(&result.document), markup, "{body}");
    }
}

// SPEC §5.1: a row without `align` centres its children, which is what `HStack` does on its own.
// The generator must therefore print no alignment for it, and an `HStack` must read back without
// one, or the two targets would stop agreeing the day SwiftUI's default changes.
#[test]
fn a_row_without_align_is_a_plain_hstack_both_ways() {
    let row = screen(
        "<stack id=\"r\" direction=\"row\">\n    <field id=\"f\" label=\"F\"/>\n    <button id=\"b\">Go</button>\n  </stack>",
    );
    let out = swift(&row, None);
    assert!(
        out.contains("HStack {") && !out.contains("HStack(alignment"),
        "{out}"
    );
    let catalog = common::catalog();
    let back =
        weft_core::serialize(&import_swiftui(&out, &ImportOptions { catalog: &catalog }).document);
    assert_eq!(back, row);
}

#[test]
fn an_explicit_row_alignment_is_kept() {
    let out = swift(
        &screen(
            "<stack id=\"r\" align=\"end\" direction=\"row\"><button id=\"b\">Go</button></stack>",
        ),
        None,
    );
    assert!(out.contains("HStack(alignment: .bottom)"), "{out}");
}
