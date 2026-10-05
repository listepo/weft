//! Weft → SwiftUI → Weft is the identity for every corpus screen and catalog example, with and
//! without sample data: the document reads back byte-identical after canonical formatting, with
//! no losses. What the generator adds for the data (initializer, `sample`, `#Preview`) is not
//! part of the screen.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_core::serialize;
use weft_swiftui::{GenerateOptions, ImportOptions, generate, import_swiftui};

#[test]
fn every_screen_reads_back_unchanged() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut failures = vec![];
    let screens = common::screens();
    for screen in &screens {
        let document = common::parse_screen(&screen.markup, &catalog, &tokens);
        let mut variants = vec![None];
        if let Some(data) = &screen.data {
            variants.push(Some(data));
        }
        for data in &variants {
            let swift = generate(
                &document,
                &GenerateOptions {
                    catalog: &catalog,
                    tokens: &tokens,
                    name: None,
                    data: *data,
                },
            )
            .unwrap();
            let name = format!(
                "{}{}",
                screen.name,
                if data.is_some() { " with data" } else { "" }
            );
            let result = import_swiftui(&swift, &ImportOptions { catalog: &catalog });
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
    assert!(
        failures.is_empty(),
        "{} of {} screens differ:\n{}",
        failures.len(),
        screens.len(),
        failures.join("\n")
    );
}
