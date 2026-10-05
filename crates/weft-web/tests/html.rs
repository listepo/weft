//! The static HTML generator over the corpus: every screen becomes a page that html5ever reads back
//! with every Weft id in place and nothing that runs.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{Document, ParseOptions, has_errors, parse, parse_json, serialize};
use weft_web::{HNode, HtmlOptions, ImportOptions, import_html, parse_html, to_html};

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
            appearance: None,
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

fn import_options<'a>(
    catalog: &'a weft_core::Catalog,
    tokens: &'a indexmap::IndexMap<String, weft_catalog::Token>,
) -> ImportOptions<'a> {
    ImportOptions { catalog, tokens }
}

#[test]
fn generated_pages_come_back_exactly() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    for (name, document) in corpus() {
        let options = HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: true,
            appearance: None,
        };
        let html = to_html(&document, &options).unwrap();
        let back = import_html(&html, &import_options(&catalog, &tokens));
        assert!(back.losses.is_empty(), "{name}: {:?}", back.losses);
        assert_eq!(serialize(&back.document), serialize(&document), "{name}");
    }
}

/// Known gaps of the convention import, found by the coverage screens (T36). Each screen here
/// must still differ, so a fix fails the test until its entry is removed.
const HTML_CONVENTION_GAPS: &[(&str, &str)] = &[
    (
        "account",
        "explicit type=\"text\" is dropped; a radio's bound text also comes back as a bound label",
    ),
    (
        "appearance",
        "explicit type=\"date\" is dropped; a segment's bound text also comes back as a bound label",
    ),
    ("dashboard", "an explicit direction=\"column\" is dropped"),
    (
        "inbox",
        "the tabs selected binding and on-change are lost; a bound tab label comes back empty; \
         dialog modal=\"false\" is dropped",
    ),
];

#[test]
fn generated_pages_without_their_source_come_back_by_convention() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let mut failures = Vec::new();
    for (name, document) in corpus() {
        let options = HtmlOptions {
            catalog: &catalog,
            tokens: &tokens,
            source: false,
            appearance: None,
        };
        let html = to_html(&document, &options).unwrap();
        let back = import_html(&html, &import_options(&catalog, &tokens));
        let got = serialize(&back.document);
        let want = serialize(&document);
        let differs = !back.losses.is_empty() || got != want || has_errors(&back.diagnostics);
        let known = HTML_CONVENTION_GAPS
            .iter()
            .any(|(gap, _)| name.contains(&format!("/corpus/{gap}/")));
        if known && !differs {
            failures.push(format!("{name}: the known gap is fixed; remove it"));
        } else if differs && !known {
            failures.push(format!(
                "##### {name}\n{got}\n--- want\n{want}\n{:#?}\n{:?}",
                back.losses, back.diagnostics
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const TILTED: &str = r#"<screen id="s" weft="0.1"><stack id="a" gap="{token.space.md}" perspective="800" rotate-y="30"><button id="b" rotate-z="-5.5">Go</button></stack></screen>"#;

fn tilted() -> Document {
    let catalog = core_catalog().unwrap();
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..Default::default()
    };
    parse(TILTED, &options).document.unwrap()
}

#[test]
fn a_tilt_is_a_transform_next_to_the_layout_style() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = HtmlOptions {
        catalog: &catalog,
        tokens: &tokens,
        source: false,
        appearance: None,
    };
    let html = to_html(&tilted(), &options).unwrap();
    assert!(
        html.contains(
            "style=\"gap: var(--weft-space-md); transform: perspective(800px) rotateY(30deg)\""
        ),
        "{html}"
    );
    assert!(
        html.contains("style=\"transform: rotateZ(-5.5deg)\""),
        "{html}"
    );
    let back = import_html(&html, &import_options(&catalog, &tokens));
    assert!(back.losses.is_empty(), "{:?}", back.losses);
    assert_eq!(serialize(&back.document), serialize(&tilted()));
}

#[test]
fn a_transform_that_is_not_a_tilt_is_a_loss() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let html = r#"<main data-weft-id="s"><div data-weft-id="a" style="transform: translateX(4px)"></div></main>"#;
    let back = import_html(html, &import_options(&catalog, &tokens));
    assert!(
        back.losses
            .iter()
            .any(|l| l.note.contains("transform: translateX(4px)")),
        "{:?}",
        back.losses
    );
}
