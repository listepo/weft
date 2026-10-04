//! The helpers against their TypeScript twins in packages/from-aria/src/build.ts, which stay
//! TypeScript for importers without WebAssembly (`@weft/figma`): `fixtures/helpers.json` holds the
//! TypeScript results (packages/from-aria/test/differential.ts), and this test reproduces them.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::core_catalog;
use weft_core::{Map, Value, parse_json, to_compact};
use weft_import::{IdState, Losses, fill_required, literal, slug};

const FIXTURE: &str = include_str!("fixtures/helpers.json");

fn fixture() -> Json {
    parse_json(FIXTURE).unwrap()
}

fn losses_json(losses: &Losses) -> String {
    to_compact(&serde_json::to_value(&losses.0).unwrap())
}

#[test]
fn slugs_match() {
    for case in fixture()["slug"].as_array().unwrap() {
        let text = case[0].as_str().unwrap();
        assert_eq!(slug(text), case[1].as_str().unwrap(), "slug({text:?})");
    }
}

#[test]
fn literals_match() {
    for case in fixture()["literal"].as_array().unwrap() {
        let text = case["text"].as_str().unwrap();
        let mut losses = Losses::default();
        assert_eq!(
            literal(&mut losses, "/p", text),
            case["out"].as_str().unwrap(),
            "literal({text:?})"
        );
        assert_eq!(
            losses_json(&losses),
            to_compact(&case["losses"]),
            "{text:?}"
        );
    }
}

#[test]
fn fresh_ids_match() {
    for case in fixture()["freshId"].as_array().unwrap() {
        let mut state = IdState::default();
        for used in case["used"].as_array().unwrap() {
            state.used.insert(used.as_str().unwrap().to_owned());
        }
        let ids: Vec<String> = case["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| state.fresh(c[0].as_str().unwrap(), c[1].as_str().unwrap()))
            .collect();
        assert_eq!(json!(ids), case["ids"], "{}", case["calls"]);
    }
}

#[test]
fn required_stand_ins_match() {
    let catalog = core_catalog().unwrap();
    let cases = fixture();
    let cases = cases["fillRequired"].as_array().unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let def = &catalog.components[case["kind"].as_str().unwrap()];
        let parent = case["parent"].as_str().map(|p| &catalog.components[p]);
        let mut losses = Losses::default();
        let mut props: Map<Value> = Map::new();
        fill_required(
            &mut losses,
            &mut props,
            def,
            parent,
            case["root"].as_bool().unwrap(),
            case["name"].as_str().unwrap(),
            "/x",
        );
        let got = to_compact(&serde_json::to_value(&props).unwrap());
        assert_eq!(got, to_compact(&case["props"]), "{case}");
        assert_eq!(losses_json(&losses), to_compact(&case["losses"]), "{case}");
    }
}
