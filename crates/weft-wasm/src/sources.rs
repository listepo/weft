//! Markup positions across the boundary. The TypeScript `SourceMap` is keyed by node identity,
//! which JSON cannot carry, so positions travel as a list in node pre-order (a node, its children,
//! then its slots in order) and each side pairs them with its own nodes by walking the same way.

use serde::{Deserialize, Serialize};
use weft_core::{Child, ListSource, Node, NodeSource, Position};

/// `[line, column]`.
type Pos = (u32, u32);

/// One node's positions: `[pos, attrs, children, slots]`, with attrs as `[name, line, column]`
/// and slots as `[name, pos, children]`. Arrays keep the payload small.
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct Src(
    Pos,
    Vec<(String, u32, u32)>,
    Vec<Option<Pos>>,
    Vec<(String, Pos, Vec<Option<Pos>>)>,
);

fn pos(p: Position) -> Pos {
    (p.line, p.column)
}

fn position((line, column): Pos) -> Position {
    Position { line, column }
}

fn encode(source: &NodeSource) -> Src {
    let mut attrs: Vec<(String, u32, u32)> = source
        .attrs
        .iter()
        .map(|(name, p)| (name.clone(), p.line, p.column))
        .collect();
    // The TypeScript map listed attributes as written; the Rust one is unordered.
    attrs.sort_by_key(|(_, line, column)| (*line, *column));
    Src(
        pos(source.pos),
        attrs,
        source.children.iter().map(|p| p.map(pos)).collect(),
        source
            .slots
            .iter()
            .map(|(name, list)| {
                (
                    name.clone(),
                    pos(list.pos),
                    list.children.iter().map(|p| p.map(pos)).collect(),
                )
            })
            .collect(),
    )
}

fn decode(src: Src) -> NodeSource {
    let Src(at, attrs, children, slots) = src;
    NodeSource {
        pos: position(at),
        attrs: attrs
            .into_iter()
            .map(|(name, line, column)| (name, Position { line, column }))
            .collect(),
        children: children.into_iter().map(|p| p.map(position)).collect(),
        slots: slots
            .into_iter()
            .map(|(name, at, children)| {
                (
                    name,
                    ListSource {
                        pos: position(at),
                        children: children.into_iter().map(|p| p.map(position)).collect(),
                    },
                )
            })
            .collect(),
    }
}

fn child_nodes(node: &Node) -> impl Iterator<Item = &Node> {
    node.children
        .iter()
        .chain(node.slots.values().flatten())
        .filter_map(Child::as_node)
}

/// Positions of every node in pre-order; `None` where a node has none.
pub fn collect(root: &Node) -> Vec<Option<Src>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        out.push(node.source.0.as_deref().map(encode));
        let start = stack.len();
        stack.extend(child_nodes(node));
        // A stack pops last-first; reversing the pushed run keeps pre-order.
        stack[start..].reverse();
    }
    out
}

/// Gives each node of `root`, in pre-order, the next entry of `sources`.
pub fn attach(root: &mut Node, sources: Vec<Option<Src>>) {
    let mut sources = sources.into_iter();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        // Destructured, so the children borrow for as long as the stack does.
        let Node {
            children,
            slots,
            source,
            ..
        } = node;
        source.0 = sources.next().flatten().map(|s| Box::new(decode(s)));
        let start = stack.len();
        stack.extend(
            children
                .iter_mut()
                .chain(slots.values_mut().flatten())
                .filter_map(|c| match c {
                    Child::Node(n) => Some(n.as_mut()),
                    Child::Text(_) => None,
                }),
        );
        stack[start..].reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weft_core::{ParseOptions, parse};

    const MARKUP: &str = r#"<screen id="s" weft="0.1"><stack id="a">hi<text id="b"/></stack><dialog id="d"><slot name="actions"><button id="c"/></slot></dialog></screen>"#;

    fn kinds_in_pre_order(root: &Node) -> Vec<String> {
        let mut out = vec![root.id.clone().unwrap_or_default()];
        for child in child_nodes(root) {
            out.extend(kinds_in_pre_order(child));
        }
        out
    }

    #[test]
    fn positions_are_listed_in_pre_order_with_slots_after_children() {
        let doc = parse(MARKUP, &ParseOptions::default()).document.unwrap();
        assert_eq!(kinds_in_pre_order(&doc.root), ["s", "a", "b", "d", "c"]);
        let sources = collect(&doc.root);
        let columns: Vec<u32> = sources.iter().map(|s| s.as_ref().unwrap().0.1).collect();
        assert_eq!(columns, [1, 27, 43, 65, 101]);
    }

    #[test]
    fn attributes_are_listed_as_written() {
        let doc = parse(MARKUP, &ParseOptions::default()).document.unwrap();
        let root = &collect(&doc.root)[0];
        let names: Vec<&str> = root
            .as_ref()
            .unwrap()
            .1
            .iter()
            .map(|a| a.0.as_str())
            .collect();
        assert_eq!(names, ["id", "weft"]);
    }

    #[test]
    fn attached_positions_round_trip() {
        let doc = parse(MARKUP, &ParseOptions::default()).document.unwrap();
        let sources = collect(&doc.root);
        let mut bare = weft_core::canonicalize(&doc);
        assert!(collect(&bare.root).iter().all(Option::is_none));
        let text = serde_json::to_string(&sources).unwrap();
        attach(&mut bare.root, serde_json::from_str(&text).unwrap());
        assert_eq!(collect(&bare.root), sources);
    }

    #[test]
    fn missing_entries_leave_nodes_without_positions() {
        let mut doc = parse(MARKUP, &ParseOptions::default()).document.unwrap();
        attach(&mut doc.root, vec![]);
        assert!(collect(&doc.root).iter().all(Option::is_none));
    }
}
