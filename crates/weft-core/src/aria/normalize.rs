//! A tree in the form a Playwright aria snapshot prints, so a tree a document declares compares
//! with one taken from a page.

use super::AriaNode;
use super::tree::{TRANSPARENT, text_node};
use crate::squash;

/// Transparent roles dissolved, adjacent text merged, whitespace collapsed, and text that only
/// repeats the name dropped.
pub fn normalize_aria(tree: &AriaNode) -> AriaNode {
    let name = squash(&tree.name);
    let children = normalize_children(&tree.children);
    let repeats = matches!(&children[..], [only] if only.role == "text" && only.name == name);
    AriaNode {
        role: tree.role.clone(),
        name,
        states: tree.states.clone(),
        url: tree.url.clone(),
        children: if repeats { Vec::new() } else { children },
    }
}

fn normalize_children(list: &[AriaNode]) -> Vec<AriaNode> {
    fn visit<'t>(n: &'t AriaNode, flat: &mut Vec<&'t AriaNode>) {
        if TRANSPARENT.contains(&n.role.as_str()) && squash(&n.name).is_empty() {
            for c in &n.children {
                visit(c, flat);
            }
        } else {
            flat.push(n);
        }
    }
    let mut flat = Vec::new();
    for n in list {
        visit(n, &mut flat);
    }
    let mut out: Vec<AriaNode> = Vec::new();
    for n in flat {
        if n.role != "text" {
            out.push(normalize_aria(n));
            continue;
        }
        let t = squash(&n.name);
        if t.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.role == "text" => {
                last.name.push(' ');
                last.name.push_str(&t);
            }
            _ => out.push(text_node(t)),
        }
    }
    out
}
