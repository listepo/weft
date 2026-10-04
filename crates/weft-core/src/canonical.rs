//! Canonical JSON (SPEC §3): one byte sequence per document, so that tools can diff and hash.

use std::cmp::Ordering;

use crate::json::{array_index, to_pretty};
use crate::model::{Child, Document, Map, Node, Value};

/// Whitespace handling of SPEC §2 for text content; only XML whitespace counts.
pub fn normalize_text(text: &str) -> String {
    text.split([' ', '\t', '\n', '\r'])
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Appends text to a child list; adjacent text runs join with one space, as markup would read
/// them. Returns whether a new child was added.
pub fn append_text(children: &mut Vec<Child>, text: String) -> bool {
    if text.is_empty() {
        return false;
    }
    if let Some(Child::Text(previous)) = children.last_mut() {
        previous.push(' ');
        previous.push_str(&text);
        return false;
    }
    children.push(Child::Text(text));
    true
}

/// SPEC §3 sorts keys by UTF-16 code unit, which differs from Rust's byte order above U+FFFF.
pub fn compare_keys(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// Sorted as SPEC §3 asks, then laid out as the JavaScript object built from those entries
/// would be: array-index keys first. Iteration order then matches the TypeScript core everywhere.
fn sorted<T>(mut entries: Vec<(String, T)>) -> Map<T> {
    entries.sort_by(|a, b| compare_keys(&a.0, &b.0));
    entries.sort_by_key(|(k, _)| array_index(k).map_or((1, 0), |i| (0, i)));
    entries.into_iter().collect()
}

pub struct Parts {
    pub kind: String,
    pub id: Option<String>,
    pub props: Vec<(String, Value)>,
    pub on: Vec<(String, String)>,
    pub slots: Vec<(String, Vec<Child>)>,
    pub children: Vec<Child>,
}

/// Builds a node with canonical key order and without empty members.
pub fn assemble_node(parts: Parts) -> Node {
    Node {
        kind: parts.kind,
        id: parts.id,
        props: sorted(parts.props),
        on: sorted(parts.on),
        slots: sorted(
            parts
                .slots
                .into_iter()
                .filter(|(_, list)| !list.is_empty())
                .collect(),
        ),
        children: parts.children,
        source: Default::default(),
    }
}

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Number(n) if *n == 0.0 => Value::Number(0.0),
        other => other.clone(),
    }
}

fn canonical_children(children: &[Child]) -> Vec<Child> {
    let mut out = Vec::new();
    for child in children {
        match child {
            Child::Text(t) => {
                append_text(&mut out, normalize_text(t));
            }
            Child::Node(n) => out.push(Child::Node(Box::new(canonical_node(n)))),
        }
    }
    out
}

fn canonical_node(node: &Node) -> Node {
    assemble_node(Parts {
        kind: node.kind.clone(),
        id: node.id.clone(),
        props: node
            .props
            .iter()
            .map(|(k, v)| (k.clone(), canonical_value(v)))
            .collect(),
        on: node
            .on
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        slots: node
            .slots
            .iter()
            .map(|(k, v)| (k.clone(), canonical_children(v)))
            .collect(),
        children: canonical_children(&node.children),
    })
}

/// The canonical form of a document; positions from parsing are dropped.
pub fn canonicalize(document: &Document) -> Document {
    Document {
        weft: document.weft.clone(),
        root: canonical_node(&document.root),
    }
}

/// Canonical JSON text: two-space indentation and a final newline.
pub fn stringify(document: &Document) -> String {
    format!("{}\n", to_pretty(&canonicalize(document)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_collapses_xml_whitespace_only() {
        assert_eq!(normalize_text("  a \t\n b\r\n "), "a b");
        assert_eq!(normalize_text("a\u{a0}b"), "a\u{a0}b");
        assert_eq!(normalize_text(" \n "), "");
    }

    #[test]
    fn adjacent_text_joins_with_one_space() {
        let mut list = vec![];
        assert!(append_text(&mut list, "a".into()));
        assert!(!append_text(&mut list, "b".into()));
        assert!(!append_text(&mut list, String::new()));
        assert_eq!(list, vec![Child::Text("a b".into())]);
    }

    #[test]
    fn keys_sort_by_utf16_code_unit() {
        // U+FF61 sorts after the surrogate pair of U+1F600 in UTF-16, before it in UTF-8.
        assert_eq!(compare_keys("\u{1F600}", "\u{FF61}"), Ordering::Less);
    }

    #[test]
    fn canonical_json_orders_keys_and_drops_empty_members() {
        let mut root = Node::new("screen");
        root.id = Some("s".into());
        root.props.insert("z".into(), Value::Number(-0.0));
        root.props.insert(
            "a".into(),
            Value::Bind {
                bind: "$.x".into(),
                not: true,
            },
        );
        root.slots.insert("empty".into(), vec![]);
        root.children = vec![Child::Text(" hi ".into())];
        let doc = Document {
            weft: "0.1".into(),
            root,
        };
        assert_eq!(
            stringify(&doc),
            "{\n  \"weft\": \"0.1\",\n  \"root\": {\n    \"kind\": \"screen\",\n    \"id\": \"s\",\n    \"props\": {\n      \"a\": {\n        \"bind\": \"$.x\",\n        \"not\": true\n      },\n      \"z\": 0\n    },\n    \"children\": [\n      \"hi\"\n    ]\n  }\n}\n"
        );
    }
}
