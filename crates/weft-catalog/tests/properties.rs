//! Properties that hold for every input: nothing panics, a catalog never differs from itself, and
//! the diff of two catalogs mirrors the diff of the same two in the other order. The
//! configuration is fixed so a failure reproduces on every machine.

#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, RngSeed};
use serde_json::{Map, Value as Json, json};
use weft_catalog::{ChangeLevel, TokenCode, diff_catalogs, load_tokens, token_types};

fn config() -> Config {
    Config {
        cases: 128,
        max_shrink_iters: 512,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0x7013),
        ..Config::default()
    }
}

fn scalar() -> impl Strategy<Value = Json> {
    prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i32>().prop_map(|n| json!(n)),
        (-1.0e3..1.0e3f64).prop_map(|n| json!(n)),
        "\\PC{0,8}".prop_map(Json::String),
        Just(json!("{a}")),
        Just(json!("{a.b}")),
        Just(json!("$type")),
    ]
}

fn any_json() -> impl Strategy<Value = Json> {
    let key = prop_oneof![
        Just("$type".to_owned()),
        Just("$value".to_owned()),
        Just("$root".to_owned()),
        Just("components".to_owned()),
        Just("props".to_owned()),
        Just("slots".to_owned()),
        Just("values".to_owned()),
        Just("min".to_owned()),
        Just("a".to_owned()),
        Just("b".to_owned()),
        "[a-z0-9.{}$]{0,5}",
    ];
    scalar().prop_recursive(5, 48, 5, move |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Json::Array),
            prop::collection::vec((key.clone(), inner), 0..5)
                .prop_map(|m| Json::Object(m.into_iter().collect::<Map<_, _>>())),
        ]
    })
}

/// A token file over a small pool of names, so that aliases hit, miss and loop.
fn token_file() -> impl Strategy<Value = Json> {
    let name = prop_oneof![Just("a"), Just("b"), Just("c"), Just("g.a"), Just("g.b")];
    let value = prop_oneof![
        name.clone().prop_map(|n| json!(format!("{{{n}}}"))),
        any::<i8>().prop_map(|n| json!(n)),
        Just(json!("x")),
    ];
    let ty = prop_oneof![Just(None), Just(Some("number")), Just(Some("color"))];
    let token = (value, ty).prop_map(|(v, t)| {
        let mut m = Map::new();
        m.insert("$value".into(), v);
        if let Some(t) = t {
            m.insert("$type".into(), json!(t));
        }
        Json::Object(m)
    });
    (
        prop::collection::vec(token.clone(), 3),
        prop::collection::vec(token, 2),
        prop::option::of(Just("number")),
    )
        .prop_map(|(top, group, ty)| {
            let mut g = Map::new();
            for (n, t) in ["a", "b"].iter().zip(group) {
                g.insert((*n).into(), t);
            }
            let mut root = Map::new();
            if let Some(ty) = ty {
                root.insert("$type".into(), json!(ty));
            }
            for (n, t) in ["a", "b", "c"].iter().zip(top) {
                root.insert((*n).into(), t);
            }
            root.insert("g".into(), Json::Object(g));
            Json::Object(root)
        })
}

fn component() -> impl Strategy<Value = Json> {
    let names = || prop::collection::vec(prop_oneof![Just("a"), Just("b"), Just("c")], 0..3);
    let prop_def = (
        prop_oneof![Just("string"), Just("number"), Just("enum")],
        names(),
        prop::option::of(0i32..4),
        prop::option::of(5i32..9),
        prop::option::of(any::<bool>()),
        prop::option::of(any::<bool>()),
        prop::option::of(any::<bool>()),
    )
        .prop_map(|(ty, values, min, max, required, bindable, writable)| {
            let mut m = Map::new();
            m.insert("description".into(), json!("d"));
            m.insert("type".into(), json!(ty));
            m.insert("values".into(), json!(values));
            for (k, v) in [("min", min), ("max", max)] {
                if let Some(v) = v {
                    m.insert(k.into(), json!(v));
                }
            }
            for (k, v) in [
                ("required", required),
                ("bindable", bindable),
                ("writable", writable),
            ] {
                if let Some(v) = v {
                    m.insert(k.into(), json!(v));
                }
            }
            Json::Object(m)
        });
    let props = prop::collection::btree_map(
        prop_oneof![Just("p"), Just("q"), Just("r")].prop_map(String::from),
        prop_def,
        0..3,
    );
    (
        prop_oneof![Just("none"), Just("text"), Just("nodes"), Just("mixed")],
        prop_oneof![Just("none"), Just("group"), Just("button")],
        prop::option::of(names()),
        props,
        names(),
        names(),
        any::<bool>(),
    )
        .prop_map(|(content, role, children, props, states, events, label)| {
            let mut m = Map::new();
            m.insert("description".into(), json!("d"));
            m.insert("role".into(), json!(role));
            m.insert("content".into(), json!(content));
            if let Some(c) = children {
                m.insert("allowedChildren".into(), json!(c));
            }
            m.insert("requiresLabel".into(), json!(label));
            m.insert("props".into(), json!(props));
            m.insert("states".into(), json!(states));
            m.insert("events".into(), json!(events));
            Json::Object(m)
        })
}

fn catalog() -> impl Strategy<Value = Json> {
    prop::collection::btree_map(
        prop_oneof![Just("x"), Just("y"), Just("z"), Just("7")].prop_map(String::from),
        component(),
        0..4,
    )
    .prop_map(
        |components| json!({"weft": "0.1", "name": "n", "version": "1", "components": components}),
    )
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn loading_tokens_never_panics_on_arbitrary_json(input in any_json()) {
        let _ = load_tokens(&input);
    }

    #[test]
    fn loading_tokens_never_panics_on_token_shaped_json(input in token_file()) {
        let loaded = load_tokens(&input);
        // Everything that resolved is typed, and the type table lists exactly those tokens.
        let table = token_types(&loaded.tokens);
        prop_assert_eq!(table.keys().collect::<Vec<_>>(), loaded.tokens.keys().collect::<Vec<_>>());
        // A token is either resolved or explained: the ones left out have a problem at their path
        // or lead through one that has.
        let has_problem = |path: &str| loaded.problems.iter().any(|p| p.path == path);
        for path in ["a", "b", "c", "g.a", "g.b"] {
            if input.pointer(&format!("/{}", path.replace('.', "/"))).is_some() {
                prop_assert!(loaded.tokens.contains_key(path) || has_problem(path), "{path}: {:?}", loaded.problems);
            }
        }
        // A cycle is reported once however many tokens lead into it.
        let cycles = loaded.problems.iter().filter(|p| p.code == TokenCode::T005).count();
        prop_assert!(cycles <= 3);
        // Loading twice gives the same answer.
        prop_assert_eq!(load_tokens(&input), loaded);
    }

    #[test]
    fn resolved_aliases_carry_the_value_of_a_token_that_is_not_an_alias(input in token_file()) {
        let loaded = load_tokens(&input);
        for (path, token) in &loaded.tokens {
            let is_alias = token.value.as_str().is_some_and(|s| {
                s.len() > 2 && s.starts_with('{') && s.ends_with('}') && !s[1..s.len() - 1].contains(['{', '}'])
            });
            prop_assert!(!is_alias, "{path} kept an alias as its value: {}", token.value);
        }
    }

    #[test]
    fn diffing_never_panics_on_arbitrary_json(a in any_json(), b in any_json()) {
        let _ = diff_catalogs(&a, &b);
    }

    #[test]
    fn a_catalog_never_differs_from_itself(a in any_json()) {
        let d = diff_catalogs(&a, &a);
        prop_assert_eq!(d.level, ChangeLevel::None);
        prop_assert!(d.changes.is_empty());
    }

    #[test]
    fn the_level_is_the_highest_level_of_the_changes(a in catalog(), b in catalog()) {
        let d = diff_catalogs(&a, &b);
        let top = d.changes.iter().map(|c| c.level).max().unwrap_or(ChangeLevel::None);
        prop_assert_eq!(d.level, top);
        prop_assert!(d.changes.iter().all(|c| c.path.starts_with("components.")));
    }

    #[test]
    fn what_was_removed_one_way_was_added_the_other_way(a in catalog(), b in catalog()) {
        let (forward, backward) = (diff_catalogs(&a, &b), diff_catalogs(&b, &a));
        for noun in ["Component", "Prop", "Slot", "State", "Event", "Enum value"] {
            let removed = |d: &weft_catalog::CatalogDiff| d.changes.iter().filter(|c| c.message.starts_with(noun) && c.message.ends_with("was removed.")).count();
            let added = |d: &weft_catalog::CatalogDiff| d.changes.iter().filter(|c| c.message.starts_with(noun) && c.message.ends_with("was added.")).count();
            prop_assert_eq!(removed(&forward), added(&backward), "{}", noun);
            prop_assert_eq!(added(&forward), removed(&backward), "{}", noun);
        }
    }

    #[test]
    fn a_component_added_is_minor_and_removed_is_major(a in catalog()) {
        let mut with = a.clone();
        with["components"]["new-one"] = json!({"description": "d", "role": "none", "content": "none"});
        let (up, down) = (diff_catalogs(&a, &with), diff_catalogs(&with, &a));
        prop_assert_eq!((up.level, up.changes.len()), (ChangeLevel::Minor, 1));
        prop_assert_eq!((down.level, down.changes.len()), (ChangeLevel::Major, 1));
    }

    #[test]
    fn the_diff_is_the_same_whatever_the_key_order_of_the_input(a in catalog(), b in catalog()) {
        // Reversing the order of the components must not change what changed, only where it is listed.
        let reverse = |c: &Json| {
            let mut r = c.clone();
            let items: Vec<_> = c["components"].as_object().unwrap().iter().rev().map(|(k, v)| (k.clone(), v.clone())).collect();
            r["components"] = Json::Object(items.into_iter().collect());
            r
        };
        let sorted = |mut d: weft_catalog::CatalogDiff| {
            d.changes.sort_by(|x, y| (x.path.as_str(), x.message.as_str()).cmp(&(y.path.as_str(), y.message.as_str())));
            d
        };
        prop_assert_eq!(sorted(diff_catalogs(&a, &b)), sorted(diff_catalogs(&reverse(&a), &reverse(&b))));
    }
}
