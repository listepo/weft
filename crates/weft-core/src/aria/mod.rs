//! The accessibility tree a document declares (SPEC §9), as the reference renderer
//! (`@weft/render-react`) exposes it, in the shape of a Playwright aria snapshot. Hosts without a
//! browser show it to agents (`weft_render`), and the renderer's tests check it against the DOM.

mod inst;
mod normalize;
mod tree;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::{Catalog, Document, Map, expand};

pub use normalize::normalize_aria;

/// One node: `role` is an ARIA role, `text` for a run of text (the text is its `name`), or
/// `fragment` for the root, which holds the top-level nodes as a snapshot of `<body>` does.
/// `states` holds only what a Playwright snapshot reports (checked, disabled, expanded, invalid,
/// level, pressed, selected).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AriaNode {
    pub role: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub states: Map<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<AriaNode>,
}

/// The normalized tree of `document` with `data` for its bindings (`Json::Null` when there is
/// none): what the reference renderer's page exposes.
pub fn expected_tree(document: &Document, catalog: &Catalog, data: &Json) -> AriaNode {
    // A use of a fragment is drawn as the fragment's body (SPEC §10.7).
    let expanded;
    let root = if document.fragments.is_empty() && catalog.fragments.is_empty() {
        &document.root
    } else {
        expanded = expand(document, catalog).document;
        &expanded.root
    };
    let children = inst::expand_root(root, catalog, data)
        .map(|r| tree::of_root(&r))
        .unwrap_or_default();
    normalize_aria(&AriaNode {
        role: "fragment".into(),
        children,
        ..AriaNode::default()
    })
}

#[cfg(test)]
mod tests;
