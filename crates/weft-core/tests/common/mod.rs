//! Shared helpers for the integration tests. The fixture catalog is the one the differential test
//! uses, so a claim here and a claim there talk about the same components.

// Each test file uses a different subset.
#![allow(dead_code, clippy::unwrap_used, clippy::panic)]

use std::sync::OnceLock;

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_core::{
    Catalog, Diagnostic, Document, Mode, ParseOptions, ParseResult, ValidateOptions, parse,
    parse_json,
};

const FIXTURE: &str = include_str!("../fixtures/differential.json");

/// Parsed once: the fixture is large and every test asks for the catalog.
pub fn catalog() -> Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let fixture = parse_json(FIXTURE).unwrap();
            serde_json::from_value(fixture["catalogs"]["test"].clone()).unwrap()
        })
        .clone()
}

/// Token path → `$type`, the same set the differential cases use.
pub fn tokens() -> IndexMap<String, String> {
    [
        ("space.sm", "dimension"),
        ("space.md", "dimension"),
        ("color.accent", "color"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect()
}

pub fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

/// Parses with the fixture catalog in lenient mode and no token or action checks.
pub fn parse_lenient(markup: &str) -> ParseResult {
    let catalog = catalog();
    parse(
        markup,
        &ParseOptions {
            catalog: Some(&catalog),
            ..ParseOptions::default()
        },
    )
}

pub fn parse_strict(markup: &str) -> ParseResult {
    let catalog = catalog();
    parse(
        markup,
        &ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            ..ParseOptions::default()
        },
    )
}

/// Codes of the diagnostics for markup, in lenient mode.
pub fn markup_codes(markup: &str) -> Vec<&'static str> {
    codes(&parse_lenient(markup).diagnostics)
}

/// The document of markup that must parse without any diagnostic.
pub fn document(markup: &str) -> Document {
    let result = parse_lenient(markup);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.document.unwrap()
}

/// Codes for JSON validated against the fixture catalog.
pub fn json_codes(input: &Json) -> Vec<&'static str> {
    let catalog = catalog();
    let options = ValidateOptions {
        catalog: Some(&catalog),
        ..ValidateOptions::default()
    };
    codes(&weft_core::validate(input, &options))
}

/// A screen with one child, for tests that need a valid frame around the claim.
pub fn screen(inner: &str) -> String {
    format!("<screen id=\"root\" weft=\"0.1\">{inner}</screen>")
}

/// Runs `f` on a thread with a large stack. Validation recurses once per element level, and a
/// debug build needs more than the 2 MiB of a test thread for a document 256 levels deep.
pub fn on_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}
