//! What every generator writes for every corpus screen and catalog example, pinned as reviewed
//! insta snapshots under `tests/snapshots/<target>/<screen>.snap`: canonical JSON, the static
//! HTML page, React and SolidJS components as JSX and TSX, and SwiftUI. The differential and
//! round-trip tests prove the outputs agree with each other; these make any change to an output a
//! diff someone reviews (`cargo insta review`) instead of a silent drift.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_core::{Document, stringify};
use weft_swiftui::GenerateOptions;
use weft_web::{Framework, HtmlOptions, JsxOptions, to_html, to_jsx};

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
        };
        // A refusal is an output too: pinning its message keeps the reason reviewed.
        weft_swiftui::generate(document, &options).unwrap_or_else(|e| format!("refused: {e}\n"))
    });
}
