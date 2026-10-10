//! A `<use>` checked against its fragment (SPEC §10.7): its attributes, actions and slots are
//! the fragment's parameters, and what it places answers to the list it stands in.

use super::{At, Owner, Validator, diag, node_at};
use crate::diagnostics::{Code, did_you_mean, one_of, quote};
use crate::fragment::{FRAGMENT, OUTLET, PARAM, USE};
use crate::inline;
use crate::model::{Catalog, Map, Node, Value};
use crate::rules::EACH;

impl<'a> Validator<'a> {
    pub(super) fn unknown_fragment(&mut self, node: &Node, path: &str) {
        let at = node_at(node, path, Some(FRAGMENT));
        let d = match node.props.get(FRAGMENT) {
            Some(Value::String(name)) => {
                let names = inline::candidate_names(self.inline, self.catalog);
                let message = format!("No fragment \"{name}\" in this project.");
                let hint = did_you_mean(name, names.iter().copied());
                diag(Code::W801, &at, message, one_of(names))
                    .got(name.clone())
                    .hint_opt(hint)
            }
            Some(value) => {
                let message = "`fragment` takes a literal name only.";
                diag(Code::W217, &at, message, "a fragment name").got(quote(value))
            }
            None => diag(
                Code::W205,
                &node_at(node, path, None),
                "<use> needs \"fragment\".",
                "fragment=\"…\"",
            ),
        };
        self.report(d);
    }

    /// An attribute, action or slot of a `<use>` that its fragment does not declare.
    pub(super) fn undeclared(&mut self, node: &Node, at: &At, name: &str, what: &str) {
        let owned = self.inline_sig(node);
        let Some(signature) = owned.as_ref().or(self.def_of(node)) else {
            return;
        };
        let known: Vec<String> = match what {
            "attribute" => signature
                .props
                .iter()
                .flat_map(|p| p.keys().cloned())
                .collect(),
            "action" => signature.events.clone().unwrap_or_default(),
            _ => signature
                .slots
                .iter()
                .flat_map(|s| s.keys().cloned())
                .collect(),
        };
        let fragment = node.props.get(FRAGMENT).map(quote).unwrap_or_default();
        let message = format!("Fragment {fragment} has no {what} parameter \"{name}\".");
        let hint = did_you_mean(name, &known);
        self.report(
            diag(Code::W802, at, message, one_of(&known))
                .got(name)
                .hint_opt(hint),
        );
    }

    /// The containment rules for what a use places where it stands.
    pub(super) fn check_placed(&mut self, node: &Node, at: &At, owner: &Owner, where_: &str) {
        let catalog: &'a Catalog = self.catalog;
        let inline = self.inline;
        let mut placed = Vec::new();
        placed_nodes(catalog, inline, node, &mut vec![], &mut placed);
        let mut seen: Vec<&str> = Vec::new();
        for child in placed {
            let kind = child.kind.as_str();
            let grows = child.props.get("grow") == Some(&Value::Bool(true));
            if grows && !self.grows_in_stack(Some(owner)) {
                let message = format!("<{kind}>, placed by this <use>, grows outside a <stack>.");
                self.report(diag(Code::W318, at, message, "a parent <stack>"));
            }
            let Some(component) = self.component(kind).filter(|_| !seen.contains(&kind)) else {
                continue;
            };
            seen.push(kind);
            if let Some(allowed) = owner
                .allowed
                .as_ref()
                .filter(|a| !a.iter().any(|k| k == kind))
            {
                let message = format!("{where_} does not take <{kind}>, which this <use> places.");
                self.report(diag(Code::W302, at, message, one_of(allowed)).got(kind));
            }
            let (Some(parents), Some(owner_kind)) =
                (component.allowed_parents.as_ref(), owner.kind.as_ref())
            else {
                continue;
            };
            if catalog.components.contains_key(owner_kind) && !parents.contains(owner_kind) {
                let message =
                    format!("<{kind}>, placed by this <use>, cannot stand in <{owner_kind}>.");
                let d = diag(
                    Code::W303,
                    at,
                    message,
                    format!("a parent {}", one_of(parents)),
                );
                self.report(d.got(owner_kind.clone()));
            }
        }
    }
}

/// The elements a use places at its own level: `<each>` is transparent and nested uses are
/// followed, each fragment once, so that a cycle cannot loop.
fn placed_nodes<'a>(
    catalog: &'a Catalog,
    inline: &'a Map<Node>,
    node: &Node,
    seen: &mut Vec<String>,
    out: &mut Vec<&'a Node>,
) {
    let Some(Value::String(name)) = node.props.get(FRAGMENT) else {
        return;
    };
    let Some(root) = inline::resolve(inline, catalog, name) else {
        return;
    };
    if seen.iter().any(|s| s == name) {
        return;
    }
    seen.push(name.clone());
    let children = root.children.iter();
    let mut stack: Vec<&'a Node> = children.rev().filter_map(|c| c.as_node()).collect();
    while let Some(child) = stack.pop() {
        match child.kind.as_str() {
            PARAM | OUTLET => {}
            EACH => stack.extend(child.children.iter().rev().filter_map(|c| c.as_node())),
            USE => placed_nodes(catalog, inline, child, seen, out),
            _ => out.push(child),
        }
    }
}
