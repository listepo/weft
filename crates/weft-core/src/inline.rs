//! Inline fragments (SPEC §10.7): a `<fragment name>` that lives on one screen.
//! A project fragment of the same name wins, and the inline one is left out, so one name
//! keeps one meaning.

use crate::diagnostics::{Code, Diagnostic, Position};
use crate::fragment::{self, signature};
use crate::model::{Catalog, Child, Document, Map, Node, Value};
use crate::rules::is_name;
use crate::values::read_value;

pub fn diagnostic(pos: Position, path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        Code::W808,
        path,
        message,
        "<fragment name=\"…\"> directly under <screen>, after <context> and before the body",
    )
    .pos(Some(pos))
    .hint("place it with the screen's other inline fragments, with only name, label and version")
}

/// Whether `node` can be the body of an inline fragment. Anything else is `W808` and left out
/// of resolution, so a use of that name falls through to the project.
pub fn usable(name: &str, node: &Node) -> bool {
    is_name(name)
        && node.kind == fragment::FRAGMENT
        && node.id.is_none()
        && node.on.is_empty()
        && node
            .props
            .iter()
            .all(|(k, v)| k == "version" || (k == "label" && matches!(v, Value::String(_))))
}

/// The fragment a name means while expanding or checking `inline`: the screen's, unless the
/// project already defines it.
pub fn resolve<'a>(inline: &'a Map<Node>, catalog: &'a Catalog, name: &str) -> Option<&'a Node> {
    if let Some(node) = inline.get(name)
        && usable(name, node)
        && !catalog.fragments.contains_key(name)
    {
        return Some(node);
    }
    catalog.fragments.get(name).map(|f| &f.document.root)
}

/// Literal types of `<use>` attributes, once every inline fragment of the screen is known.
/// A use can precede the fragment it names, and the parser types literals as it reads.
pub fn retype(node: &mut Node, inline: &Map<Node>, catalog: Option<&Catalog>) {
    if node.kind == fragment::USE
        && let Some(Value::String(name)) = node.props.get("fragment").cloned()
    {
        let from_project =
            catalog.and_then(|c| c.fragments.get(&name).map(|f| f.signature.clone()));
        let sig = from_project.or_else(|| {
            inline
                .get(&name)
                .filter(|n| usable(&name, n))
                .map(signature)
        });
        if let Some(sig) = sig {
            let keys: Vec<String> = node.props.keys().cloned().collect();
            for key in keys {
                let Some(def) = sig.prop(&key) else { continue };
                let Some(Value::String(raw)) = node.props.get(&key).cloned() else {
                    continue;
                };
                if let Ok(typed) = read_value(&raw, Some(def.kind)) {
                    node.props.insert(key, typed);
                }
            }
        }
    }
    for child in &mut node.children {
        if let Child::Node(n) = child {
            retype(n, inline, catalog);
        }
    }
    for list in node.slots.values_mut() {
        for child in list {
            if let Child::Node(n) = child {
                retype(n, inline, catalog);
            }
        }
    }
}

/// Names a `<use>` may mean: the screen's inline fragments, then the project's.
pub fn candidate_names<'a>(inline: &'a Map<Node>, catalog: &'a Catalog) -> Vec<&'a str> {
    let mut names: Vec<&str> = inline
        .iter()
        .filter(|(name, node)| usable(name, node) && !catalog.fragments.contains_key(*name))
        .map(|(name, _)| name.as_str())
        .collect();
    for name in catalog.fragments.keys() {
        if !names.contains(&name.as_str()) {
            names.push(name);
        }
    }
    names
}

/// An inline fragment carried on a fragment file is never part of that file.
pub fn screen_fragments(document: &Document) -> bool {
    document.root.kind != fragment::FRAGMENT
}
