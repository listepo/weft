//! Every snapshot file under `tests/snapshots/` is one a test writes. An unreferenced file
//! (`cargo insta test --unreferenced=reject`) is dead weight in review and in the cache; this
//! test fails on one without needing cargo-insta, so `root:rust-test` rejects it in CI.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn snap_files(dir: &Path, root: &Path, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            snap_files(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "snap") {
            out.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

fn expected() -> BTreeSet<String> {
    let screens: Vec<String> = common::corpus()
        .into_iter()
        .chain(common::examples())
        .map(|s| s.name)
        .collect();
    let corpus: Vec<String> = common::corpus().into_iter().map(|s| s.name).collect();
    let mut out = BTreeSet::new();
    let mut add = |target: &str, name: &str| {
        out.insert(format!("{target}/{name}.snap"));
    };
    // snapshot_all in snapshots.rs
    for target in [
        "json",
        "html",
        "react-jsx",
        "react-tsx",
        "solid-jsx",
        "solid-tsx",
        "lit-js",
        "slint",
        "swiftui",
        // a2ui_export_snapshots in interop.rs
        "a2ui",
        "a2ui-losses",
    ] {
        for name in &screens {
            add(target, name);
        }
    }
    // snapshot_with_data in snapshots.rs
    for target in ["html-data", "swiftui-data"] {
        for name in &corpus {
            add(target, name);
        }
    }
    // json_render_export_snapshots in interop.rs (corpus only, plus the shared losses file)
    for name in &corpus {
        add("json-render", name);
    }
    add("json-render-losses", "all");
    // a2ui_corpus_renditions_import in interop.rs: only screens that carry screen.a2ui.json
    for screen in common::corpus() {
        if screen.path.with_file_name("screen.a2ui.json").is_file() {
            add("a2ui-import", &screen.name);
            add("a2ui-import-losses", &screen.name);
        }
    }
    // Fixed names of snapshots.rs and document_schema.rs
    for (target, name) in [
        ("swiftui-shared", "WeftTokens"),
        ("swiftui-shared", "corpus-login"),
        ("swiftui-shared", "project-WeftTokens"),
        ("swiftui-shared", "project-offers"),
        ("swiftui-appearance", "example-WeftTokens"),
        ("swiftui-appearance", "example-review"),
        ("html-appearance", "example-tokens-css"),
        ("html-appearance", "example-review"),
        ("document-schema", "weft-core"),
        ("document-schema", "project-shop"),
    ] {
        add(target, name);
    }
    out
}

#[test]
fn every_snapshot_is_referenced() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let mut actual = BTreeSet::new();
    snap_files(&root, &root, &mut actual);
    let expected = expected();
    let orphans: Vec<_> = actual.difference(&expected).collect();
    let missing: Vec<_> = expected.difference(&actual).collect();
    assert!(
        orphans.is_empty(),
        "unreferenced snapshots (delete them or teach a test to write them): {orphans:#?}"
    );
    assert!(
        missing.is_empty(),
        "snapshots no test writes (run the suite with INSTA_UPDATE=always and review): {missing:#?}"
    );
}
