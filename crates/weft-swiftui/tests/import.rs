//! SwiftUI that `weft swiftui` did not print: a hand-written screen imports to a pinned document
//! with pinned losses, and any source at all imports without panicking to a document that
//! validates in lenient mode (SPEC §9). Regenerate the pins with WEFT_UPDATE_FIXTURES=1.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, RngSeed};
use weft_core::{Mode, ValidateOptions, has_errors, serialize, validate_document};
use weft_swiftui::{ImportOptions, ImportResult, import_swiftui};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/import");

fn import(source: &str) -> ImportResult {
    import_swiftui(
        source,
        &ImportOptions {
            catalog: &common::catalog(),
        },
    )
}

fn assert_lenient_valid(result: &ImportResult) {
    let diagnostics = validate_document(
        &result.document,
        &ValidateOptions {
            catalog: Some(&common::catalog()),
            mode: Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    assert!(!has_errors(&diagnostics), "{diagnostics:#?}");
}

/// Compares `actual` with the pinned file, or rewrites the pin when asked to.
fn pinned(name: &str, actual: &str) {
    let path = format!("{FIXTURES}/{name}");
    if std::env::var("WEFT_UPDATE_FIXTURES").as_deref() == Ok("1") {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        actual, expected,
        "{name} changed; regenerate with WEFT_UPDATE_FIXTURES=1"
    );
}

#[test]
fn a_hand_written_screen_imports_with_the_expected_losses() {
    let source = std::fs::read_to_string(format!("{FIXTURES}/Settings.swift")).unwrap();
    let result = import(&source);
    assert!(result.diagnostics.is_empty(), "{:#?}", result.diagnostics);
    assert_lenient_valid(&result);
    pinned("Settings.weft", &serialize(&result.document));
    let losses = serde_json::to_string_pretty(&result.losses).unwrap() + "\n";
    pinned("Settings.losses.json", &losses);
}

#[test]
fn source_without_a_view_is_unreadable() {
    let result = import("let x = 1\n");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code.as_str(), "W601");
    assert_lenient_valid(&result);
}

fn config() -> Config {
    Config {
        cases: 256,
        max_shrink_iters: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x5_1f7),
        ..Config::default()
    }
}

/// Pieces of SwiftUI, joined in any order: mostly broken source that still reaches the reader.
fn fragment() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("struct V: View {"),
        Just("var body: some View {"),
        Just("@Bindable var model: M"),
        Just("@Observable final class M { var a = \"\"; var items: [String] = [] }"),
        Just("enum A: String { case go = \"x.go\" }"),
        Just("VStack(alignment: .leading) {"),
        Just("HStack {"),
        Just("List {"),
        Just("Form {"),
        Just("TabView(selection: $model.a) {"),
        Just("Picker(\"p\", selection: $model.a) {"),
        Just("ForEach(model.items, id: \\.self) { item in"),
        Just("ForEach(Array(model.items.enumerated()), id: \\.offset) { i, item in"),
        Just("if model.a.isEmpty {"),
        Just("} else {"),
        Just("Text(\"{$.x}\")"),
        Just("Text(item)"),
        Just("Text(\"a \\(model.a)\")"),
        Just("Button(\"b\") { send(.go) }"),
        Just("TextField(\"t\", text: $model.a)"),
        Just("Toggle(\"t\", isOn: .constant(true))"),
        Just("V()"),
        Just(".accessibilityIdentifier(\"dup\")"),
        Just(".weftProp(\"variant\", \"primary\")"),
        Just(".sheet(isPresented: $model.open) {"),
        Just(".padding()"),
        Just("{"),
        Just("}"),
        Just("("),
        Just(")"),
        Just("\""),
        Just("$"),
    ]
    .prop_map(String::from)
}

fn source() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        prop::collection::vec(fragment(), 0..40).prop_map(|parts| parts.join("\n")),
        prop::collection::vec(fragment(), 0..40).prop_map(|parts| format!(
            "import SwiftUI\nstruct S: View {{\n@Bindable var model: M\nvar body: some View {{\n{}\n}}\n}}\n",
            parts.join("\n")
        )),
    ]
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn any_source_imports_to_a_lenient_valid_document(source in source()) {
        let result = import(&source);
        assert_lenient_valid(&result);
    }
}
