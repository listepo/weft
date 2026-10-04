//! The data schema check (SPEC §10.5) against the TypeScript core: the `data` cases of
//! `fixtures/differential.json` (packages/core/test/differential.ts) hold the TypeScript results.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_core::{
    Catalog, DataCheckOptions, ParseOptions, check_data, compile_data_schema, parse, parse_json,
};

const FIXTURE: &str = include_str!("fixtures/differential.json");
const SHOWN: usize = 15;

#[test]
fn data_checks_match_the_typescript_core() {
    let fixture = parse_json(FIXTURE).unwrap();
    let catalog = |name: &str| -> Catalog {
        serde_json::from_value(fixture["catalogs"][name].clone()).unwrap()
    };
    let catalogs = [("test", catalog("test")), ("core", catalog("core"))];
    let cases = fixture["data"].as_array().unwrap();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    for case in cases {
        let name = case["catalog"].as_str().unwrap();
        let catalog = &catalogs.iter().find(|(n, _)| *n == name).unwrap().1;
        let (schema, problems) = compile_data_schema(&case["schema"]);
        let markup = case["markup"].as_str().unwrap();
        let parsed = parse(
            markup,
            &ParseOptions {
                catalog: Some(catalog),
                ..Default::default()
            },
        );
        let diagnostics = parsed.document.as_ref().map_or_else(Vec::new, |d| {
            check_data(
                d,
                &DataCheckOptions {
                    catalog,
                    data: &schema,
                },
            )
        });
        let problems: Vec<Json> = problems
            .iter()
            .map(|p| json!({ "code": p.code.as_str(), "pointer": p.pointer, "message": p.message }))
            .collect();
        let got = json!({
            "problems": problems,
            "diagnostics": serde_json::to_value(&diagnostics).unwrap(),
        });
        if got != case["expect"] {
            failures.push(format!(
                "{}\n  expected: {}\n  got:      {got}",
                case["name"], case["expect"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} data cases differ from the TypeScript core:\n{}",
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

/// Not a fixture case, so that the fixture stays within serde_json's default recursion limit;
/// packages/core/test/data.test.ts makes the same claim.
#[test]
fn schemas_stop_at_256_levels() {
    let mut schema = json!({ "type": "string" });
    for _ in 0..300 {
        schema = json!({ "type": "object", "properties": { "a": schema } });
    }
    let (_, problems) = compile_data_schema(&schema);
    let pointer = "/properties/a".repeat(257);
    let got: Vec<Json> = problems
        .iter()
        .map(|p| json!({ "code": p.code.as_str(), "pointer": p.pointer, "message": p.message }))
        .collect();
    assert_eq!(
        got,
        vec![json!({
            "code": "W709",
            "pointer": pointer,
            "message": format!("Schemas nest deeper than 256 levels at {pointer}."),
        })]
    );
}
