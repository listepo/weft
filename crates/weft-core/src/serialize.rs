//! Canonical markup (SPEC §3): the serializer emits exactly one text per canonical document.

use crate::canonical::{canonicalize, compare_keys};
use crate::model::{Child, Document, Node};
use crate::rules::SLOT;
use crate::values::format_value;

const INDENT: &str = "  ";

pub fn serialize(document: &Document) -> String {
    let doc = canonicalize(document);
    let mut lines = Vec::new();
    write_node(&doc.root, "", &mut lines, Some(&doc.weft));
    format!("{}\n", lines.join("\n"))
}

/// Tabs and newlines become references because XML would otherwise read them as spaces.
fn escape_attribute(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c => out.push(c),
        }
    }
    out
}

/// `>` is escaped too, because `]]>` is not allowed in XML text.
fn escape_text(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

fn write_node(node: &Node, indent: &str, lines: &mut Vec<String>, weft: Option<&str>) {
    let mut attributes: Vec<(String, String)> = Vec::new();
    if let Some(id) = &node.id {
        attributes.push(("id".into(), id.clone()));
    }
    let mut props: Vec<(String, String)> = node
        .props
        .iter()
        .filter(|(name, _)| weft.is_none() || name.as_str() != "weft")
        .map(|(name, value)| (name.clone(), format_value(value)))
        .collect();
    // The root carries the document version as an ordinary attribute, sorted with the props.
    if let Some(weft) = weft.filter(|w| !w.is_empty()) {
        props.push(("weft".into(), weft.to_owned()));
    }
    props.sort_by(|a, b| compare_keys(&a.0, &b.0));
    attributes.extend(props);
    attributes.extend(
        node.on
            .iter()
            .map(|(event, action)| (format!("on-{event}"), action.clone())),
    );
    let attrs: String = attributes
        .iter()
        .map(|(n, v)| format!(" {n}=\"{}\"", escape_attribute(v)))
        .collect();
    let head = format!("<{}{attrs}", node.kind);
    let slots: Vec<(&String, &Vec<Child>)> = node.slots.iter().collect();
    write_element(&head, &node.kind, &node.children, &slots, indent, lines);
}

fn write_element(
    head: &str,
    kind: &str,
    children: &[Child],
    slots: &[(&String, &Vec<Child>)],
    indent: &str,
    lines: &mut Vec<String>,
) {
    match (children, slots.is_empty()) {
        ([], true) => lines.push(format!("{indent}{head}/>")),
        ([Child::Text(only)], true) => {
            lines.push(format!("{indent}{head}>{}</{kind}>", escape_text(only)));
        }
        _ => {
            lines.push(format!("{indent}{head}>"));
            let inner = format!("{indent}{INDENT}");
            for child in children {
                match child {
                    Child::Text(t) => lines.push(format!("{inner}{}", escape_text(t))),
                    Child::Node(n) => write_node(n, &inner, lines, None),
                }
            }
            for (name, list) in slots {
                let head = format!("<{SLOT} name=\"{}\"", escape_attribute(name));
                write_element(&head, SLOT, list, &[], &inner, lines);
            }
            lines.push(format!("{indent}</{kind}>"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Value;

    #[test]
    fn markup_is_canonical_and_escaped() {
        let mut text = Node::new("text");
        text.children = vec![Child::Text("a < b & c > d".into())];
        let mut root = Node::new("screen");
        root.id = Some("s".into());
        root.props
            .insert("title".into(), Value::String("{x}\n\"q\"".into()));
        root.on.insert("load".into(), "init".into());
        root.slots.insert(
            "footer".into(),
            vec![Child::Node(Box::new(Node::new("divider")))],
        );
        root.children = vec![Child::Node(Box::new(text))];
        let doc = Document {
            weft: "0.1".into(),
            root,
        };
        assert_eq!(
            serialize(&doc),
            "<screen id=\"s\" title=\"{{x}&#10;&quot;q&quot;\" weft=\"0.1\" on-load=\"init\">\n  <text>a &lt; b &amp; c &gt; d</text>\n  <slot name=\"footer\">\n    <divider/>\n  </slot>\n</screen>\n"
        );
    }
}
