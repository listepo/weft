//! `schemas/weft.schema.json` is generated from the loader's own table of settings, so editors
//! and the loader agree on every key. `WEFT_UPDATE_FIXTURES=1` rewrites the file.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::Value as Json;
use weft_catalog::{ProjectOptions, load_project, project_file_schema};

fn schema_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schemas/weft.schema.json")
}

#[test]
fn the_published_schema_is_the_generated_one() {
    let generated = format!(
        "{}\n",
        serde_json::to_string_pretty(&project_file_schema()).unwrap()
    );
    let file = schema_file();
    if std::env::var_os("WEFT_UPDATE_FIXTURES").is_some() {
        std::fs::write(&file, &generated).unwrap();
    }
    let published = std::fs::read_to_string(&file).unwrap_or_default();
    assert!(
        published == generated,
        "schemas/weft.schema.json is stale; regenerate with WEFT_UPDATE_FIXTURES=1"
    );
}

/// One value for every setting the schema describes, built from the schema itself.
fn example(schema: &Json, defs: &Json) -> Json {
    if let Some(reference) = schema.get("$ref").and_then(Json::as_str) {
        let name = reference.trim_start_matches("#/$defs/");
        return example(&defs[name], defs);
    }
    if let Some(values) = schema.get("enum") {
        return values[0].clone();
    }
    match schema.get("type").and_then(Json::as_str) {
        Some("object") => match schema.get("properties").and_then(Json::as_object) {
            Some(properties) => properties
                .iter()
                .map(|(k, v)| (k.clone(), example(v, defs)))
                .collect::<serde_json::Map<_, _>>()
                .into(),
            None => serde_json::json!({ "some-plugin": { "any": true } }),
        },
        Some("array") => Json::Array(vec![example(&schema["items"], defs)]),
        Some("boolean") => Json::Bool(true),
        Some("integer") => Json::from(7),
        Some("string") if schema.get("pattern").is_some() && schema.get("minLength").is_none() => {
            Json::from("cart.add")
        }
        Some("string") => Json::from("dir/file.json"),
        other => panic!("the example builder does not know {other:?}"),
    }
}

#[test]
fn everything_the_schema_allows_loads_without_a_diagnostic() {
    let schema = project_file_schema();
    let mut file = example(&schema, &schema["$defs"]);
    // The resources name files; this test is about keys, so content mode keeps them out.
    for resource in ["tokens", "catalog", "data"] {
        file.as_object_mut().unwrap().remove(resource);
    }
    let read = |_: &str| None;
    let loaded = load_project(
        &file,
        &ProjectOptions {
            read: Some(&read),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(loaded.diagnostics, vec![], "{file}");
    for section in [
        "validate", "format", "render", "export", "import", "mcp", "plugins",
    ] {
        assert_eq!(
            loaded.project.settings.get(section),
            file.get(section),
            "{section}"
        );
    }
}
