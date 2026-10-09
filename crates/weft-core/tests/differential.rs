//! The Rust core against the TypeScript core: `fixtures/differential.json` holds the TypeScript
//! results for every case (packages/core/test/differential.ts), and this test reproduces them.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_core::{
    ApplyOptions, Author, Catalog, Mode, ParseOptions, ValidateOptions, apply_patches, parse,
    parse_json, serialize, stringify, validate,
};

const FIXTURE: &str = include_str!("fixtures/differential.json");
/// Enough failures to see a pattern without flooding the output.
const SHOWN: usize = 15;

struct Context {
    catalogs: IndexMap<String, Catalog>,
    tokens: IndexMap<String, String>,
}

fn mode(case: &Json) -> Mode {
    if case["mode"] == "strict" {
        Mode::Strict
    } else {
        Mode::Lenient
    }
}

fn actions(case: &Json) -> Option<Vec<String>> {
    case["actions"].as_array().map(|a| {
        a.iter()
            .filter_map(|s| s.as_str().map(str::to_owned))
            .collect()
    })
}

fn to_json<T: serde::Serialize>(value: &T) -> Json {
    serde_json::to_value(value).unwrap_or(Json::Null)
}

fn report(failures: &[String], total: usize, what: &str) {
    assert!(
        failures.is_empty(),
        "{} of {total} {what} cases differ from the TypeScript core:\n{}",
        failures.len(),
        failures
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

fn load() -> (Json, Context) {
    let fixture = parse_json(FIXTURE).unwrap_or(Json::Null);
    let catalogs = fixture["catalogs"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    serde_json::from_value::<Catalog>(v.clone())
                        .ok()
                        .map(|c| (k.clone(), c))
                })
                .collect()
        })
        .unwrap_or_default();
    let tokens = fixture["tokens"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_owned()))
                .collect()
        })
        .unwrap_or_default();
    (fixture, Context { catalogs, tokens })
}

#[test]
fn fixture_catalogs_load() {
    let (_, ctx) = load();
    assert_eq!(ctx.catalogs.keys().collect::<Vec<_>>(), ["test", "core"]);
}

#[test]
fn markup_matches_the_typescript_core() {
    let (fixture, ctx) = load();
    let cases = fixture["markup"].as_array().cloned().unwrap_or_default();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    for case in &cases {
        let actions = actions(case);
        let options = ParseOptions {
            catalog: case["catalog"].as_str().and_then(|c| ctx.catalogs.get(c)),
            mode: mode(case),
            tokens: (case["tokens"] == true).then_some(&ctx.tokens),
            actions: actions.as_deref(),
        };
        let markup = case["markup"].as_str().unwrap_or_default();
        let result = parse(markup, &options);
        let got = serde_json::json!({
            "diagnostics": to_json(&result.diagnostics),
            "json": result.document.as_ref().map(stringify),
            "markup": result.document.as_ref().map(serialize),
        });
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  markup: {markup:?}\n  expected: {}\n  got:      {got}",
                case["name"], case["expect"]
            ));
        }
    }
    report(&failures, cases.len(), "markup");
}

#[test]
fn json_matches_the_typescript_core() {
    let (fixture, ctx) = load();
    let cases = fixture["json"].as_array().cloned().unwrap_or_default();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    for case in &cases {
        let actions = actions(case);
        let catalog = case["catalog"].as_str().and_then(|c| ctx.catalogs.get(c));
        let options = ValidateOptions {
            catalog,
            mode: mode(case),
            tokens: (case["tokens"] == true).then_some(&ctx.tokens),
            actions: actions.as_deref(),
        };
        // JSON.stringify drops object keys whose value is undefined; the fixture already did.
        let diagnostics = if catalog.is_some() {
            validate(&case["json"], &options)
        } else {
            vec![]
        };
        let got = serde_json::json!({ "diagnostics": to_json(&diagnostics) });
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  json: {}\n  expected: {}\n  got:      {got}",
                case["name"], case["json"], case["expect"]
            ));
        }
    }
    report(&failures, cases.len(), "JSON");
}

#[test]
fn patches_match_the_typescript_core() {
    let (fixture, ctx) = load();
    let Some(catalog) = ctx.catalogs.get("test") else {
        panic!("no test catalog")
    };
    let base_markup = fixture["patchBase"].as_str().unwrap_or_default();
    let base = parse(
        base_markup,
        &ParseOptions {
            catalog: Some(catalog),
            tokens: Some(&ctx.tokens),
            mode: Mode::Strict,
            actions: None,
        },
    );
    assert!(base.diagnostics.is_empty(), "{:?}", base.diagnostics);
    let Some(base) = base.document else {
        panic!("the patch base does not parse")
    };
    let cases = fixture["patches"].as_array().cloned().unwrap_or_default();
    let mut failures = Vec::new();
    for case in &cases {
        let author = case.get("author").map(|a| Author {
            by: a["by"].as_str().unwrap_or_default().to_owned(),
            name: a.get("name").and_then(Json::as_str).map(str::to_owned),
        });
        let options = ApplyOptions {
            catalog,
            mode: mode(case),
            tokens: Some(&ctx.tokens),
            actions: None,
            author: author.as_ref(),
            read_only_context: case.get("context").and_then(Json::as_str) == Some("read-only"),
        };
        let result = apply_patches(&base, &case["patches"], &options);
        let got = serde_json::json!({
            "diagnostics": to_json(&result.diagnostics),
            "markup": result.document.as_ref().map(serialize),
        });
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  patches: {}\n  expected: {}\n  got:      {got}",
                case["name"], case["patches"], case["expect"]
            ));
        }
    }
    report(&failures, cases.len(), "patch");
}
