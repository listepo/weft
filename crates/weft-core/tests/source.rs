//! Claims about the positions a parsed document carries: where each element, attribute, text
//! entry and slot starts, counted in lines and UTF-16 columns from one, and absent once the
//! document is canonical.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_core::{Child, Node, Position, canonicalize};

fn at(line: u32, column: u32) -> Position {
    Position { line, column }
}

const MARKUP: &str = "<screen id=\"s\" weft=\"0.1\">\n  <form id=\"f\" on-submit=\"go\">\n    \u{1F600}<field id=\"x\" label=\"L\"/>\n    <slot name=\"footer\"><link id=\"l\">t</link></slot>\n  </form>\n  <text id=\"t\">a <!-- c --> b</text></screen>";

fn parsed() -> weft_core::Document {
    // The text inside <form> is a validation error, and the document is kept all the same.
    common::parse_lenient(MARKUP).document.unwrap()
}

fn form(doc: &weft_core::Document) -> &Node {
    doc.root.children[0].as_node().unwrap()
}

#[test]
fn an_element_starts_at_its_opening_angle_bracket() {
    let doc = parsed();
    let root = doc.root.source.0.as_ref().unwrap();
    assert_eq!(root.pos, at(1, 1));
    assert_eq!(form(&doc).source.0.as_ref().unwrap().pos, at(2, 3));
}

#[test]
fn an_attribute_starts_at_its_name() {
    let doc = parsed();
    let root = doc.root.source.0.as_ref().unwrap();
    assert_eq!(root.attrs["id"], at(1, 9));
    assert_eq!(root.attrs["weft"], at(1, 16));
    let form = form(&doc).source.0.as_ref().unwrap();
    assert_eq!(form.attrs["on-submit"], at(2, 16));
}

#[test]
fn children_positions_line_up_with_children_and_text_has_one_too() {
    let doc = parsed();
    let f = form(&doc);
    let source = f.source.0.as_ref().unwrap();
    assert_eq!(source.children.len(), f.children.len());
    assert!(matches!(f.children[0], Child::Text(_)));
    assert_eq!(source.children[0], Some(at(3, 5)));
    // The emoji is two UTF-16 units, so the next element starts two columns later.
    assert_eq!(source.children[1], Some(at(3, 7)));
    assert_eq!(
        f.children[1]
            .as_node()
            .unwrap()
            .source
            .0
            .as_ref()
            .unwrap()
            .pos,
        at(3, 7)
    );
}

#[test]
fn text_interrupted_by_a_comment_is_one_entry_that_starts_where_the_text_does() {
    let doc = parsed();
    let t = doc.root.children[1].as_node().unwrap();
    assert_eq!(t.children, [Child::Text("a b".into())]);
    assert_eq!(t.source.0.as_ref().unwrap().children, [Some(at(6, 16))]);
}

#[test]
fn a_slot_has_the_position_of_its_slot_element_and_of_its_children() {
    let doc = parsed();
    let slot = &form(&doc).source.0.as_ref().unwrap().slots["footer"];
    assert_eq!(slot.pos, at(4, 5));
    assert_eq!(slot.children, [Some(at(4, 25))]);
}

#[test]
fn a_document_that_did_not_come_from_markup_has_no_positions() {
    let doc = parsed();
    let canonical = canonicalize(&doc);
    assert!(canonical.root.source.0.is_none());
    assert!(form(&canonical).source.0.is_none());
    assert!(
        form(&canonical).children[1]
            .as_node()
            .unwrap()
            .source
            .0
            .is_none()
    );
    assert!(weft_core::Node::new("x").source.0.is_none());
}

#[test]
fn positions_do_not_take_part_in_equality() {
    let doc = parsed();
    assert_eq!(doc, canonicalize(&doc));
    assert_eq!(doc.root.source, Node::new("x").source);
}

#[test]
fn positions_follow_a_crlf_line_break_and_a_byte_order_mark_is_not_a_column() {
    let markup =
        "\u{FEFF}<screen id=\"s\" weft=\"0.1\">\r\n<stack id=\"a\"/>\r<stack id=\"b\"/></screen>";
    let doc = common::parse_lenient(markup).document.unwrap();
    assert_eq!(doc.root.source.0.as_ref().unwrap().pos, at(1, 1));
    let positions: Vec<_> = doc
        .root
        .children
        .iter()
        .map(|c| c.as_node().unwrap().source.0.as_ref().unwrap().pos)
        .collect();
    assert_eq!(positions, [at(2, 1), at(3, 1)]);
}
