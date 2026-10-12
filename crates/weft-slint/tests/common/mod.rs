//! What the generator tests and the screenshot tests both load: the corpus, the core catalog,
//! a strict parse and the headless Slint backend. Each integration test compiles this module on
//! its own.

#![allow(dead_code)]

use std::path::PathBuf;

use indexmap::IndexMap;
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens, token_types};
use weft_core::{Catalog, Document, Mode, ParseOptions, has_errors, parse, parse_json};

pub fn here(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

pub fn screens() -> Vec<(String, PathBuf)> {
    let mut found = Vec::new();
    let mut dirs: Vec<_> = std::fs::read_dir(here("../../corpus"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("screen.weft").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        found.push((name, dir.join("screen.weft")));
    }
    found.push(("controls".to_owned(), here("tests/fixtures/controls.weft")));
    found
}

pub fn setup() -> (Catalog, IndexMap<String, Token>) {
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    (core_catalog().unwrap(), tokens)
}

pub fn document(markup: &str, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> Document {
    let types = token_types(tokens);
    let options = ParseOptions {
        catalog: Some(catalog),
        mode: Mode::Strict,
        tokens: Some(&types),
        actions: None,
    };
    let parsed = parse(markup, &options);
    assert!(!has_errors(&parsed.diagnostics), "{:?}", parsed.diagnostics);
    parsed.document.unwrap()
}

pub fn exported_component(source: &str) -> &str {
    source
        .lines()
        .find_map(|line| {
            let rest = line.strip_prefix("export component ")?;
            rest.split_whitespace().next()
        })
        .unwrap()
}

/// One backend per thread. `threading: false` keeps the platform thread-local, so parallel tests
/// do not share it; a second call on the same thread finds it already set.
pub fn ensure_backend() {
    i_slint_core::with_global_context(
        || {
            let backend = i_slint_backend_testing::TestingBackend::new(
                i_slint_backend_testing::TestingBackendOptions {
                    mock_time: true,
                    threading: false,
                    renderer_name: Some("software".into()),
                },
            );
            Ok(Box::new(backend) as Box<dyn i_slint_core::platform::Platform>)
        },
        |ctx| ctx.set_color_scheme(i_slint_core::items::ColorScheme::Light),
    )
    .unwrap();
}
