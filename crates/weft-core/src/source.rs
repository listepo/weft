//! Markup positions carried beside the model, so canonical JSON stays free of presentation
//! details while validation can still point at a line and column.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::diagnostics::Position;
use crate::model::{Child, Document, Node};
use crate::rules::is_id;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListSource {
    pub pos: Position,
    pub children: Vec<Option<Position>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeSource {
    pub pos: Position,
    /// Keyed by attribute name as written: `id`, `weft`, `on-press`, …
    pub attrs: HashMap<String, Position>,
    /// Index-aligned with `node.children`.
    pub children: Vec<Option<Position>>,
    pub slots: IndexMap<String, ListSource>,
}

/// Positions of one node, present only when the node came from `parse`. Equality ignores it:
/// two documents are equal when their content is.
#[derive(Clone, Debug, Default)]
pub struct Source(pub Option<Box<NodeSource>>);

impl PartialEq for Source {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// One step of a diagnostic path (SPEC §6.1): `kind#id`, or `kind[index]` when the element has
/// no usable id and its position in the parent's list is known.
pub fn path_segment(kind: &str, id: Option<&str>, index: Option<usize>) -> String {
    match (id, index) {
        (Some(id), _) if is_id(id) => format!("{kind}#{id}"),
        (_, Some(i)) => format!("{kind}[{i}]"),
        _ => kind.to_owned(),
    }
}

/// Every element of `document` with its diagnostic path (SPEC §6.1): `/screen#home/stack[0]`,
/// `slot[name]` for a slot's list. A host that marks or finds elements by path names them as the
/// diagnostics do, so an agent can patch what a person picked.
pub fn element_paths(document: &Document) -> Vec<(&Node, String)> {
    let root = &document.root;
    let mut stack = vec![(
        root,
        format!("/{}", path_segment(&root.kind, root.id.as_deref(), None)),
    )];
    let mut paths = Vec::new();
    while let Some((node, path)) = stack.pop() {
        for (name, list) in &node.slots {
            push_list(&mut stack, list, &format!("{path}/slot[{name}]"));
        }
        push_list(&mut stack, &node.children, &path);
        paths.push((node, path));
    }
    paths
}

/// The elements of one list, indexed as diagnostics index them: text counts too.
fn push_list<'a>(stack: &mut Vec<(&'a Node, String)>, list: &'a [Child], path: &str) {
    for (index, child) in list.iter().enumerate() {
        if let Child::Node(node) = child {
            let segment = path_segment(&node.kind, node.id.as_deref(), Some(index));
            stack.push((node.as_ref(), format!("{path}/{segment}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ParseOptions, parse};

    #[test]
    fn an_element_without_an_id_is_its_kind_and_its_place() {
        // Parsed without a catalog, so no id is required; text counts in a list's indices.
        let markup = r#"<screen weft="0.3"><stack>Hi<button/></stack></screen>"#;
        let mut document = parse(markup, &ParseOptions::default())
            .document
            .expect("parses");
        let mut ok = Node::new("button");
        ok.id = Some("ok".into());
        document
            .root
            .slots
            .insert("header".into(), vec![Child::Node(Box::new(ok))]);
        let mut paths: Vec<String> = element_paths(&document)
            .into_iter()
            .map(|(_, path)| path)
            .collect();
        paths.sort();
        assert_eq!(
            paths,
            [
                "/screen",
                "/screen/slot[header]/button#ok",
                "/screen/stack[0]",
                "/screen/stack[0]/button[1]",
            ]
        );
    }
}
