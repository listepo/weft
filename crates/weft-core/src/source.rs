//! Markup positions carried beside the model, so canonical JSON stays free of presentation
//! details while validation can still point at a line and column.

use std::collections::HashMap;

use indexmap::IndexMap;

use crate::diagnostics::Position;
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
