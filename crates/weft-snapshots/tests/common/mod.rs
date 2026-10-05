//! The screens both tests run over: the corpus and the catalog examples.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens, token_types};
use weft_core::{Catalog, Diagnostic, Document, Mode, ParseOptions, parse, parse_json};

pub struct Screen {
    /// `corpus-login` or `example-button`: unique, and safe as a snapshot file name.
    pub name: String,
    pub path: PathBuf,
    pub markup: String,
}

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn weft_files(dir: &Path, nested: bool) -> Vec<PathBuf> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter_map(|p| {
            if nested {
                Some(p.join("screen.weft")).filter(|f| f.is_file())
            } else {
                Some(p).filter(|f| f.extension().is_some_and(|e| e == "weft"))
            }
        })
        .collect();
    out.sort();
    out
}

pub fn corpus() -> Vec<Screen> {
    weft_files(&root().join("corpus"), true)
        .into_iter()
        .map(|path| {
            let dir = path.parent().unwrap().file_name().unwrap();
            Screen {
                name: format!("corpus-{}", dir.to_string_lossy()),
                markup: std::fs::read_to_string(&path).unwrap(),
                path,
            }
        })
        .collect()
}

pub fn examples() -> Vec<Screen> {
    weft_files(&root().join("packages/catalog/examples"), false)
        .into_iter()
        .map(|path| Screen {
            name: format!("example-{}", path.file_stem().unwrap().to_string_lossy()),
            markup: std::fs::read_to_string(&path).unwrap(),
            path,
        })
        .collect()
}

pub fn catalog() -> Catalog {
    core_catalog().unwrap()
}

pub fn tokens() -> IndexMap<String, Token> {
    load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens
}

/// Parses in strict mode against the core catalog and the default tokens, as `weft check` does.
pub fn parse_strict(
    markup: &str,
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
) -> (Option<Document>, Vec<Diagnostic>) {
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
    (result.document, result.diagnostics)
}

/// A project directory under the repository root, loaded as `weft.json` there says.
pub struct Project {
    pub dir: PathBuf,
    pub catalog: Catalog,
    pub tokens: IndexMap<String, Token>,
}

pub fn project(dir: &str) -> Project {
    let dir = root().join(dir);
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
    let text = std::fs::read_to_string(dir.join("weft.json")).unwrap();
    let options = weft_catalog::ProjectOptions {
        read: Some(&read),
        ..weft_catalog::ProjectOptions::default()
    };
    let load = weft_catalog::load_project_text(&text, &options).unwrap();
    assert!(load.diagnostics.is_empty(), "{:?}", load.diagnostics);
    Project {
        catalog: load.project.catalog,
        tokens: load.project.tokens.unwrap(),
        dir,
    }
}
