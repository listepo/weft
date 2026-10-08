//! The ids of the elements an exporter writes: the document's own, and fresh ones for the
//! elements a format needs that the document does not have.

use std::collections::HashSet;

use weft_core::{Child, Node};

pub fn collect_ids(n: &Node, ids: &mut HashSet<String>) {
    ids.extend(n.id.clone());
    for c in n.children.iter().chain(n.slots.values().flatten()) {
        if let Child::Node(n) = c {
            collect_ids(n, ids);
        }
    }
}

/// `base`, or `base-2`, `base-3`, … when it is taken; the id returned is taken from then on.
pub fn fresh_id(ids: &mut HashSet<String>, base: &str) -> String {
    let (mut id, mut n) = (base.to_owned(), 1);
    while !ids.insert(id.clone()) {
        n += 1;
        id = format!("{base}-{n}");
    }
    id
}
