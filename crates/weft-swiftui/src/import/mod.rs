//! Swift source → Weft, with a loss table (SPEC §9, "From SwiftUI"). Source that `generate`
//! printed reads back to the same document with no losses; any other SwiftUI is mapped view by
//! view, and whatever Weft cannot hold is listed as a loss instead of failing. The `weft:source`
//! comment `generate` leaves, with the screen's context, is believed only when the code reads back
//! to the document it claims.
//!
//! The source is untrusted: it is parsed, never compiled or run, its size, element count and
//! nesting are bounded (`W602`), and the result always validates in lenient mode.

mod read;
mod repair;
mod syntax;

use weft_core::{Catalog, Code, Diagnostic, serialize};
pub use weft_import::{ImportResult, Loss, LossKind, MAX_DEPTH, MAX_NODES};
use weft_import::{empty_result, limit_reached, provenance};

use crate::generate::SOURCE_OPTIONS;

/// Longest source read, in bytes; the rest is not imported.
pub const MAX_SOURCE_LENGTH: usize = 2_000_000;

/// Syntax nodes per element and nesting levels per element level that the reader allows: a view
/// with a chain of modifiers is many syntax nodes deep.
const SYNTAX_NODES_PER_ELEMENT: usize = 50;
const SYNTAX_DEPTH_PER_LEVEL: usize = 4;

pub struct ImportOptions<'a> {
    pub catalog: &'a Catalog,
}

/// Reads the first SwiftUI view of `source` (the one no other view in the file uses) into a
/// Weft document.
pub fn import_swiftui(source: &str, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics = vec![];
    let mut text = source;
    if text.len() > MAX_SOURCE_LENGTH {
        let mut end = MAX_SOURCE_LENGTH;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        // Cut at a line end so the parser does not see half a token.
        end = text[..end].rfind('\n').map_or(end, |i| i + 1);
        text = &text[..end];
        limit_reached(
            &mut diagnostics,
            "#",
            &format!("is longer than {MAX_SOURCE_LENGTH} bytes"),
        );
    }
    let parsed = match syntax::parse(
        text,
        MAX_DEPTH * SYNTAX_DEPTH_PER_LEVEL,
        MAX_NODES * SYNTAX_NODES_PER_ELEMENT,
    ) {
        Ok(p) => p,
        Err(syntax::ParseError::Grammar) => {
            diagnostics.push(Diagnostic::new(
                Code::W601,
                "#",
                "The Swift grammar could not be loaded.",
                "Swift source",
            ));
            return empty_result(diagnostics);
        }
    };
    if parsed.truncated {
        limit_reached(
            &mut diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    let Some(mut result) = read::read(&parsed, options.catalog, &mut diagnostics) else {
        diagnostics.push(Diagnostic::new(
            Code::W601,
            "#",
            "The source declares no SwiftUI view: no type that conforms to `View` and has a `body`.",
            "Swift source with a `struct …: View { var body: some View { … } }`",
        ));
        return empty_result(diagnostics);
    };
    if parsed.had_errors {
        result.losses.push(Loss {
            kind: LossKind::Structure,
            path: read::element_path("", &result.document.root),
            note: "the source has Swift syntax errors; the parts the parser could not read are skipped".to_owned(),
        });
    }
    repair::repair(&mut result, options.catalog);
    result.diagnostics.splice(0..0, diagnostics);
    verified(text, &result, options.catalog).unwrap_or(result)
}

/// The document the `weft:source` comment claims, context included, when the code reads back to
/// it with no loss and no diagnostic. Unlike the web and Slint importers, this compares documents
/// rather than regenerated code: the Swift also depends on tokens, appearance and sample data that
/// the importer is not given, and the reader already gives back exactly what `generate` printed
/// (SPEC §9). Context has no other form in the code, so a forged comment on foreign Swift brings
/// context back only when that Swift reads as the claimed screen, as the regenerated code would.
fn verified(source: &str, read: &ImportResult, catalog: &Catalog) -> Option<ImportResult> {
    if !read.losses.is_empty() || !read.diagnostics.is_empty() {
        return None;
    }
    let (options, markup) = provenance::find_lines(source, "//")?;
    if options.split_whitespace().next() != Some(SOURCE_OPTIONS) {
        return None;
    }
    let (document, diagnostics) = provenance::claimed(&markup, catalog)?;
    // The code draws each use expanded (SPEC §10.7) and has no form for context.
    let mut expected = weft_core::expand(&document, catalog).document;
    expected.context.clear();
    expected.weft.clone_from(&read.document.weft);
    (serialize(&expected) == serialize(&read.document)).then(|| ImportResult {
        document,
        losses: Vec::new(),
        diagnostics,
    })
}
