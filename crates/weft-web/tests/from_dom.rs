//! The HTML importer against `fixtures/from-dom.json` (written by packages/from-aria/test/
//! differential.ts through this crate). The fixture began as the results of the TypeScript importer
//! this crate replaced, with the tree htmlparser2 built for each page; the port reproduced every
//! case whose tree html5ever builds the same (all renders, corpus pages and hand-written samples,
//! and 197 of 250 random pages). The rest are pages the HTML standard repairs, where html5ever reads
//! them as a browser does.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use weft_catalog::core_catalog;
use weft_core::{parse_json, to_compact};
use weft_web::from_dom;

const FIXTURE: &str = include_str!("fixtures/from-dom.json");
const SHOWN: usize = 15;

/// Unoptimized builds spend several kilobytes of stack per nesting level, and the fixture holds
/// pages nested past the import limit; release WebAssembly fits them in its 1 MiB.
fn with_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn imports_match_the_typescript_importer_where_the_trees_agree() {
    with_stack(compare_with_fixture);
}

fn compare_with_fixture() {
    let catalog = core_catalog().unwrap();
    let cases = parse_json(FIXTURE).unwrap();
    let cases = cases.as_array().unwrap();
    let mut failures = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let html = case["html"].as_str().unwrap();
        let got = to_compact(&serde_json::to_value(from_dom(html, &catalog)).unwrap());
        let expected = to_compact(&case["result"]);
        if got != expected {
            failures.push(format!(
                "case {i}\n  html: {html:?}\n  expected: {expected}\n  got:      {got}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from the fixture:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any text is a page: the importer answers every input without panicking.
    #[test]
    fn importing_never_panics(html in ".{0,200}") {
        let catalog = core_catalog().unwrap();
        let result = from_dom(&html, &catalog);
        prop_assert!(result.losses.len() >= 5);
    }

    #[test]
    fn importing_markup_never_panics(
        parts in proptest::collection::vec(
            prop_oneof![
                Just("<div>"), Just("</div>"), Just("<table>"), Just("<tr>"), Just("<td>"),
                Just("<label for=a>"), Just("<input id=a>"), Just("<select><option>"),
                Just("<p>"), Just("<template>"), Just("<form>"), Just("<button>"),
                Just("<span data-weft-id=x[1] style='display:grid;gap:1px'>"), Just("text"),
                Just("<div role=tablist><div role=tab id=t aria-selected=true>"),
                Just("<div role=tabpanel aria-labelledby=t>"), Just("<svg><foreignObject>"),
            ],
            0..60,
        )
    ) {
        let catalog = core_catalog().unwrap();
        let _ = from_dom(&parts.concat(), &catalog);
    }
}
