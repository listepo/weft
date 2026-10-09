//! Claims about the shared data contracts of SPEC §3, §5 and §7 and the helpers on them.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use serde_json::json;
use weft_core::{
    ARIA_ROLES, Catalog, Child, ComponentDef, Content, Document, MAX_DEPTH, Node, PropDef,
    PropDefault, PropType, SlotDef, Value, WEFT_VERSION,
};

#[test]
fn the_format_version_and_the_depth_limit_are_the_ones_of_the_spec() {
    assert_eq!(WEFT_VERSION, "0.2");
    assert_eq!(MAX_DEPTH, 256);
}

#[test]
fn values_serialize_as_the_canonical_json_forms() {
    let to = |v: &Value| serde_json::to_value(v).unwrap();
    assert_eq!(to(&Value::String("x".into())), json!("x"));
    assert_eq!(to(&Value::Number(2.5)), json!(2.5));
    assert_eq!(to(&Value::Bool(true)), json!(true));
    assert_eq!(
        to(&Value::Bind {
            bind: "$.a".into(),
            not: false
        }),
        json!({"bind": "$.a"})
    );
    assert_eq!(
        to(&Value::Bind {
            bind: "$.a".into(),
            not: true
        }),
        json!({"bind": "$.a", "not": true})
    );
    assert_eq!(
        to(&Value::Token("space.md".into())),
        json!({"token": "space.md"})
    );
}

#[test]
fn a_node_serializes_only_its_non_empty_members_in_spec_order() {
    let n = Node::new("stack");
    assert_eq!(serde_json::to_string(&n).unwrap(), r#"{"kind":"stack"}"#);
    let mut n = Node::new("stack");
    n.id = Some("a".into());
    n.children = vec![
        Child::Text("t".into()),
        Child::Node(Box::new(Node::new("x"))),
    ];
    assert_eq!(
        serde_json::to_string(&n).unwrap(),
        r#"{"kind":"stack","id":"a","children":["t",{"kind":"x"}]}"#
    );
}

#[test]
fn a_document_serializes_the_version_before_the_root() {
    let d = Document {
        weft: "0.1".into(),
        context: Vec::new(),
        root: Node::new("screen"),
    };
    assert_eq!(
        serde_json::to_string(&d).unwrap(),
        r#"{"weft":"0.1","root":{"kind":"screen"}}"#
    );
}

#[test]
fn child_as_node_returns_elements_only() {
    let node = Child::Node(Box::new(Node::new("x")));
    assert_eq!(node.as_node().map(|n| n.kind.as_str()), Some("x"));
    assert!(Child::Text("t".into()).as_node().is_none());
}

#[test]
fn nodes_compare_by_content_and_ignore_source_positions() {
    let a = common::document(&common::screen("<stack id=\"a\"/>"));
    let mut b = Document {
        weft: "0.1".into(),
        context: Vec::new(),
        root: Node::new("screen"),
    };
    b.root.id = Some("root".into());
    let mut stack = Node::new("stack");
    stack.id = Some("a".into());
    b.root.children = vec![Child::Node(Box::new(stack))];
    assert_eq!(a, b);
}

#[test]
fn prop_types_have_their_catalog_names() {
    for (ty, name) in [
        (PropType::String, "string"),
        (PropType::Number, "number"),
        (PropType::Boolean, "boolean"),
        (PropType::Enum, "enum"),
        (PropType::Token, "token"),
    ] {
        assert_eq!(ty.as_str(), name);
        assert_eq!(serde_json::to_value(ty).unwrap(), json!(name));
        assert_eq!(serde_json::from_value::<PropType>(json!(name)).unwrap(), ty);
    }
    assert!(serde_json::from_value::<PropType>(json!("color")).is_err());
}

#[test]
fn content_models_use_lowercase_names() {
    for (c, name) in [
        (Content::None, "none"),
        (Content::Text, "text"),
        (Content::Nodes, "nodes"),
        (Content::Mixed, "mixed"),
    ] {
        assert_eq!(serde_json::to_value(c).unwrap(), json!(name));
    }
}

#[test]
fn a_prop_definition_reads_camel_case_and_omits_what_it_does_not_set() {
    let def: PropDef = serde_json::from_value(json!({
        "description": "D", "type": "token", "tokenType": "dimension", "required": true,
        "default": 3, "bindable": false, "writable": true, "min": 0, "max": 9, "integer": true
    }))
    .unwrap();
    assert_eq!(def.token_type.as_deref(), Some("dimension"));
    assert_eq!(def.default, Some(PropDefault::Number(3.0)));
    let minimal = PropDef::new("D", PropType::String);
    assert_eq!(
        serde_json::to_value(&minimal).unwrap(),
        json!({"description": "D", "type": "string"})
    );
}

#[test]
fn defaults_read_as_boolean_number_or_string() {
    let read = |v| serde_json::from_value::<PropDefault>(v).unwrap();
    assert_eq!(read(json!(true)), PropDefault::Bool(true));
    assert_eq!(read(json!(1.5)), PropDefault::Number(1.5));
    assert_eq!(read(json!("x")), PropDefault::String("x".into()));
    assert!(serde_json::from_value::<PropDefault>(json!(null)).is_err());
}

#[test]
fn catalog_json_with_a_field_the_model_does_not_know_is_refused() {
    let bad_prop = json!({"description": "D", "type": "string", "pattern": "^a"});
    assert!(serde_json::from_value::<PropDef>(bad_prop).is_err());
    assert!(serde_json::from_value::<SlotDef>(json!({"description": "D", "extra": 1})).is_err());
    assert!(
        serde_json::from_value::<ComponentDef>(
            json!({"description": "D", "role": "none", "content": "none", "extra": 1})
        )
        .is_err()
    );
    assert!(
        serde_json::from_value::<Catalog>(
            json!({"weft": "0.1", "name": "n", "version": "1", "components": {}, "extra": 1})
        )
        .is_err()
    );
}

#[test]
fn a_component_finds_its_props_and_slots_by_name() {
    let catalog = common::catalog();
    let dialog = &catalog.components["dialog"];
    assert_eq!(
        dialog.prop("modal").map(|p| p.kind),
        Some(PropType::Boolean)
    );
    assert!(dialog.prop("nope").is_none());
    assert_eq!(dialog.slot("actions").and_then(|s| s.required), Some(true));
    assert!(dialog.slot("nope").is_none());
    // A component without props or slots has none to find.
    let item = &catalog.components["item"];
    assert!(item.prop("x").is_none());
    assert!(item.slot("x").is_none());
}

#[test]
fn aria_roles_are_sorted_unique_and_lowercase() {
    assert!(ARIA_ROLES.windows(2).all(|w| w[0] < w[1]));
    assert!(
        ARIA_ROLES
            .iter()
            .all(|r| r.chars().all(|c| c.is_ascii_lowercase()))
    );
    assert!(ARIA_ROLES.contains(&"button"));
    assert!(ARIA_ROLES.contains(&"group"));
    // Abstract roles are not usable.
    for abstract_role in [
        "command",
        "composite",
        "input",
        "landmark",
        "range",
        "roletype",
        "section",
        "select",
        "structure",
        "widget",
        "window",
    ] {
        assert!(!ARIA_ROLES.contains(&abstract_role), "{abstract_role}");
    }
}
