//! The shared document builder against the TypeScript one: `fixtures/build.json` holds what
//! `buildDocument` of @weft/from-aria returned for every case (packages/from-aria/test/
//! differential.ts), and this test reproduces it key order and all.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use serde_json::{Value as Json, json};
use weft_catalog::core_catalog;
use weft_core::{Catalog, parse_json, to_compact};
use weft_import::{BuildOptions, Note, Scalar, Sem, build_document};

const FIXTURE: &str = include_str!("fixtures/build.json");
/// Enough failures to see a pattern without flooding the output.
const SHOWN: usize = 15;

fn run(sems: &[Sem], reserved: Vec<String>, catalog: &Catalog) -> Json {
    let built = build_document(
        sems,
        BuildOptions {
            catalog,
            reserved,
            diagnostics: Vec::new(),
        },
    );
    let mut out = serde_json::to_value(&built.result).unwrap();
    out.as_object_mut()
        .unwrap()
        .insert("rootPath".into(), json!(built.root_path));
    out
}

#[test]
fn documents_match_the_typescript_builder() {
    let catalog = core_catalog().unwrap();
    let cases = parse_json(FIXTURE).unwrap();
    let cases = cases.as_array().unwrap();
    let mut failures = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let sems: Vec<Sem> = serde_json::from_value(case["sems"].clone()).unwrap();
        let reserved: Vec<String> = serde_json::from_value(case["reserved"].clone()).unwrap();
        // Compared as text: key order is part of the contract, and Value equality ignores it.
        let got = to_compact(&run(&sems, reserved, &catalog));
        let expected = to_compact(&case["result"]);
        if got != expected {
            failures.push(format!(
                "case {i}\n  sems: {}\n  expected: {expected}\n  got:      {got}",
                to_compact(&case["sems"])
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from the TypeScript builder:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

const ROLES: &[&str] = &[
    "main",
    "button",
    "link",
    "list",
    "listitem",
    "table",
    "row",
    "cell",
    "columnheader",
    "rowgroup",
    "tablist",
    "tab",
    "tabpanel",
    "checkbox",
    "radio",
    "radiogroup",
    "combobox",
    "option",
    "textbox",
    "heading",
    "text",
    "dialog",
    "generic",
    "img",
    "form",
    "spinbutton",
    "paragraph",
    "x",
];

fn scalar() -> impl Strategy<Value = Scalar> {
    prop_oneof![
        any::<bool>().prop_map(Scalar::Bool),
        (-2i32..8).prop_map(|n| Scalar::Number(f64::from(n))),
        ".{0,4}".prop_map(Scalar::String),
    ]
}

fn leaf() -> impl Strategy<Value = Sem> {
    (
        prop_oneof![4 => proptest::sample::select(ROLES).prop_map(str::to_owned), 1 => ".{0,6}"],
        ".{0,8}",
        proptest::collection::btree_map(
            proptest::sample::select(&["checked", "selected", "level", "disabled"][..]),
            scalar(),
            0..3,
        ),
        proptest::collection::btree_map(
            proptest::sample::select(&["type", "value", "href", "state", "submit"][..]),
            ".{0,6}",
            0..3,
        ),
        proptest::option::of(proptest::sample::select(&["a", "b", "screen", "1bad"][..])),
        proptest::option::of(proptest::sample::select(&["a", "b"][..])),
        proptest::option::of(proptest::collection::vec(
            proptest::sample::select(&["a", "b", "c"][..]),
            0..2,
        )),
        any::<bool>(),
    )
        .prop_map(
            |(role, name, states, props, id, reference, labelled, note)| Sem {
                role,
                name,
                states: states.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
                props: props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
                kind: None,
                id: id.map(str::to_owned),
                reference: reference.map(str::to_owned),
                labelled_by: labelled.map(|l| l.into_iter().map(str::to_owned).collect()),
                notes: if note {
                    vec![Note {
                        kind: weft_import::LossKind::Ids,
                        note: "n".into(),
                    }]
                } else {
                    Vec::new()
                },
                children: Vec::new(),
            },
        )
}

fn tree() -> impl Strategy<Value = Sem> {
    leaf().prop_recursive(5, 64, 5, |inner| {
        (leaf(), proptest::collection::vec(inner, 0..5)).prop_map(|(mut s, c)| {
            s.children = c;
            s
        })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Whatever a source hands over, the builder answers with a valid-shaped result.
    #[test]
    fn building_never_panics(top in proptest::collection::vec(tree(), 0..4), reserve in any::<bool>()) {
        let catalog = core_catalog().unwrap();
        let reserved = if reserve { vec!["a".to_owned(), "screen".to_owned()] } else { Vec::new() };
        let out = run(&top, reserved, &catalog);
        prop_assert!(out["document"]["root"]["kind"].is_string());
    }
}
