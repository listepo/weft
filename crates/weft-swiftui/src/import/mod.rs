//! Swift source → Weft, with a loss table (SPEC §9, "From SwiftUI"). Source that `generate`
//! printed reads back to the same document with no losses; any other SwiftUI is mapped view by
//! view, and whatever Weft cannot hold is listed as a loss instead of failing.
//!
//! The source is untrusted: it is parsed, never compiled or run, its size, element count and
//! nesting are bounded (`W602`), and the result always validates in lenient mode.

mod read;
mod repair;
mod syntax;

use serde::Serialize;
use weft_core::{Catalog, Code, Diagnostic, Document, Node, WEFT_VERSION};

/// Longest source read, in bytes; the rest is not imported.
pub const MAX_SOURCE_LENGTH: usize = 2_000_000;
/// Most elements imported.
pub const MAX_NODES: usize = 20_000;
/// Deepest element nesting imported.
pub const MAX_DEPTH: usize = 200;

/// Syntax nodes per element and nesting levels per element level that the reader allows: a view
/// with a chain of modifiers is many syntax nodes deep.
const SYNTAX_NODES_PER_ELEMENT: usize = 50;
const SYNTAX_DEPTH_PER_LEVEL: usize = 4;

pub struct ImportOptions<'a> {
    pub catalog: &'a Catalog,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub document: Document,
    pub losses: Vec<Loss>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Loss {
    pub kind: LossKind,
    /// The element path (SPEC §6.1) the loss is about.
    pub path: String,
    pub note: String,
}

/// The loss kinds of SPEC §9.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LossKind {
    Ids,
    Bindings,
    Actions,
    Tokens,
    Layout,
    Repetition,
    Slots,
    Hidden,
    Props,
    Values,
    Names,
    Kinds,
    Text,
    Structure,
}

fn limit_reached(diagnostics: &mut Vec<Diagnostic>, what: &str) {
    diagnostics.push(Diagnostic::new(
        Code::W602,
        "#",
        format!("The input {what}; the rest was not imported."),
        format!("at most {MAX_SOURCE_LENGTH} bytes, {MAX_NODES} elements, nested at most {MAX_DEPTH} levels deep"),
    ));
}

fn empty_result(diagnostics: Vec<Diagnostic>) -> ImportResult {
    let mut root = Node::new("screen");
    root.id = Some("screen".to_owned());
    ImportResult {
        document: Document {
            weft: WEFT_VERSION.to_owned(),
            root,
        },
        losses: vec![Loss {
            kind: LossKind::Structure,
            path: "/screen#screen".to_owned(),
            note: "nothing could be imported".to_owned(),
        }],
        diagnostics,
    }
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
    result
}
