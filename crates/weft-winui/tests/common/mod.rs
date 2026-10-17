//! What the generator tests and the read-back tests both load: the corpus, the catalog examples,
//! the controls fixture, the core catalog and a strict parse. Each integration test compiles this
//! module on its own.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens, token_types};
use weft_core::{Catalog, Document, Mode, ParseOptions, has_errors, parse, parse_json};
use weft_winui::{GenerateOptions, Generated, generate};

/// The screens the generator cannot draw yet: each is refused, every other corpus screen and
/// example generates and has golden files the Windows build compiles (T105.7). T105.3–T105.5
/// shrink this list until it is empty.
pub const PENDING: &[&str] = &[];
/// The namespace of the goldens: the Windows build compiles them in one project (T105.7).
pub const NAMESPACE: &str = "WeftScreens";

pub fn here(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(path)
}

pub fn setup() -> (Catalog, IndexMap<String, Token>) {
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    (core_catalog().unwrap(), tokens)
}

pub fn document(markup: &str, catalog: &Catalog, tokens: &IndexMap<String, Token>) -> Document {
    let types = token_types(tokens);
    let parsed = parse(
        markup,
        &ParseOptions {
            catalog: Some(catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    assert!(!has_errors(&parsed.diagnostics), "{:?}", parsed.diagnostics);
    parsed.document.unwrap()
}

fn weft_files(dir: &Path, nested: bool) -> Vec<PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .map(|p| if nested { p.join("screen.weft") } else { p })
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "weft"))
        .collect();
    out.sort();
    out
}

/// The fixtures of `tests/fixtures/`, each named after its file: every prop the generator draws,
/// the containers the examples leave out, and the data shapes the view model holds.
pub const FIXTURES: &[&str] = &[
    "containers",
    "controls",
    "data",
    "layout",
    "repetition",
    "tokens",
    "values",
];
/// The screens generated with their own tokens in `Page.Resources` rather than the shared
/// `WeftTokens.xaml`, so the Windows build compiles both forms.
pub const OWN_TOKENS: &[&str] = &["tokens"];

/// Every corpus screen and catalog example, named `corpus-<dir>` or `example-<file>`, and the
/// fixtures.
pub fn screens() -> Vec<(String, PathBuf)> {
    let corpus = weft_files(&here("../../corpus"), true)
        .into_iter()
        .map(|p| {
            let dir = p.parent().unwrap().file_name().unwrap().to_string_lossy();
            (format!("corpus-{dir}"), p)
        });
    let examples = weft_files(&here("../../packages/catalog/examples"), false)
        .into_iter()
        .map(|p| {
            let stem = p.file_stem().unwrap().to_string_lossy();
            (format!("example-{stem}"), p)
        });
    let fixtures = FIXTURES
        .iter()
        .map(|f| (f.to_string(), here(&format!("tests/fixtures/{f}.weft"))));
    corpus.chain(examples).chain(fixtures).collect()
}

pub fn options<'a>(
    catalog: &'a Catalog,
    tokens: &'a IndexMap<String, Token>,
    name: Option<&'a str>,
) -> GenerateOptions<'a> {
    GenerateOptions {
        catalog,
        tokens,
        name,
        namespace: NAMESPACE,
        shared_tokens: true,
        appearance: None,
    }
}

/// The controls fixture, parsed.
pub fn controls(catalog: &Catalog, tokens: &IndexMap<String, Token>) -> Document {
    let markup = std::fs::read_to_string(here("tests/fixtures/controls.weft")).unwrap();
    document(&markup, catalog, tokens)
}

/// The covered screens, each generated under its own page name so the goldens never share a class.
pub fn covered() -> Vec<(String, Document, Generated)> {
    let (catalog, tokens) = setup();
    screens()
        .into_iter()
        .filter(|(name, _)| !PENDING.contains(&name.as_str()))
        .map(|(name, path)| {
            let doc = document(&std::fs::read_to_string(path).unwrap(), &catalog, &tokens);
            let options = GenerateOptions {
                shared_tokens: !OWN_TOKENS.contains(&name.as_str()),
                ..options(&catalog, &tokens, Some(&name))
            };
            let generated = generate(&doc, &options).unwrap();
            (name, doc, generated)
        })
        .collect()
}

/// The body of a screen whose paths are C# keywords: a plain one, one already ending in `_`, a
/// contextual one, and a list with a keyword member (T105.17).
pub const KEYWORDS_BODY: &str = r#"<switch id="p" checked="{$.public}" label="P"/><text id="t" text="{$.public_}"/><text id="v" text="{$.value}"/><stack id="st"><each id="e" as="c" in="{$.class}"><text id="n" text="{$c.event}"/></each></stack>"#;

/// A screen around `body`, the shape of every small test document.
pub fn screen(body: &str) -> String {
    format!(r#"<screen id="s" weft="0.2">{body}</screen>"#)
}
