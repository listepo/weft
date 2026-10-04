//! The Rust catalog crate against the TypeScript package: `fixtures/differential.json` holds the
//! TypeScript results for every case (packages/catalog/test/differential.ts), and this test
//! reproduces them. It also checks the embedded core catalog and its examples.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{CORE_CATALOG_JSON, core_catalog, diff_catalogs, load_tokens, token_types};
use weft_core::{Mode, ParseOptions, parse, parse_json};

const FIXTURE: &str = include_str!("fixtures/differential.json");
/// Enough failures to see a pattern without flooding the output.
const SHOWN: usize = 15;

fn report(failures: &[String], total: usize, what: &str) {
    assert!(
        failures.is_empty(),
        "{} of {total} {what} cases differ from the TypeScript package:\n{}",
        failures.len(),
        failures
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

fn fixture() -> Json {
    parse_json(FIXTURE).unwrap()
}

fn diff_both(previous: &Json, next: &Json) -> Json {
    json!({
        "forward": serde_json::to_value(diff_catalogs(previous, next)).unwrap(),
        "reversed": serde_json::to_value(diff_catalogs(next, previous)).unwrap(),
    })
}

/// Replays one edit of the fixture: walk `path`, then set `value` at `key` or delete `key`.
fn apply_edit(catalog: &mut Json, edit: &Json) {
    let mut target = catalog;
    for key in edit["path"].as_array().unwrap() {
        target = &mut target[key.as_str().unwrap()];
    }
    let key = edit["key"].as_str().unwrap();
    let map = target.as_object_mut().unwrap();
    match edit.get("value") {
        Some(value) => {
            map.insert(key.to_owned(), value.clone());
        }
        None => {
            map.shift_remove(key);
        }
    }
}

#[test]
fn tokens_match_the_typescript_package() {
    let fixture = fixture();
    let cases = fixture["tokens"].as_array().unwrap();
    let mut failures = Vec::new();
    for case in cases {
        let loaded = load_tokens(&case["input"]);
        let entries: Vec<Json> = loaded
            .tokens
            .iter()
            .map(|(path, token)| json!([path, token]))
            .collect();
        let got = json!({ "tokens": entries, "problems": loaded.problems });
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  input: {}\n  expected: {}\n  got:      {got}",
                case["name"], case["input"], case["expect"]
            ));
        }
    }
    report(&failures, cases.len(), "token");
}

#[test]
fn catalog_diffs_match_the_typescript_package() {
    let fixture = fixture();
    let base = &fixture["diffBase"];
    let core: Json = serde_json::from_str(CORE_CATALOG_JSON).unwrap();
    let mut failures = Vec::new();
    let rows = fixture["rows"].as_array().unwrap();
    for case in rows {
        let got = diff_both(base, &case["next"]);
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  expected: {}\n  got:      {got}",
                case["name"], case["expect"]
            ));
        }
    }
    let edits = fixture["edits"].as_array().unwrap();
    for case in edits {
        let mut next = core.clone();
        for edit in case["edits"].as_array().unwrap() {
            apply_edit(&mut next, edit);
        }
        let got = diff_both(&core, &next);
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  edits: {}\n  expected: {}\n  got:      {got}",
                case["name"], case["edits"], case["expect"]
            ));
        }
    }
    report(&failures, rows.len() + edits.len(), "diff");
}

#[test]
fn every_example_is_valid_in_strict_mode_against_the_core_catalog() {
    let catalog = core_catalog().unwrap();
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/catalog/examples"
    );
    let tokens_json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/catalog/tokens/default.tokens.json"
    ))
    .unwrap();
    let tokens = token_types(&load_tokens(&parse_json(&tokens_json).unwrap()).tokens);
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "weft") {
            continue;
        }
        let markup = std::fs::read_to_string(&path).unwrap();
        let options = ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            tokens: Some(&tokens),
            actions: None,
        };
        let result = parse(&markup, &options);
        assert!(
            result.diagnostics.is_empty(),
            "{}: {:?}",
            path.display(),
            result.diagnostics
        );
        seen += 1;
    }
    assert_eq!(
        seen,
        catalog.components.len(),
        "one example per catalog kind"
    );
}
