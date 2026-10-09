//! Weft → SwiftUI → Weft is the identity for every corpus screen, catalog example and example
//! project screen that places no fragment, with the tokens in the screen file and in the shared `WeftTokens`, and with and
//! without sample data: the document reads back byte-identical after canonical formatting, with
//! no losses. What the generator adds for the data (initializer, `sample`, `#Preview`) is not
//! part of the screen.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{Catalog, WEFT_VERSION, serialize};
use weft_swiftui::{GenerateOptions, ImportOptions, generate, import_swiftui};

fn round_trip(
    screens: &[common::Screen],
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
    shared_tokens: bool,
) -> Vec<String> {
    let mut failures = vec![];
    // The generator draws a use's expansion, whose instance paths are no document ids, and
    // importers never write `<use>` (SPEC §10.7): such a screen cannot read back as written.
    for screen in screens.iter().filter(|s| !s.markup.contains("<use ")) {
        let mut document = common::parse_screen(&screen.markup, catalog, tokens);
        // The importer stamps the current version; the corpus stays at 0.1.
        document.weft = WEFT_VERSION.into();
        let mut variants = vec![None];
        if let Some(data) = &screen.data {
            variants.push(Some(data));
        }
        for data in variants {
            let name = format!(
                "{} (shared {shared_tokens}{})",
                screen.name,
                if data.is_some() { ", with data" } else { "" }
            );
            let options = GenerateOptions {
                catalog,
                tokens,
                name: None,
                shared_tokens,
                data,
                appearance: None,
            };
            let swift = match generate(&document, &options) {
                Ok(swift) => swift,
                Err(e) => {
                    failures.push(format!("{name}: {e}"));
                    continue;
                }
            };
            let result = import_swiftui(&swift, &ImportOptions { catalog });
            let expected = serialize(&document);
            let actual = serialize(&result.document);
            if expected != actual {
                failures.push(format!(
                    "{name}:\n--- expected\n{expected}\n--- actual\n{actual}"
                ));
            } else if !result.losses.is_empty() || !result.diagnostics.is_empty() {
                failures.push(format!(
                    "{name}: losses {:?} diagnostics {:?}",
                    result.losses, result.diagnostics
                ));
            }
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
