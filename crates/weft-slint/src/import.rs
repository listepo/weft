//! Slint → Weft for files the generator printed: the document comes from the leading
//! `// weft:source slint` comment (SPEC §9, Provenance), and is believed only when generating
//! from it gives the file back. A file this rejects is what `read_slint` reads.

use indexmap::IndexMap;
use weft_catalog::{Token, token_types};
use weft_core::{Catalog, Diagnostic, Document, Mode, ParseOptions, has_errors, parse};

use crate::generate::{GenerateOptions, MARKER, generate};

/// The longest source the importer reads, as the SwiftUI and A2UI importers bound theirs.
pub const MAX_SOURCE_LENGTH: usize = 2_000_000;

pub struct ImportOptions<'a> {
    pub catalog: &'a Catalog,
    /// The tokens the file was generated with: a `gap` token is compared by its px value.
    pub tokens: &'a IndexMap<String, Token>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the source is longer than {MAX_SOURCE_LENGTH} bytes")]
    TooLong,
    #[error("no leading `{MARKER}` comment; reading hand-written Slint is not supported yet")]
    NoSource,
    #[error("the source comment is not a valid Weft document ({} diagnostics)", .0.len())]
    Invalid(Vec<Diagnostic>),
    #[error(
        "the file is not what its source comment generates: it was edited, or other tokens apply"
    )]
    Edited,
}

/// Lines compared trimmed and without blank ones, so a reindented file still matches.
fn normalized(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn import_slint(source: &str, options: &ImportOptions<'_>) -> Result<Document, ImportError> {
    if source.len() > MAX_SOURCE_LENGTH {
        return Err(ImportError::TooLong);
    }
    let mut lines = source.lines().skip_while(|l| l.trim().is_empty());
    let head = lines
        .next()
        .and_then(|l| l.trim().strip_prefix(MARKER))
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
        .ok_or(ImportError::NoSource)?;
    let name = head
        .split_whitespace()
        .find_map(|w| w.strip_prefix("name="));
    let markup: Vec<&str> = lines
        .map(str::trim_start)
        .map_while(|l| l.strip_prefix("// ").or((l == "//").then_some("")))
        .collect();
    let types = token_types(options.tokens);
    let parsed = parse(
        &markup.join("\n"),
        &ParseOptions {
            catalog: Some(options.catalog),
            mode: Mode::Strict,
            tokens: Some(&types),
            actions: None,
        },
    );
    let document = match parsed.document {
        Some(d) if !has_errors(&parsed.diagnostics) => d,
        _ => return Err(ImportError::Invalid(parsed.diagnostics)),
    };
    let again = generate(
        &document,
        &GenerateOptions {
            catalog: options.catalog,
            tokens: options.tokens,
            name,
        },
    )
    .map_err(|_| ImportError::Edited)?;
    if normalized(&again) != normalized(source) {
        return Err(ImportError::Edited);
    }
    Ok(document)
}
