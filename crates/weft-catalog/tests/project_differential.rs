//! Project loading (SPEC §10) against the TypeScript package: the `projects` cases of
//! `fixtures/differential.json` (packages/catalog/test/differential.ts) hold the TypeScript
//! results, with the catalog reduced to the kinds that differ from the core.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{ProjectOptions, core_catalog, load_project_text};
use weft_core::{parse_json, to_compact};

const FIXTURE: &str = include_str!("fixtures/differential.json");
const SHOWN: usize = 15;

/// JSON as the TypeScript side wrote it: JavaScript numbers, so `5.0` and `5` compare equal.
fn js<T: serde::Serialize>(value: &T) -> Json {
    parse_json(&to_compact(value)).unwrap()
}

#[test]
fn projects_match_the_typescript_package() {
    let fixture = parse_json(FIXTURE).unwrap();
    let shared = &fixture["projectFiles"];
    let core = js(&core_catalog().unwrap().components);
    let cases = fixture["projects"].as_array().unwrap();
    assert!(!cases.is_empty());
    let mut failures = Vec::new();
    for case in cases {
        let files = case.get("files").unwrap_or(shared);
        let read = |name: &str| files.get(name).and_then(Json::as_str).map(str::to_owned);
        let content = case.get("content").is_some();
        let options = if content {
            ProjectOptions {
                prefix: "#/project",
                ..Default::default()
            }
        } else {
            ProjectOptions {
                read: Some(&read),
                ..Default::default()
            }
        };
        let loaded = load_project_text(case["project"].as_str().unwrap(), &options).unwrap();
        let project = &loaded.project;
        let components = js(&project.catalog.components);
        let changed: Vec<Json> = components
            .as_object()
            .unwrap()
            .iter()
            .filter(|(kind, def)| core.get(kind.as_str()) != Some(def))
            .map(|(kind, def)| json!([kind, def]))
            .collect();
        let kinds: Vec<Json> = (project.kinds.iter())
            .filter(|(_, k)| k.catalog != "weft-core" || !k.extended_by.is_empty())
            .map(|(kind, k)| json!([kind, k]))
            .collect();
        let tokens = project.tokens.as_ref().map(|tokens| {
            tokens
                .iter()
                .map(|(path, token)| json!([path, token]))
                .collect::<Vec<_>>()
        });
        let got = json!({
            "catalog": {
                "name": project.catalog.name,
                "version": project.catalog.version,
                "changed": changed,
            },
            "catalogs": project.catalogs,
            "kinds": kinds,
            "tokens": tokens,
            "actions": project.actions,
            "data": project.data.is_some(),
            "settings": project.settings,
            "diagnostics": serde_json::to_value(&loaded.diagnostics).unwrap(),
        });
        if js(&got) != case["expect"] {
            failures.push(format!(
                "{}\n  project: {}\n  expected: {}\n  got:      {got}",
                case["name"], case["project"], case["expect"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} project cases differ from the TypeScript package:\n{}",
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
