//! The crate graph, checked with `cargo metadata`: the core stays a pure library that builds for
//! WebAssembly and native addons, and only the binary owns the CLI dependencies.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::process::Command;

use serde_json::Value as Json;

fn dependencies(package: &str) -> Vec<String> {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    let metadata: Json = serde_json::from_slice(&out.stdout).unwrap();
    let packages = metadata["packages"].as_array().unwrap();
    let found = packages.iter().find(|p| p["name"] == package).unwrap();
    found["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["kind"].is_null())
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn core_depends_only_on_pure_crates() {
    let mut deps = dependencies("weft-core");
    deps.sort();
    assert_eq!(
        deps,
        ["indexmap", "ryu-js", "serde", "serde_json", "thiserror"]
    );
}

#[test]
fn the_catalog_depends_only_on_the_core_and_pure_crates() {
    let mut deps = dependencies("weft-catalog");
    deps.sort();
    assert_eq!(
        deps,
        ["indexmap", "serde", "serde_json", "thiserror", "weft-core"]
    );
}

#[test]
fn only_the_wasm_bindings_add_wasm_bindgen_to_the_pure_crates() {
    let mut deps = dependencies("weft-wasm");
    deps.sort();
    assert_eq!(
        deps,
        [
            "indexmap",
            "serde",
            "serde_json",
            "thiserror",
            "wasm-bindgen",
            "weft-catalog",
            "weft-core",
            // Optional: only the `web` build of the module carries the web importers.
            "weft-import",
            "weft-web"
        ]
    );
}

#[test]
fn only_the_cli_depends_on_clap_and_anyhow() {
    let cli = dependencies("weft-cli");
    assert!(cli.iter().any(|d| d == "clap") && cli.iter().any(|d| d == "anyhow"));
    let core = dependencies("weft-core");
    assert!(
        !core.iter().any(|d| d == "clap" || d == "anyhow"),
        "{core:?}"
    );
}

#[test]
fn the_importer_kit_depends_only_on_the_core_and_pure_crates() {
    let mut deps = dependencies("weft-import");
    deps.sort();
    assert_eq!(
        deps,
        [
            "indexmap",
            "serde",
            "serde_json",
            "unicode-normalization",
            "weft-core"
        ]
    );
}

#[test]
fn the_web_crate_adds_only_its_parsers() {
    let mut deps = dependencies("weft-web");
    deps.sort();
    assert_eq!(
        deps,
        [
            "html5ever",
            "serde",
            "serde_json",
            "weft-core",
            "weft-import"
        ]
    );
}
