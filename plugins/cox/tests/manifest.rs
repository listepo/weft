//! `plugin.toml` read into cox's own manifest type, the type cox's `docs/plugin.schema.json` is
//! generated from: unknown keys are denied at every level and `validate` runs cox's own checks,
//! so the manifest is checked by the pinned cox revision without vendoring the schema or reading
//! outside this repository.

// Helpers of a test file fail the test the same way a #[test] body does.
#![allow(clippy::unwrap_used, clippy::panic)]

use cox_plugin_sdk::{Capabilities, DesktopCaps, PluginManifest};
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
fn it_asks_for_nothing_but_tools_and_the_canvas() {
    let expected = Capabilities {
        tools: tool_names(),
        desktop: DesktopCaps {
            canvas: true,
            ..DesktopCaps::default()
        },
        ..Capabilities::default()
    };
    assert_eq!(manifest().capabilities, expected);
}

/// cox loads a skill only when its folder holds a `SKILL.md` whose `name` is the folder's and the
/// declared one, so a renamed folder or file would drop the guide with nothing but a notice.
#[test]
fn every_declared_skill_folder_holds_its_skill() {
    let manifest = manifest();
    let names: Vec<&str> = manifest.skills.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["weft-spec"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for skill in &manifest.skills {
        let dir = root.join(&skill.dir);
        let text = std::fs::read_to_string(dir.join("SKILL.md")).unwrap();
        let header = text.split("---").nth(1).unwrap();
        assert!(
            header.lines().any(|l| l == format!("name: {}", skill.name)),
            "{header}"
        );
        assert!(dir.join("AGENT-SPEC.md").is_file());
    }
}
