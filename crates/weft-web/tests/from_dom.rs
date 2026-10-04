//! The HTML importer against the TypeScript one: `fixtures/from-dom.json` holds what `fromDom` of
//! @weft/from-aria returned for every case (packages/from-aria/test/differential.ts), with the tree
//! htmlparser2 built. This crate parses with html5ever, which repairs malformed markup the way a
//! browser does and htmlparser2 does not; a result is compared wherever the two trees agree.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use serde_json::{Value as Json, json};
use weft_catalog::core_catalog;
use weft_core::{parse_json, to_compact};
use weft_web::tree::DOCUMENT;
use weft_web::{Dom, HNode, from_dom, parse_html};

const FIXTURE: &str = include_str!("fixtures/from-dom.json");
const SHOWN: usize = 15;
/// Deeper than the import reads, the fixture marks a subtree instead of spelling it out.
const DUMP_DEPTH: usize = 256;

fn dump(dom: &Dom, nodes: &[usize], depth: usize) -> Json {
    if depth > DUMP_DEPTH {
        return json!(["…"]);
    }
    let mut out: Vec<Json> = Vec::new();
    for &n in nodes {
        match &dom.nodes[n] {
            HNode::Text(t) => out.push(json!(t)),
            HNode::Element {
                name,
                attrs,
                children,
            } => out.push(json!([name, attrs, dump(dom, children, depth + 1)])),
        }
    }
    while out
        .last()
        .and_then(Json::as_str)
        .is_some_and(|t| weft_import::js_trim(t).is_empty())
    {
        out.pop();
    }
    Json::Array(out)
}

fn tree(dom: &Dom) -> Json {
    let html = dom
        .elements(DOCUMENT)
        .find(|&e| dom.name(e) == Some("html"));
    let part = |name: &str| {
        html.and_then(|h| dom.elements(h).find(|&e| dom.name(e) == Some(name)))
            .map_or(json!([]), |e| dump(dom, dom.children(e), 0))
    };
    json!({ "head": part("head"), "body": part("body") })
}

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
    let mut fixed_trees = Vec::new();
    let (mut compared, mut samples) = (0, 0);
    for (i, case) in cases.iter().enumerate() {
        let html = case["html"].as_str().unwrap();
        let sample = case["sample"] == json!(true);
        samples += usize::from(sample);
        if to_compact(&tree(&parse_html(html))) != to_compact(&case["tree"]) {
            if !sample {
                fixed_trees.push(i);
            }
            continue;
        }
        compared += 1;
        let got = to_compact(&serde_json::to_value(from_dom(html, &catalog)).unwrap());
        let expected = to_compact(&case["result"]);
        if got != expected {
            failures.push(format!(
                "case {i}\n  html: {html:?}\n  expected: {expected}\n  got:      {got}"
            ));
        }
    }
    eprintln!(
        "{compared} of {} cases compared ({samples} random); trees differ in {} fixed cases: {fixed_trees:?}",
        cases.len(),
        fixed_trees.len()
    );
    // Well-formed markup (renders, corpus pages, hand-written samples) parses the same in both.
    assert!(fixed_trees.is_empty(), "trees differ: {fixed_trees:?}");
    assert!(
        compared * 2 > cases.len(),
        "too few cases compared: {compared}"
    );
    assert!(
        failures.is_empty(),
        "{} of {compared} compared cases differ from the TypeScript importer:\n{}",
        failures.len(),
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
