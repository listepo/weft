//! `plugin.toml` read into cox's own manifest type, the type cox's `docs/plugin.schema.json` is
//! generated from: unknown keys are denied at every level and `validate` runs cox's own checks,
//! so the manifest is checked by the pinned cox revision without vendoring the schema or reading
//! outside this repository.

// Helpers of a test file fail the test the same way a #[test] body does.
#![allow(clippy::unwrap_used, clippy::panic)]

use cox_plugin_sdk::{Capabilities, PluginManifest};
use weft_cox::{Weft, tool_names};

fn manifest() -> PluginManifest {
    let text = include_str!("../plugin.toml");
    toml::from_str(text).unwrap()
}

#[test]
fn the_manifest_is_valid_for_cox() {
    let manifest = manifest();
    manifest.validate().unwrap();
    assert_eq!(manifest.id, "weft");
    assert_eq!(manifest.wasm.as_deref(), Some("weft.wasm"));
}

#[test]
fn it_grants_exactly_the_tools_the_plugin_declares() {
    let (weft, _) = Weft::from_config(&serde_json::json!({})).unwrap();
    let declared: Vec<String> = weft
        .specs()
        .iter()
        .filter_map(|spec| spec["name"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(manifest().capabilities.tools, declared);
    assert_eq!(declared, tool_names());
}

#[test]
fn it_asks_for_nothing_but_tools() {
    let expected = Capabilities {
        tools: tool_names(),
        ..Capabilities::default()
    };
    assert_eq!(manifest().capabilities, expected);
}
