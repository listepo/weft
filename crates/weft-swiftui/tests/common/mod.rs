//! The screens every test runs over: the corpus and the catalog examples.

#![allow(dead_code)]

use std::path::PathBuf;

use indexmap::IndexMap;
use weft_catalog::{Token, core_catalog, load_tokens, token_types};
use weft_core::{Catalog, Document, Mode, ParseOptions, parse, parse_json};

pub struct Screen {
    /// A unique name: `corpus/login` or `examples/button`.
    pub name: String,
    pub markup: String,
    /// The screen's sample data (`data.json` of a corpus screen).
    pub data: Option<serde_json::Value>,
}

pub fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Screens the generator refuses by design: an array-index binding path has no SwiftUI model
/// property to bind (`generate` says to iterate with `<each>` instead). `swift.rs` checks the
/// refusal; every other test skips them.
pub const REFUSED: &[&str] = &["corpus/leaderboard"];

/// Every screen, including the ones in [`REFUSED`].
pub fn all_screens() -> Vec<Screen> {
    let mut out = vec![];
    let mut corpus: Vec<_> = std::fs::read_dir(root().join("corpus"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("screen.weft").is_file())
        .collect();
    corpus.sort();
    for dir in corpus {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        out.push(Screen {
            name: format!("corpus/{name}"),
            markup: std::fs::read_to_string(dir.join("screen.weft")).unwrap(),
            data: std::fs::read_to_string(dir.join("data.json"))
                .ok()
                .map(|text| parse_json(&text).unwrap()),
        });
    }
    let mut examples: Vec<_> = std::fs::read_dir(root().join("packages/catalog/examples"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "weft"))
        .collect();
    examples.sort();
    for path in examples {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        out.push(Screen {
            name: format!("examples/{name}"),
            markup: std::fs::read_to_string(&path).unwrap(),
            data: None,
        });
    }
    let mut fixtures: Vec<_> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "weft"))
            .collect();
    fixtures.sort();
    for path in fixtures {
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        out.push(Screen {
            name: format!("fixtures/{name}"),
            markup: std::fs::read_to_string(&path).unwrap(),
            data: None,
        });
    }
    assert!(out.len() > 30, "found only {} screens", out.len());
    out
}

/// The screens the generator supports.
pub fn screens() -> Vec<Screen> {
    all_screens()
        .into_iter()
        .filter(|s| !REFUSED.contains(&s.name.as_str()))
        .collect()
}

pub fn catalog() -> Catalog {
    core_catalog().unwrap()
}

pub fn tokens() -> IndexMap<String, Token> {
    let json = parse_json(weft_swiftui::DEFAULT_TOKENS_JSON).unwrap();
    load_tokens(&json).tokens
}

pub fn parse_screen(markup: &str, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> Document {
    let types = token_types(tokens);
    let result = parse(
        markup,
        &ParseOptions {
            catalog: Some(catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    result.document.unwrap()
}

/// `corpus/login` → `CorpusLogin`, so every screen gets its own type names in one module.
pub fn type_name(name: &str) -> String {
    name.replace('/', "-")
}
