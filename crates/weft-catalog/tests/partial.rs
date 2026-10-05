//! Streaming (SPEC §6.3): every corpus screen and catalog example, cut after every character,
//! reads as a finished prefix document whose only diagnostics are the pending ones of the cut.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use weft_catalog::{core_catalog, load_tokens, token_types};
use weft_core::{Code, Mode, ParseOptions, parse, parse_json, parse_partial, stringify};

/// What a cut can explain: an element, comment or document that is not finished, a reference or
/// slot that a later chunk can still supply.
const TAIL: [Code; 6] = [
    Code::W110,
    Code::W114,
    Code::W115,
    Code::W208,
    Code::W309,
    Code::W314,
];

fn screens() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut out = vec![];
    for entry in std::fs::read_dir(root.join("corpus")).unwrap() {
        let path = entry.unwrap().path().join("screen.weft");
        if path.exists() {
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    for entry in std::fs::read_dir(root.join("packages/catalog/examples")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "weft") {
            out.push((
                path.display().to_string(),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    assert!(out.len() >= 19, "found only {} screens", out.len());
    out
}

fn nodes(json: &str) -> usize {
    json.matches("\"kind\"").count()
}

#[test]
fn every_prefix_of_every_screen_is_a_valid_partial_document() {
    let catalog = core_catalog().unwrap();
    let tokens_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/catalog/tokens/default.tokens.json"
    ))
    .unwrap();
    let tokens = token_types(&load_tokens(&parse_json(&tokens_json).unwrap()).tokens);
    let options = ParseOptions {
        catalog: Some(&catalog),
        mode: Mode::Strict,
        tokens: Some(&tokens),
        actions: None,
    };
    for (name, markup) in screens() {
        let whole = parse(&markup, &options);
        assert!(
            whole.diagnostics.is_empty(),
            "{name}: {:?}",
            whole.diagnostics
        );
        let root_tag_end = markup.find('>').unwrap() + 1;
        let mut previous = 0;
        let cuts = markup.char_indices().map(|(i, _)| i).chain([markup.len()]);
        for cut in cuts {
            let prefix = &markup[..cut];
            let r = parse_partial(prefix, &options);
            let at = format!("{name} cut at {cut}: {prefix:?}");
            assert!(r.diagnostics.is_empty(), "{at}: {:?}", r.diagnostics);
            for d in &r.pending {
                assert!(TAIL.contains(&d.code), "{at}: {d:?}");
            }
            let Some(document) = &r.document else {
                assert!(cut < root_tag_end, "{at}: no document");
                assert!(!r.pending.is_empty(), "{at}: nothing pending");
                continue;
            };
            assert!(cut >= root_tag_end, "{at}: document before the root tag");
            // The prefix only ever grows: a later chunk never removes an element.
            let count = nodes(&stringify(document));
            assert!(count >= previous, "{at}: {count} nodes after {previous}");
            previous = count;
            if cut == markup.len() {
                assert!(r.pending.is_empty(), "{at}: {:?}", r.pending);
                let whole = whole.document.as_ref().map(stringify);
                assert_eq!(Some(stringify(document)), whole, "{at}");
            }
        }
    }
}

#[test]
fn what_a_later_chunk_can_supply_is_pending_not_an_error() {
    let catalog = core_catalog().unwrap();
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let head =
        "<screen id=\"s\" weft=\"0.1\"><list id=\"l\"><each id=\"e\" as=\"x\" in=\"{$.xs}\">";
    let r = parse_partial(head, &options);
    assert!(r.document.is_some());
    assert!(r.diagnostics.is_empty(), "{:?}", r.diagnostics);
    assert!(r.pending.iter().any(|d| d.code == Code::W314));
    // Once the element is closed without a child, the same check is an error.
    let r = parse_partial(&format!("{head}</each>"), &options);
    assert!(r.diagnostics.iter().any(|d| d.code == Code::W314));
}
