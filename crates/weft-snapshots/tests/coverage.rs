//! The corpus exercises the whole core catalog: every kind, prop, enum value, state, event and
//! slot, every binding form, every token type a token-typed prop accepts and every spacing token.
//! The generators, importers, snapshots and screenshots run over the corpus, so this is what makes
//! "every screen" mean "everything the format can say". Each corpus screen is also strictly valid
//! and canonical.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{Child, Node, PropType, Value, has_errors, serialize};

type Tokens = IndexMap<String, Token>;

fn walk(node: &Node, tokens: &Tokens, seen: &mut BTreeSet<String>) {
    let kind = &node.kind;
    seen.insert(format!("kind {kind}"));
    if kind == "each" {
        seen.insert("form <each>".into());
    }
    for (name, value) in &node.props {
        seen.insert(format!("prop {kind}.{name}"));
        match value {
            Value::String(v) => {
                seen.insert(format!("value {kind}.{name}={v}"));
                seen.insert("form literal".into());
                // A literal that starts with `{` can only be written escaped, as `{{`.
                if v.starts_with('{') {
                    seen.insert("form {{ escaped literal".into());
                }
                if name == "hidden" {
                    seen.insert("form literal hidden".into());
                }
            }
            Value::Number(_) | Value::Bool(_) => {
                seen.insert("form literal".into());
                if name == "hidden" {
                    seen.insert("form literal hidden".into());
                }
            }
            Value::Bind { bind, not } => {
                seen.insert(if bind.starts_with("$.") || bind == "$" {
                    "form {$.path}".into()
                } else {
                    "form {$item.path} loop path".into()
                });
                if *not {
                    seen.insert("form {!$.path}".into());
                }
                if bind
                    .split('.')
                    .any(|s| s.bytes().all(|b| b.is_ascii_digit()))
                {
                    seen.insert("form array-index segment".into());
                }
                if name == "hidden" {
                    seen.insert("form bound hidden".into());
                }
                if name == "label" {
                    seen.insert("form bound label".into());
                }
            }
            Value::Token(token) => {
                seen.insert(format!("token {token}"));
                if let Some(t) = tokens.get(token) {
                    seen.insert(format!("token type {}", t.kind));
                }
            }
        }
    }
    for event in node.on.keys() {
        seen.insert(format!("event {kind}.{event}"));
    }
    for (slot, children) in &node.slots {
        seen.insert(format!("slot {kind}.{slot}"));
        walk_children(children, tokens, seen);
    }
    walk_children(&node.children, tokens, seen);
}

fn walk_children(children: &[Child], tokens: &Tokens, seen: &mut BTreeSet<String>) {
    for child in children {
        if let Some(node) = child.as_node() {
            walk(node, tokens, seen);
        }
    }
}

#[test]
fn every_corpus_screen_is_strictly_valid_and_canonical() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let screens = common::corpus();
    assert_eq!(screens.len(), 19);
    for screen in screens {
        let (document, diagnostics) = common::parse_strict(&screen.markup, &catalog, &tokens);
        assert!(
            !has_errors(&diagnostics),
            "{}: {diagnostics:?}",
            screen.name
        );
        assert_eq!(
            serialize(&document.unwrap()),
            screen.markup,
            "{}",
            screen.name
        );
        assert!(
            screen.path.with_file_name("data.json").is_file(),
            "{}: data.json",
            screen.name
        );
    }
}

#[test]
fn the_corpus_uses_the_whole_catalog() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut need = BTreeSet::new();
    let mut token_types = BTreeSet::new();
    for (kind, def) in &catalog.components {
        need.insert(format!("kind {kind}"));
        for (name, prop) in def.props.iter().flatten() {
            // `screen.weft` is the document's version, held by the document, not by a prop.
            if kind == "screen" && name == "weft" {
                continue;
            }
            need.insert(format!("prop {kind}.{name}"));
            if prop.kind == PropType::Enum {
                for value in prop.values.iter().flatten() {
                    need.insert(format!("value {kind}.{name}={value}"));
                }
            }
            if prop.kind == PropType::Token {
                token_types.extend(prop.token_type.clone());
            }
        }
        if def.requires_label == Some(true) {
            need.insert(format!("prop {kind}.label"));
        }
        for state in def.states.iter().flatten() {
            need.insert(format!("value {kind}.state={state}"));
        }
        for event in def.events.iter().flatten() {
            need.insert(format!("event {kind}.{event}"));
        }
        for slot in def.slots.iter().flat_map(|s| s.keys()) {
            need.insert(format!("slot {kind}.{slot}"));
        }
    }
    // The catalog's token props take `dimension` tokens only (gap); every spacing step is used
    // as well, since those are the tokens a gap is meant to take.
    assert!(!token_types.is_empty());
    for kind in &token_types {
        need.insert(format!("token type {kind}"));
    }
    for path in tokens.keys().filter(|p| p.starts_with("space.")) {
        need.insert(format!("token {path}"));
    }
    for form in [
        "literal",
        "{{ escaped literal",
        "{$.path}",
        "{!$.path}",
        "{$item.path} loop path",
        "array-index segment",
        "<each>",
        "literal hidden",
        "bound hidden",
        "bound label",
    ] {
        need.insert(format!("form {form}"));
    }

    let mut seen = BTreeSet::new();
    for screen in common::corpus() {
        let (document, _) = common::parse_strict(&screen.markup, &catalog, &tokens);
        walk(&document.unwrap().root, &tokens, &mut seen);
    }
    let missing: Vec<_> = need.difference(&seen).collect();
    assert!(missing.is_empty(), "the corpus never uses:\n{missing:#?}");
}
