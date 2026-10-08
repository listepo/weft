//! The crate graph, checked with `cargo metadata`: the core stays a pure library that builds for
//! WebAssembly and native addons, and only the binary owns the CLI dependencies.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::process::Command;

use serde_json::Value as Json;

/// The normal (not dev or build) dependencies of `package`, as `cargo metadata` lists them.
fn entries(package: &str) -> Vec<Json> {
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
        .cloned()
        .collect()
}

fn dependencies(package: &str) -> Vec<String> {
    entries(package)
        .iter()
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
        [
            "indexmap",
            "semver",
            "serde",
            "serde_json",
            "thiserror",
            "weft-core"
        ]
    );
}

#[test]
fn the_binding_layer_depends_only_on_pure_crates() {
    let mut deps = dependencies("weft-binding");
    deps.sort();
    assert_eq!(
        deps,
        [
            "indexmap",
            "serde",
            "serde_json",
            "thiserror",
            "weft-catalog",
            "weft-core",
            // Optional: only the `web` feature carries the web importers.
            "weft-import",
            "weft-web"
        ]
    );
}

#[test]
fn only_the_wasm_bindings_add_wasm_bindgen_to_the_binding_layer() {
    let mut deps = dependencies("weft-wasm");
    deps.sort();
    assert_eq!(deps, ["wasm-bindgen", "weft-binding", "weft-core"]);
}

#[test]
fn only_the_native_addon_adds_napi_to_the_binding_layer() {
    let mut deps = dependencies("weft-node");
    deps.sort();
    assert_eq!(deps, ["napi", "napi-derive", "weft-binding", "weft-core"]);
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
            "indexmap",
            "oxc_allocator",
            "oxc_ast",
            "oxc_parser",
            "oxc_span",
            "serde",
            "serde_json",
            "unicode-properties",
            "weft-catalog",
            "weft-core",
            "weft-import"
        ]
    );
}

#[test]
fn the_swiftui_generator_builds_without_the_importers_c_parser() {
    let mut deps = dependencies("weft-swiftui");
    deps.sort();
    assert_eq!(
        deps,
        [
            "indexmap",
            "serde",
            "serde_json",
            "tree-sitter",
            "tree-sitter-swift",
            "weft-catalog",
            "weft-core",
            "weft-import"
        ]
    );
    // tree-sitter is C, which does not build for wasm32-unknown-unknown: only the `import`
    // feature may pull it in, so the generator stays a pure crate. The shared importer kit
    // comes with the importer.
    let mut optional: Vec<String> = entries("weft-swiftui")
        .iter()
        .filter(|d| d["optional"] == true)
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    optional.sort();
    assert_eq!(
        optional,
        ["tree-sitter", "tree-sitter-swift", "weft-import"]
    );
}

#[test]
fn the_slint_target_compiles_slint_only_in_its_tests() {
    let mut deps: Vec<String> = entries("weft-slint")
        .iter()
        .filter(|d| d["optional"] != true)
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    deps.sort();
    // The generator and the comment-path importer are pure Rust over the core. The Slint
    // compiler (`slint-interpreter`) is a dev-dependency that checks the generated files.
    // The syntax tree (`i-slint-compiler`) is only the optional `import` feature.
    assert_eq!(deps, ["indexmap", "thiserror", "weft-catalog", "weft-core"]);
    let mut optional: Vec<String> = entries("weft-slint")
        .iter()
        .filter(|d| d["optional"] == true)
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect();
    optional.sort();
    assert_eq!(optional, ["i-slint-compiler", "weft-import"]);
}
