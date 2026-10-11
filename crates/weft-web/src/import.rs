//! Source → Weft (SPEC §9, "From source code"): HTML pages, and React or Solid components. Output of
//! the generators comes back exactly through its `weft:source` comment; anything else is read by
//! its markup conventions, and what Weft cannot hold is listed as losses. The input is untrusted:
//! it is only parsed, never run, and size, depth and node count are bounded.

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{Catalog, Diagnostic};
use weft_import::{ImportResult, squash};

use crate::dom::{Conventions, bounded, find_body, read_dom};
use crate::html::{HtmlOptions, to_html};
use crate::provenance;
use crate::tree::{DOCUMENT, Dom, HNode, parse_html};

pub struct ImportOptions<'a> {
    pub catalog: &'a Catalog,
    /// The design tokens `var(--weft-…)` and `gap-*` references resolve against.
    pub tokens: &'a IndexMap<String, Token>,
}

/// Imports an HTML page. Never fails: what cannot be read is reported in the result.
pub fn import_html(html: &str, options: &ImportOptions<'_>) -> ImportResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let html = bounded(html, &mut diagnostics);
    let dom = parse_html(html);
    if let Some((_, markup)) = provenance::find(html, "<!--", "-->")
        && let Some((document, parse_diagnostics)) = provenance::claimed(&markup, options.catalog)
        && let Ok(again) = to_html(
            &document,
            &HtmlOptions {
                catalog: options.catalog,
                tokens: options.tokens,
                source: true,
                appearance: None,
            },
        )
        && same_body(&dom, &parse_html(&again))
    {
        diagnostics.extend(parse_diagnostics);
        return ImportResult {
            document,
            losses: Vec::new(),
            diagnostics,
        };
    }
    let conventions = Conventions::new(options.tokens);
    read_dom(
        &dom,
        options.catalog,
        Some(&conventions),
        std::collections::HashMap::new(),
        diagnostics,
    )
    .result
}

/// The content a reader sees: elements, and text that is not only whitespace.
fn shown(dom: &Dom, el: usize) -> impl Iterator<Item = usize> + '_ {
    dom.children(el)
        .iter()
        .copied()
        .filter(|&c| match &dom.nodes[c] {
            HNode::Text(t) => !squash(t).is_empty(),
            HNode::Element { .. } => true,
        })
}

/// Whether two pages have the same body: the same elements with the same attributes in any order,
/// and the same text up to whitespace, which formatters change. Iterative, as pages nest deeply.
pub(crate) fn same_body(a: &Dom, b: &Dom) -> bool {
    let (Some(x), Some(y)) = (find_body(a), find_body(b)) else {
        return false;
    };
    same_tree(a, b, x, y)
}

pub(crate) fn same_tree(a: &Dom, b: &Dom, x: usize, y: usize) -> bool {
    let mut pending = vec![(x, y)];
    while let Some((x, y)) = pending.pop() {
        match (&a.nodes[x], &b.nodes[y]) {
            (HNode::Text(s), HNode::Text(t)) => {
                if squash(s) != squash(t) {
                    return false;
                }
            }
            (HNode::Element { name: n, .. }, HNode::Element { name: m, .. }) => {
                if x != DOCUMENT && n != m {
                    return false;
                }
                let mut p = a.attrs(x).to_vec();
                let mut q = b.attrs(y).to_vec();
                p.sort();
                q.sort();
                if p != q {
                    return false;
                }
                let xs: Vec<usize> = shown(a, x).collect();
                let ys: Vec<usize> = shown(b, y).collect();
                if xs.len() != ys.len() {
                    return false;
                }
                pending.extend(xs.into_iter().zip(ys));
            }
            _ => return false,
        }
    }
    true
}
