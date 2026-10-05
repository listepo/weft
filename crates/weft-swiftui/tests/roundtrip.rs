//! Weft → SwiftUI → Weft is the identity for every corpus screen, catalog example and example
//! project screen, with the tokens in the screen file and in the shared `WeftTokens`: the document
//! reads back byte-identical after canonical formatting, with no losses.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{Catalog, serialize};
use weft_swiftui::{GenerateOptions, ImportOptions, generate, import_swiftui};

fn round_trip(
    screens: &[common::Screen],
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
    shared_tokens: bool,
) -> Vec<String> {
    let mut failures = vec![];
    for screen in screens {
        let document = common::parse_screen(&screen.markup, catalog, tokens);
        let options = GenerateOptions {
            catalog,
            tokens,
            name: None,
            shared_tokens,
        };
        let swift = match generate(&document, &options) {
            Ok(swift) => swift,
            Err(e) => {
                failures.push(format!("{} (shared {shared_tokens}): {e}", screen.name));
                continue;
            }
        };
        let result = import_swiftui(&swift, &ImportOptions { catalog });
        let expected = serialize(&document);
        let actual = serialize(&result.document);
        if expected != actual {
            failures.push(format!(
                "{} (shared {shared_tokens}):\n--- expected\n{expected}\n--- actual\n{actual}",
                screen.name
            ));
        } else if !result.losses.is_empty() || !result.diagnostics.is_empty() {
            failures.push(format!(
                "{} (shared {shared_tokens}): losses {:?} diagnostics {:?}",
                screen.name, result.losses, result.diagnostics
            ));
        }
    }
    failures
}

#[test]
fn every_screen_reads_back_unchanged() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let screens = common::screens();
    let projects = common::projects();
    let mut failures = vec![];
    for shared in [false, true] {
        failures.extend(round_trip(&screens, &catalog, &tokens, shared));
        for p in &projects {
            failures.extend(round_trip(&p.screens, &p.catalog, &p.tokens, shared));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} screens differ:\n{}",
        failures.len(),
        2 * (screens.len() + projects.iter().map(|p| p.screens.len()).sum::<usize>()),
        failures.join("\n")
    );
}
