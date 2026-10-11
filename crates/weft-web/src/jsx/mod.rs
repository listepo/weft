//! Weft document → a self-contained React or SolidJS component, or a Lit element (SPEC §9, "To
//! JSX"). The mapping
//! restates `@weft/render-react` kind by kind rather than sharing it: the renderer builds elements
//! with closures over resolved values, while this module prints source text, so a shared table
//! would only cover tag names. packages/to-jsx/test/equivalence.test.ts renders both on every
//! corpus screen and catalog example and compares the results, which is what keeps the two from
//! drifting. React, SolidJS and Lit share one render plan; they differ only in how a loop, a
//! condition, a dynamic tag and a few attributes are spelled, and Lit prints the element tree as
//! `html` templates (`lit.rs`) instead of JSX.
//!
//! The document is untrusted. No document string ever reaches the output except through
//! `JSON.stringify` (a JavaScript string literal) or a JSX text/attribute run whose characters are
//! all inert; identifiers come only from this module or are checked against the loop-variable
//! grammar.

mod gen_collections;
mod gen_core;
mod gen_module;
mod gen_pieces;
mod gen_render;
mod gen_widgets;
mod helpers;
mod lit;
mod model;
mod names;
mod print;
mod runtime;
mod tree;
mod value;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

use serde_json::Value as Json;
use weft_core::Catalog;

use tree::C;

pub use runtime::{Helper, RUNTIME, react_runtime};

pub(crate) use model::N;
pub(crate) use value::clamp_number;

use model::{Ctx, Piece, Scope};

/// Same bound as the renderer's expansion: deeper content is not rendered.
pub const MAX_DEPTH: usize = 200;

/// The framework a component is generated for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Framework {
    #[default]
    React,
    Solid,
    /// A `LitElement` that renders into the light DOM, so the page's stylesheet reaches it as it
    /// does the React and SolidJS output.
    Lit,
}

#[derive(Clone, Copy, Debug)]
pub struct JsxOptions<'a> {
    pub catalog: &'a Catalog,
    /// `WeftScreen` when absent; must match `[A-Z][A-Za-z0-9]*`.
    pub component_name: Option<&'a str>,
    pub framework: Framework,
    /// TSX: typed props and helpers.
    pub typescript: bool,
    /// Leave the canonical source in a leading comment, so `import_jsx` gives the document back.
    pub source: bool,
}

/// What the generator refuses: a component name the output could not carry safely, or an option
/// the Lit target does not offer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsxError {
    BadComponentName,
    /// The Lit target has no TypeScript flavour and no `weft:source` comment (there is no Lit
    /// importer), so asking for one is an error rather than a silent downgrade.
    LitOption(&'static str),
}

impl fmt::Display for JsxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadComponentName => f.write_str("componentName must match /^[A-Z][A-Za-z0-9]*$/"),
            Self::LitOption(name) => write!(f, "the lit target has no {name}"),
        }
    }
}

impl std::error::Error for JsxError {}

fn is_component_name(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric())
}

/// The component for `document`, which may be any JSON: what is not a document renders nothing.
pub fn to_jsx(document: &Json, options: &JsxOptions<'_>) -> Result<String, JsxError> {
    let name = options.component_name.unwrap_or("WeftScreen");
    if !is_component_name(name) {
        return Err(JsxError::BadComponentName);
    }
    if options.framework == Framework::Lit {
        if options.typescript {
            return Err(JsxError::LitOption("typescript flavour"));
        }
        if options.source {
            return Err(JsxError::LitOption("source comment"));
        }
    }
    let mut g = Gen::new(options.catalog, options.framework, options.typescript);
    if let Some(map) = document.get("fragments").and_then(Json::as_object) {
        g.inline_names = map.keys().cloned().collect();
    }
    // Lit's templates are strings, where a comment would reach the DOM.
    if options.framework != Framework::Lit {
        g.notes = notes(document);
    }
    let scope = Rc::new(Scope::default());
    let expanded = expanded(document, options.catalog);
    let root = expanded
        .as_ref()
        .unwrap_or(document)
        .as_object()
        .and_then(|o| o.get("root"));
    let pieces = match root {
        Some(root) => g.pieces_of(std::slice::from_ref(root), &scope, 0),
        None => Vec::new(),
    };
    let first = pieces.into_iter().find_map(|p| match p {
        Piece::Node(n, hidden) => Some((n, hidden)),
        _ => None,
    });
    let mut body = "null".to_owned();
    if let Some((n, hidden)) = first
        && let Some(mut c) = g.render(&n, &Ctx::default())
    {
        if let C::J(j) = &mut c {
            let mut kids = g.comments("");
            kids.extend(g.comments(n.doc_id));
            kids.append(&mut j.kids);
            j.kids = kids;
        }
        body = g.root_body(c, hidden);
    }
    let functions = g.inline_functions(document);
    let module = g.module(name, &body, &functions);
    match options
        .source
        .then(|| source_comment(document, options, name))
        .flatten()
    {
        Some(comment) => Ok(format!("{comment}\n{module}")),
        None => Ok(module),
    }
}

/// Readable notes for developers, one line per context entry (SPEC §2.3), keyed by the id its
/// `for` names and by `""` for the entries about the screen (`provenance::note`).
fn notes(document: &Json) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let entries = document.get("context").and_then(Json::as_array);
    for entry in entries.into_iter().flatten() {
        let field = |name: &str| entry.get(name).and_then(Json::as_str);
        let (Some(kind), Some(by), Some(name), Some(text)) =
            (field("kind"), field("by"), field("name"), field("text"))
        else {
            continue;
        };
        out.entry(field("for").unwrap_or_default().to_owned())
            .or_default()
            .push(crate::provenance::note(
                kind,
                field("status"),
                by,
                name,
                text,
            ));
    }
    out
}

/// With fragments, the document with each use expanded (SPEC §10.7). Only a document of the right
/// shape is expanded; anything else renders as written, which draws no use.
fn expanded(document: &Json, catalog: &weft_core::Catalog) -> Option<Json> {
    if catalog.fragments.is_empty() {
        return None;
    }
    let options = weft_core::ValidateOptions {
        catalog: None,
        mode: weft_core::Mode::Lenient,
        tokens: None,
        actions: None,
    };
    if weft_core::has_errors(&weft_core::validate(document, &options)) {
        return None;
    }
    let mut doc = weft_core::to_document(document);
    // Inline fragments are components of their own; only project fragments expand here.
    doc.fragments.clear();
    let result = weft_core::expand(&doc, catalog);
    serde_json::to_value(&result.document).ok()
}

/// The `weft:source` comment: the framework, the component name and the flavour on its first
/// line, so the importer can regenerate the same module. Only a document of the right shape has
/// a source to leave.
fn source_comment(document: &Json, options: &JsxOptions<'_>, name: &str) -> Option<String> {
    let diagnostics = weft_core::validate(
        document,
        &weft_core::ValidateOptions {
            catalog: Some(options.catalog),
            mode: weft_core::Mode::Lenient,
            tokens: None,
            actions: None,
        },
    );
    if weft_core::has_errors(&diagnostics) {
        return None;
    }
    let markup = weft_core::serialize(&weft_core::canonicalize(&weft_core::to_document(document)));
    let framework = match options.framework {
        Framework::React => "react",
        Framework::Solid => "solid",
        Framework::Lit => "lit",
    };
    let flavour = if options.typescript {
        " typescript"
    } else {
        ""
    };
    let head = format!("{framework} name={name}{flavour}");
    Some(crate::provenance::comment("/*", &head, &markup, "*/"))
}

pub(crate) struct Gen<'a> {
    catalog: &'a Catalog,
    fw: Framework,
    ts: bool,
    used: HashSet<&'static str>,
    fragment: bool,
    /// Names of the screen's inline fragments, drawn as components rather than expanded.
    inline_names: HashSet<String>,
    /// True while the body of an inline-fragment component is being printed.
    in_inline: bool,
    /// SolidJS imports, by module.
    solid: BTreeSet<&'static str>,
    solid_web: BTreeSet<&'static str>,
    /// What the Lit module imports beyond `LitElement` and `html`, set when printing.
    lit_uses: lit::Uses,
    /// Readable context notes by element id (`notes`).
    notes: HashMap<String, Vec<String>>,
}
