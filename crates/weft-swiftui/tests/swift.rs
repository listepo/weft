//! Every corpus screen and catalog example generates, without and with its sample data, and the
//! generated Swift typechecks with the installed Xcode for iOS 17 and macOS 14. Without `xcrun`
//! (Linux, CI without Xcode) the typecheck is skipped with a message; generation is still checked.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use weft_swiftui::{GenerateOptions, generate};

fn generate_all() -> Vec<(String, String)> {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut failures = vec![];
    let mut out = vec![];
    for screen in common::screens() {
        let document = common::parse_screen(&screen.markup, &catalog, &tokens);
        let mut variants = vec![(screen.name.clone(), None)];
        if let Some(data) = &screen.data {
            variants.push((format!("{}-sample", screen.name), Some(data)));
        }
        for (name, data) in variants {
            let type_name = common::type_name(&name);
            let options = GenerateOptions {
                catalog: &catalog,
                tokens: &tokens,
                name: Some(&type_name),
                data,
            };
            match generate(&document, &options) {
                Ok(swift) => out.push((name, swift)),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    out
}

#[test]
fn refused_screens_are_refused_with_the_reason() {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let refused: Vec<_> = common::all_screens()
        .into_iter()
        .filter(|s| common::REFUSED.contains(&s.name.as_str()))
        .collect();
    assert_eq!(refused.len(), common::REFUSED.len());
    for screen in refused {
        let document = common::parse_screen(&screen.markup, &catalog, &tokens);
        let options = GenerateOptions {
            catalog: &catalog,
            tokens: &tokens,
            name: None,
            data: screen.data.as_ref(),
        };
        let error = generate(&document, &options).unwrap_err().to_string();
        assert!(error.contains("array index"), "{}: {error}", screen.name);
    }
}

#[test]
fn every_screen_generates_deterministically() {
    let first = generate_all();
    let second = generate_all();
    assert_eq!(first, second);
}

fn write_all(dir: &Path) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).unwrap();
    generate_all()
        .into_iter()
        .map(|(name, swift)| {
            let path = dir.join(format!("{}.swift", common::type_name(&name)));
            std::fs::write(&path, swift).unwrap();
            path
        })
        .collect()
}

fn xcrun_available() -> bool {
    Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn typecheck(sdk: &str, target: &str) {
    if !xcrun_available() {
        eprintln!(
            "skipped: `xcrun swiftc` is not available, so the generated Swift is not typechecked for {target}"
        );
        return;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("swiftui-{sdk}"));
    let files = write_all(&dir);
    let output = Command::new("xcrun")
        .args([
            "--sdk",
            sdk,
            "swiftc",
            "-typecheck",
            "-swift-version",
            "6",
            "-target",
            target,
        ])
        .args(&files)
        // The hand-written sample the import test reads must be real SwiftUI too.
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/import/Settings.swift"
        ))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "swiftc rejected the generated Swift in {}:\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn generated_swift_typechecks_for_ios_17() {
    typecheck("iphonesimulator", "arm64-apple-ios17.0-simulator");
}

#[test]
fn generated_swift_typechecks_for_macos_14() {
    typecheck("macosx", "arm64-apple-macos14.0");
}
