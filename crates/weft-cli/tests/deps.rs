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
fn only_the_cli_depends_on_clap_and_anyhow() {
    let cli = dependencies("weft-cli");
    assert!(cli.iter().any(|d| d == "clap") && cli.iter().any(|d| d == "anyhow"));
    let core = dependencies("weft-core");
    assert!(
        !core.iter().any(|d| d == "clap" || d == "anyhow"),
        "{core:?}"
    );
}
