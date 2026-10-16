//! WinUI → Weft for pages the generator wrote: the document comes from the `weft:source winui`
//! comment (SPEC §9, Provenance) and is believed only when generating from it gives the same XAML,
//! compared as XML. A page this rejects is for the hand-written reader (T105.8).

use indexmap::IndexMap;
use roxmltree::{Node, NodeType, ParsingOptions};
use weft_catalog::{Token, token_types};
use weft_core::{Catalog, Diagnostic, Document, Mode, ParseOptions, has_errors, parse};
use weft_import::{on_reader_stack, provenance};

use crate::generate::{GenerateOptions, TARGET, generate};

/// The longest source the importer reads, as the other importers bound theirs.
pub const MAX_SOURCE_LENGTH: usize = 2_000_000;
/// A generated page has a few XML nodes per Weft element (the element, its indentation text and
/// a wrapper at most), and a document at most `MAX_NODES` elements.
pub(crate) const MAX_XML_NODES: u32 = weft_import::MAX_NODES as u32 * 8;
/// Element nesting the parser is given: a Weft level is a few XAML levels at most (a property
/// element, a template, a slot panel), and a debug build parses this deep within the reader stack.
pub const MAX_XML_DEPTH: usize = weft_import::MAX_DEPTH * 4;

/// The page's own tokens (SPEC §9, To WinUI), in the presentation namespace.
const RESOURCES: (&str, &str) = (PRESENTATION, "Page.Resources");
const PRESENTATION: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
const XAML: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

pub struct ImportOptions<'a> {
    pub catalog: &'a Catalog,
    /// The tokens the page was generated with, against which the comment is checked.
    pub tokens: &'a IndexMap<String, Token>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("the source is longer than {MAX_SOURCE_LENGTH} bytes")]
    TooLong,
    #[error("the source is not well-formed XML within the importer's bounds: {0}")]
    Xml(String),
    #[error("no `weft:source winui` comment with a name and a namespace")]
    NoSource,
    #[error("the source comment is not a valid Weft document ({} diagnostics)", .0.len())]
    Invalid(Vec<Diagnostic>),
    #[error("the page is not what its source comment generates: it was edited")]
    Edited,
}

fn xml(source: &str) -> Result<roxmltree::Document<'_>, ImportError> {
    let options = ParsingOptions {
        nodes_limit: MAX_XML_NODES,
        ..ParsingOptions::default()
    };
    roxmltree::Document::parse_with_options(source, options)
        .map_err(|e| ImportError::Xml(e.to_string()))
}

pub fn import_winui(source: &str, options: &ImportOptions<'_>) -> Result<Document, ImportError> {
    if source.len() > MAX_SOURCE_LENGTH {
        return Err(ImportError::TooLong);
    }
    // roxmltree parses an element within its parent's call, as in `read_winui`.
    if nested_past(source, MAX_XML_DEPTH) {
        let what = format!("elements nest more than {MAX_XML_DEPTH} levels deep");
        return Err(ImportError::Xml(what));
    }
    on_reader_stack("weft-import-winui", || read_back(source, options))
        .unwrap_or_else(|| Err(ImportError::Xml("the reader stopped unexpectedly".into())))
}

fn read_back(source: &str, options: &ImportOptions<'_>) -> Result<Document, ImportError> {
    let page = xml(source)?;
    // Only a comment before the root element is the page's own, as the generator writes it.
    let comment = page
        .root()
        .children()
        .take_while(|n| !n.is_element())
        .filter_map(|n| n.is_comment().then(|| n.text()).flatten())
        .find(|t| t.trim_start().starts_with(provenance::MARKER))
        .ok_or(ImportError::NoSource)?;
    let wrapped = format!("<!--{comment}-->");
    let (head, markup) = provenance::find(&wrapped, "<!--", "-->").ok_or(ImportError::NoSource)?;
    let mut words = head.split_whitespace();
    let option = |key: &str| head.split_whitespace().find_map(|w| w.strip_prefix(key));
    let (Some(TARGET), Some(name), Some(namespace)) =
        (words.next(), option("name="), option("namespace="))
    else {
        return Err(ImportError::NoSource);
    };
    let types = token_types(options.tokens);
    let parsed = parse(
        &markup,
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
    // A page that holds its own tokens was generated without the shared dictionary.
    let own = page
        .root_element()
        .children()
        .any(|c| c.has_tag_name(RESOURCES));
    let again = generate(
        &document,
        &GenerateOptions {
            catalog: options.catalog,
            tokens: options.tokens,
            name: Some(name),
            namespace,
            shared_tokens: !own,
            appearance: None,
        },
    )
    .map_err(|_| ImportError::Edited)?;
    let regenerated = xml(&again.xaml)?;
    if !same(page.root_element(), regenerated.root_element(), false) {
        return Err(ImportError::Edited);
    }
    Ok(document)
}

/// The nodes that carry meaning: elements and text that is not blank.
fn meaningful<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(|c| match c.node_type() {
        NodeType::Element => true,
        NodeType::Text => c.text().is_some_and(|t| !t.trim().is_empty()),
        _ => false,
    })
}

/// Equal as XML: the same expanded names, the same attributes in any order, the same non-blank
/// text; comments, processing instructions and whitespace do not count. In the page's own
/// resources only the elements and their keys count (`loose`): the values are the project's
/// tokens, which may have changed since, not the screen.
fn same(a: Node<'_, '_>, b: Node<'_, '_>, loose: bool) -> bool {
    if a.is_text() || b.is_text() {
        let both = a.is_text() && b.is_text();
        return both && (loose || a.text().map(str::trim) == b.text().map(str::trim));
    }
    let loose = loose || a.has_tag_name(RESOURCES);
    a.tag_name() == b.tag_name()
        && attributes(a, loose) == attributes(b, loose)
        && meaningful(a).count() == meaningful(b).count()
        && meaningful(a)
            .zip(meaningful(b))
            .all(|(x, y)| same(x, y, loose))
}

fn attributes<'a>(n: Node<'a, '_>, loose: bool) -> Vec<(Option<&'a str>, &'a str, &'a str)> {
    let key =
        |at: &roxmltree::Attribute<'_, '_>| (at.namespace(), at.name()) == (Some(XAML), "Key");
    let mut all: Vec<_> = n
        .attributes()
        .map(|at| {
            let value = if loose && !key(&at) { "" } else { at.value() };
            (at.namespace(), at.name(), value)
        })
        .collect();
    all.sort_unstable();
    all
}

/// Whether elements nest deeper than `limit`. Where this count and the parser part, the parser
/// stops at an error first, so it never recurses deeper than the count.
pub(crate) fn nested_past(source: &str, limit: usize) -> bool {
    let bytes = source.as_bytes();
    let after = |from: usize, end: &[u8]| {
        bytes[from..]
            .windows(end.len())
            .position(|w| w == end)
            .map_or(bytes.len(), |at| from + at + end.len())
    };
    let (mut i, mut depth) = (0, 0usize);
    while let Some(at) = bytes[i..].iter().position(|&b| b == b'<') {
        i += at + 1;
        let rest = &bytes[i..];
        i = match rest.first() {
            Some(b'/') => {
                depth = depth.saturating_sub(1);
                after(i, b">")
            }
            // Comments, CDATA, instructions and declarations hold no elements.
            Some(b'!') if rest.starts_with(b"!--") => after(i, b"-->"),
            Some(b'!') if rest.starts_with(b"![CDATA[") => after(i, b"]]>"),
            Some(b'?') => after(i, b"?>"),
            Some(b'!') => after(i, b">"),
            _ => {
                // An attribute value may hold `>`; a tag that ends `/>` is empty.
                let mut quote = None;
                let mut j = i;
                while j < bytes.len() {
                    match (quote, bytes[j]) {
                        (None, b'>') => break,
                        (None, q @ (b'"' | b'\'')) => quote = Some(q),
                        (Some(q), b) if b == q => quote = None,
                        _ => {}
                    }
                    j += 1;
                }
                if bytes.get(j.wrapping_sub(1)) != Some(&b'/') {
                    depth += 1;
                    if depth > limit {
                        return true;
                    }
                }
                j + 1
            }
        }
        .min(bytes.len());
    }
    false
}
