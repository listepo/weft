//! What every generator writes for every corpus screen and catalog example, pinned as reviewed
//! insta snapshots under `tests/snapshots/<target>/<screen>.snap`: canonical JSON, the static
//! HTML page, React and SolidJS components as JSX and TSX, and SwiftUI; and for every corpus
//! screen, the static page and SwiftUI generated with its sample data (`html-data`,
//! `swiftui-data`). The differential and
//! round-trip tests prove the outputs agree with each other; these make any change to an output a
//! diff someone reviews (`cargo insta review`) instead of a silent drift.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::Value as Json;
use weft_core::{Document, parse_json, stringify};
use weft_swiftui::GenerateOptions;
use weft_web::{Framework, HtmlOptions, JsxOptions, to_html, to_html_with_data, to_jsx};

fn screens() -> Vec<(String, Document)> {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut out = Vec::new();
    for screen in common::corpus().into_iter().chain(common::examples()) {
        let (document, diagnostics) = common::parse_strict(&screen.markup, &catalog, &tokens);
        let document = document.unwrap_or_else(|| panic!("{}: {diagnostics:?}", screen.name));
        out.push((screen.name, document));
    }
    assert!(out.len() > 40, "found only {} screens", out.len());
    out
}

/// Snapshots `output` of every screen into `tests/snapshots/<target>/<screen>.snap`.
fn snapshot_all(target: &str, output: impl Fn(&Document) -> String) {
    for (name, document) in screens() {
        insta::with_settings!({
            snapshot_path => format!("snapshots/{target}"),
            prepend_module_to_snapshot => false,
            omit_expression => true,
            description => format!("{name} as {target}"),
        }, {
            insta::assert_snapshot!(name.clone(), output(&document));
        });
    }
}

/// Snapshots `output` of every corpus screen with its `data.json` into
/// `tests/snapshots/<target>/<screen>.snap`.
fn snapshot_with_data(target: &str, output: impl Fn(&Document, &Json) -> String) {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut count = 0;
    for screen in common::corpus() {
        let data = screen.path.with_file_name("data.json");
        let data = parse_json(&std::fs::read_to_string(&data).unwrap()).unwrap();
        let (document, _) = common::parse_strict(&screen.markup, &catalog, &tokens);
        let document = document.unwrap();
        insta::with_settings!({
            snapshot_path => format!("snapshots/{target}"),
            prepend_module_to_snapshot => false,
            omit_expression => true,
            description => format!("{} as {target}", screen.name),
        }, {
            insta::assert_snapshot!(screen.name.clone(), output(&document, &data));
        });
        count += 1;
    }
    assert!(count > 10, "found only {count} corpus screens");
}

#[test]
fn canonical_json() {
    snapshot_all("json", stringify);
}

#[test]
fn static_html() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    snapshot_all("html", |document| {
        let options = HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: false,
        };
        to_html(document, &options).unwrap()
    });
}

#[test]
fn static_html_with_data() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    snapshot_with_data("html-data", |document, data| {
        let options = HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: false,
        };
        to_html_with_data(document, &options, Some(data)).unwrap()
    });
}

fn jsx(framework: Framework, typescript: bool) -> impl Fn(&Document) -> String {
    let catalog = common::catalog();
    move |document| {
        let options = JsxOptions {
            catalog: &catalog,
            component_name: None,
            framework,
            typescript,
            source: false,
        };
        to_jsx(&serde_json::to_value(document).unwrap(), &options).unwrap()
    }
}

#[test]
fn react_jsx() {
    snapshot_all("react-jsx", jsx(Framework::React, false));
}

#[test]
fn react_tsx() {
    snapshot_all("react-tsx", jsx(Framework::React, true));
}

#[test]
fn solid_jsx() {
    snapshot_all("solid-jsx", jsx(Framework::Solid, false));
}

#[test]
fn solid_tsx() {
    snapshot_all("solid-tsx", jsx(Framework::Solid, true));
}

#[test]
fn swiftui() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    snapshot_all("swiftui", |document| {
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
            shared_tokens: false,
            data: None,
        };
        // A refusal is an output too: pinning its message keeps the reason reviewed.
        weft_swiftui::generate(document, &options).unwrap_or_else(|e| format!("refused: {e}\n"))
    });
}

#[test]
fn swiftui_with_data() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    snapshot_with_data("swiftui-data", |document, data| {
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
            shared_tokens: false,
            data: Some(data),
        };
        weft_swiftui::generate(document, &options).unwrap_or_else(|e| format!("refused: {e}\n"))
    });
}

/// The shared `WeftTokens.swift`, and screens that read it: a corpus screen with the default
/// tokens, and a project screen that calls the views of its catalog extension's kinds.
#[test]
fn swiftui_shared_tokens() {
    let snapshot = |name: &str, output: String| {
        insta::with_settings!({
            snapshot_path => "snapshots/swiftui-shared",
            prepend_module_to_snapshot => false,
            omit_expression => true,
            description => format!("{name} as SwiftUI with shared tokens"),
        }, {
            insta::assert_snapshot!(name.to_owned(), output);
        });
    };
    let catalog = common::catalog();
    let tokens = common::tokens();
    snapshot("WeftTokens", weft_swiftui::generate_tokens(&tokens).0);
    let login = screens()
        .into_iter()
        .find(|(n, _)| n == "corpus-login")
        .unwrap()
        .1;
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
        shared_tokens: true,
        data: None,
    };
    snapshot(
        "corpus-login",
        weft_swiftui::generate(&login, &options).unwrap(),
    );

    let project = common::project("crates/weft-swiftui/tests/fixtures/project");
    let (swift, skipped) = weft_swiftui::generate_tokens(&project.tokens);
    assert!(skipped.is_empty(), "{skipped:?}");
    snapshot("project-WeftTokens", swift);
    let markup = std::fs::read_to_string(project.dir.join("screens/offers.weft")).unwrap();
    let (document, diagnostics) = common::parse_strict(&markup, &project.catalog, &project.tokens);
    let document = document.unwrap_or_else(|| panic!("{diagnostics:?}"));
    // The sample initializer and `#Preview` pass the data through the custom views too.
    let data = std::fs::read_to_string(project.dir.join("sample.data.json")).unwrap();
    let data = parse_json(&data).unwrap();
    let options = GenerateOptions {
        catalog: &project.catalog,
        tokens: &project.tokens,
        name: None,
        shared_tokens: true,
        data: Some(&data),
    };
    snapshot(
        "project-offers",
        weft_swiftui::generate(&document, &options).unwrap(),
    );
}
