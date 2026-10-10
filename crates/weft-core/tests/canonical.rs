//! Claims about canonical forms (SPEC §3): one JSON text and one markup text per document, so
//! that tools can diff and hash.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{document, screen};
use weft_core::{
    Child, Document, Node, ParseOptions, Value, canonicalize, parse, serialize, stringify,
};

fn node(kind: &str, id: &str) -> Node {
    let mut n = Node::new(kind);
    n.id = Some(id.to_owned());
    n
}

fn doc(root: Node) -> Document {
    Document {
        weft: "0.1".into(),
        context: Vec::new(),
        fragments: Default::default(),
        root,
    }
}

fn text(s: &str) -> Child {
    Child::Text(s.to_owned())
}

fn element(n: Node) -> Child {
    Child::Node(Box::new(n))
}

// ---- stringify ----------------------------------------------------------------------------

#[test]
fn canonical_json_has_two_space_indentation_and_a_final_newline() {
    let s = stringify(&doc(node("screen", "s")));
    assert_eq!(
        s,
        "{\n  \"weft\": \"0.1\",\n  \"root\": {\n    \"kind\": \"screen\",\n    \"id\": \"s\"\n  }\n}\n"
    );
}

#[test]
fn canonical_json_orders_node_members_as_the_spec_lists_them() {
    let mut n = node("screen", "s");
    n.children.push(text("c"));
    n.slots.insert("x".into(), vec![text("s")]);
    n.on.insert("load".into(), "go".into());
    n.props.insert("a".into(), Value::Bool(true));
    let s = stringify(&doc(n));
    let positions: Vec<usize> = ["kind", "id", "props", "on", "slots", "children"]
        .iter()
        .map(|k| s.find(&format!("\"{k}\"")).unwrap())
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "{s}");
}

#[test]
fn canonical_json_sorts_props_on_and_slots_by_utf16_code_unit() {
    let mut n = node("screen", "s");
    for key in ["b", "a", "B", "\u{FF61}", "\u{1F600}", "a-b", "a0"] {
        n.props.insert(key.into(), Value::Bool(true));
        n.on.insert(key.into(), "go".into());
        n.slots.insert(key.into(), vec![text("x")]);
    }
    let canonical = canonicalize(&doc(n));
    let expected = ["B", "a", "a-b", "a0", "b", "\u{1F600}", "\u{FF61}"];
    assert_eq!(canonical.root.props.keys().collect::<Vec<_>>(), expected);
    assert_eq!(canonical.root.on.keys().collect::<Vec<_>>(), expected);
    assert_eq!(canonical.root.slots.keys().collect::<Vec<_>>(), expected);
}

#[test]
fn canonical_json_lists_integer_like_keys_first_as_a_javascript_object_does() {
    let mut n = node("screen", "s");
    for key in ["b", "10", "2", "a"] {
        n.props.insert(key.into(), Value::Bool(true));
    }
    let canonical = canonicalize(&doc(n));
    assert_eq!(
        canonical.root.props.keys().collect::<Vec<_>>(),
        ["2", "10", "a", "b"]
    );
}

#[test]
fn canonical_json_omits_empty_members_and_empty_slot_lists() {
    let mut n = node("screen", "s");
    n.slots.insert("empty".into(), vec![]);
    let s = stringify(&doc(n));
    for member in ["props", "on", "slots", "children"] {
        assert!(!s.contains(member), "{member} in {s}");
    }
}

#[test]
fn canonical_json_writes_negative_zero_as_zero_and_numbers_as_javascript_does() {
    let mut n = node("screen", "s");
    n.props.insert("z".into(), Value::Number(-0.0));
    n.props.insert("big".into(), Value::Number(1e21));
    n.props.insert("small".into(), Value::Number(1e-7));
    n.props.insert("half".into(), Value::Number(0.5));
    let s = stringify(&doc(n));
    assert!(s.contains("\"z\": 0\n") || s.contains("\"z\": 0,"), "{s}");
    assert!(s.contains("\"big\": 1e+21"), "{s}");
    assert!(s.contains("\"small\": 1e-7"), "{s}");
    assert!(s.contains("\"half\": 0.5"), "{s}");
}

#[test]
fn canonical_json_normalizes_text_and_joins_adjacent_runs() {
    let mut n = node("text", "t");
    n.children = vec![
        text("  a \n b "),
        text("c"),
        text("   "),
        element(node("x", "x")),
        text("d"),
    ];
    let canonical = canonicalize(&doc(n));
    assert_eq!(canonical.root.children.len(), 3);
    assert_eq!(canonical.root.children[0], text("a b c"));
    assert_eq!(canonical.root.children[2], text("d"));
}

#[test]
fn canonical_json_keeps_values_equal_to_the_catalog_default_as_written() {
    let doc = document(&screen("<stack id=\"a\" direction=\"column\"/>"));
    assert!(stringify(&doc).contains("\"direction\": \"column\""));
}

#[test]
fn canonical_json_writes_references_as_objects() {
    let mut n = node("screen", "s");
    n.props.insert(
        "a".into(),
        Value::Bind {
            bind: "$.x".into(),
            not: false,
        },
    );
    n.props.insert(
        "b".into(),
        Value::Bind {
            bind: "$.y".into(),
            not: true,
        },
    );
    n.props.insert("c".into(), Value::Token("space.md".into()));
    let s = stringify(&doc(n));
    assert!(
        s.contains("\"a\": {\n        \"bind\": \"$.x\"\n      }"),
        "{s}"
    );
    assert!(
        s.contains("\"bind\": \"$.y\",\n        \"not\": true"),
        "{s}"
    );
    assert!(s.contains("\"token\": \"space.md\""), "{s}");
}

#[test]
fn canonicalize_drops_markup_positions() {
    let parsed = document(&screen("<stack id=\"a\"/>"));
    assert!(parsed.root.source.0.is_some());
    let canonical = canonicalize(&parsed);
    assert!(canonical.root.source.0.is_none());
    assert_eq!(canonical, parsed);
}

#[test]
fn canonicalize_is_idempotent_and_stringify_is_stable_over_it() {
    let mut n = node("screen", "s");
    n.props.insert("z".into(), Value::Number(-0.0));
    n.props.insert("a".into(), Value::String("x".into()));
    n.children = vec![text(" a  b "), text("c")];
    let once = canonicalize(&doc(n));
    let twice = canonicalize(&once);
    assert_eq!(once, twice);
    assert_eq!(stringify(&once), stringify(&twice));
    assert_eq!(stringify(&once), stringify(&doc(once.root.clone())));
}

#[test]
fn canonicalize_does_not_change_its_input() {
    let mut n = node("screen", "s");
    n.children = vec![text(" a ")];
    let d = doc(n);
    let before = d.clone();
    let _ = canonicalize(&d);
    assert_eq!(d, before);
}

#[test]
fn canonicalize_normalizes_text_inside_slots_too() {
    let mut n = node("screen", "s");
    n.slots.insert("x".into(), vec![text("  a  "), text("b")]);
    assert_eq!(canonicalize(&doc(n)).root.slots["x"], [text("a b")]);
}

// ---- serialize ----------------------------------------------------------------------------

#[test]
fn markup_ends_with_a_newline_and_indents_two_spaces_per_level() {
    let mut inner = node("text", "t");
    inner.children = vec![text("x")];
    let mut n = node("screen", "s");
    n.children = vec![element(inner)];
    assert_eq!(
        serialize(&doc(n)),
        "<screen id=\"s\" weft=\"0.1\">\n  <text id=\"t\">x</text>\n</screen>\n"
    );
}

#[test]
fn an_element_with_no_content_is_self_closing_and_one_text_child_stays_on_the_line() {
    let n = node("screen", "s");
    assert_eq!(serialize(&doc(n)), "<screen id=\"s\" weft=\"0.1\"/>\n");
}

#[test]
fn mixed_content_puts_every_child_on_its_own_line() {
    let mut n = node("item", "i");
    n.children = vec![text("a"), element(node("x", "x")), text("b")];
    let s = serialize(&doc(n));
    assert_eq!(
        s,
        "<item id=\"i\" weft=\"0.1\">\n  a\n  <x id=\"x\"/>\n  b\n</item>\n"
    );
}

#[test]
fn attributes_are_id_then_sorted_props_with_the_version_then_sorted_events() {
    let mut n = node("screen", "s");
    n.props.insert("zeta".into(), Value::Bool(true));
    n.props.insert("alpha".into(), Value::Bool(false));
    n.props.insert("weft-x".into(), Value::Number(1.0));
    n.on.insert("zoom".into(), "a".into());
    n.on.insert("click".into(), "b".into());
    assert_eq!(
        serialize(&doc(n)),
        "<screen id=\"s\" alpha=\"false\" weft=\"0.1\" weft-x=\"1\" zeta=\"true\" on-click=\"b\" on-zoom=\"a\"/>\n"
    );
}

#[test]
fn a_document_without_a_version_writes_no_version_attribute() {
    let d = Document {
        weft: String::new(),
        context: Vec::new(),
        fragments: Default::default(),
        root: node("screen", "s"),
    };
    assert_eq!(serialize(&d), "<screen id=\"s\"/>\n");
}

#[test]
fn named_slots_come_after_the_default_content_sorted_by_name() {
    let mut n = node("form", "f");
    n.children = vec![element(node("a", "a"))];
    n.slots.insert("zz".into(), vec![element(node("z", "z"))]);
    n.slots.insert("footer".into(), vec![text("t")]);
    assert_eq!(
        serialize(&doc(n)),
        "<form id=\"f\" weft=\"0.1\">\n  <a id=\"a\"/>\n  <slot name=\"footer\">t</slot>\n  <slot name=\"zz\">\n    <z id=\"z\"/>\n  </slot>\n</form>\n"
    );
}

#[test]
fn attribute_values_escape_ampersand_less_than_quote_tab_and_line_breaks_only() {
    let mut n = node("screen", "s");
    n.props
        .insert("t".into(), Value::String("a&b<c>d\"e'f\tg\nh\ri".into()));
    assert_eq!(
        serialize(&doc(n)),
        "<screen id=\"s\" t=\"a&amp;b&lt;c>d&quot;e'f&#9;g&#10;h&#13;i\" weft=\"0.1\"/>\n"
    );
}

#[test]
fn text_escapes_ampersand_less_than_and_greater_than_only() {
    let mut n = node("text", "t");
    n.children = vec![text("a&b<c>d\"e'f]]>g")];
    assert_eq!(
        serialize(&doc(n)),
        "<text id=\"t\" weft=\"0.1\">a&amp;b&lt;c&gt;d\"e'f]]&gt;g</text>\n"
    );
}

#[test]
fn a_string_that_starts_with_a_brace_is_written_with_a_doubled_brace() {
    let mut n = node("screen", "s");
    n.props.insert("a".into(), Value::String("{$.x}".into()));
    n.props.insert("b".into(), Value::String("{{".into()));
    n.props.insert("c".into(), Value::String("x{".into()));
    assert_eq!(
        serialize(&doc(n)),
        "<screen id=\"s\" a=\"{{$.x}\" b=\"{{{\" c=\"x{\" weft=\"0.1\"/>\n"
    );
}

#[test]
fn references_and_scalars_are_written_in_their_attribute_forms() {
    let mut n = node("screen", "s");
    n.props.insert(
        "a".into(),
        Value::Bind {
            bind: "$.x".into(),
            not: false,
        },
    );
    n.props.insert(
        "b".into(),
        Value::Bind {
            bind: "$.y".into(),
            not: true,
        },
    );
    n.props.insert("c".into(), Value::Token("space.md".into()));
    n.props.insert("d".into(), Value::Number(-0.0));
    n.props.insert("e".into(), Value::Number(1e21));
    n.props.insert("f".into(), Value::Bool(false));
    assert_eq!(
        serialize(&doc(n)),
        "<screen id=\"s\" a=\"{$.x}\" b=\"{!$.y}\" c=\"{token.space.md}\" d=\"0\" e=\"1e+21\" f=\"false\" weft=\"0.1\"/>\n"
    );
}

#[test]
fn serialize_canonicalizes_first_so_equal_documents_give_equal_text() {
    let mut a = node("text", "t");
    a.children = vec![text("  x   y ")];
    a.props.insert("b".into(), Value::Bool(true));
    a.props.insert("a".into(), Value::Bool(true));
    let mut b = node("text", "t");
    b.children = vec![text("x y")];
    b.props.insert("a".into(), Value::Bool(true));
    b.props.insert("b".into(), Value::Bool(true));
    assert_eq!(serialize(&doc(a)), serialize(&doc(b)));
}

#[test]
fn markup_survives_a_round_trip_through_the_parser() {
    let markup = "<screen id=\"s\" weft=\"0.1\">\n  <form id=\"f\" on-submit=\"go\">\n    <button id=\"b\" label=\"a &amp; &lt;b> &#9; {{x}\" submit=\"true\">T &amp; C</button>\n    <slot name=\"footer\">\n      <link id=\"l\">x</link>\n    </slot>\n  </form>\n</screen>\n";
    let parsed = document(markup);
    assert_eq!(serialize(&parsed), markup);
}

#[test]
fn text_that_looks_like_markup_survives_a_round_trip() {
    let mut n = node("text", "t");
    n.children = vec![text("<b>bold</b> & ]]> {$.x} &amp;")];
    let original = doc(n);
    let markup = serialize(&original);
    let back = parse(&markup, &ParseOptions::default());
    assert!(
        back.diagnostics.is_empty(),
        "{markup}: {:?}",
        back.diagnostics
    );
    assert_eq!(back.document.unwrap(), original);
}

#[test]
fn attribute_values_with_whitespace_and_references_survive_a_round_trip() {
    let mut n = node("screen", "s");
    n.props.insert(
        "v".into(),
        Value::String(" \t lead & trail \n\r{x} <\"'> ".into()),
    );
    let original = doc(n);
    let back = parse(&serialize(&original), &ParseOptions::default());
    assert!(back.diagnostics.is_empty(), "{:?}", back.diagnostics);
    assert_eq!(back.document.unwrap(), original);
}

#[test]
fn serialize_is_idempotent_under_parse() {
    let markup = serialize(&document(&screen(
        "<stack id=\"a\" gap=\"{token.space.sm}\"><text id=\"t\">  hi   there </text></stack>",
    )));
    let again = serialize(&document(&markup));
    assert_eq!(markup, again);
}
