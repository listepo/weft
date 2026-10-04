//! The static HTML generator over the corpus: every screen becomes a page that html5ever reads back
//! with every Weft id in place and nothing that runs.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{Document, ParseOptions, parse, parse_json};
use weft_web::{HNode, HtmlOptions, parse_html, to_html};

fn corpus() -> Vec<(String, Document)> {
    let catalog = core_catalog().unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path().join("screen.weft");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let parsed = parse(
                &text,
                &ParseOptions {
                    catalog: Some(&catalog),
                    ..Default::default()
                },
            );
            out.push((path.display().to_string(), parsed.document.unwrap()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn every_corpus_screen_becomes_a_static_page() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let screens = corpus();
    assert!(screens.len() >= 10);
    for (name, document) in screens {
        let options = HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: false,
        };
        let html = to_html(&document, &options).unwrap();
        if std::env::var_os("WEFT_DUMP").is_some() {
            println!("##### {name}\n{html}");
        }
        assert!(html.starts_with("<!doctype html>\n"), "{name}");
        assert!(!html.to_lowercase().contains("javascript:"), "{name}");
        for node in &parse_html(&html).nodes {
            if let HNode::Element {
                name: tag, attrs, ..
            } = node
            {
                assert_ne!(tag, "script", "{name}");
                assert!(!attrs.iter().any(|(k, _)| k.starts_with("on")), "{name}");
            }
        }
        let root = document.root.id.as_deref().unwrap();
        assert!(html.contains(&format!("data-weft-id=\"{root}\"")), "{name}");
    }
}
