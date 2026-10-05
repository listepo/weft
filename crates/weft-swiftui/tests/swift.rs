//! Every corpus screen, catalog example and example project screen generates, without and with its
//! sample data, with the tokens in the screen file and in the shared `WeftTokens`, and the
//! generated Swift typechecks with the installed Xcode for iOS 17 and macOS 14. Without `xcrun` (Linux, CI without Xcode) the
//! typecheck is skipped with a message; generation is still checked.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::Catalog;
use weft_swiftui::{GenerateOptions, TOKENS_FILE, generate, generate_tokens};

/// One Swift module: the screens in both token forms, and the shared tokens file.
fn module(
    screens: &[common::Screen],
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
) -> Vec<(String, String)> {
    let mut failures = vec![];
    let mut out = vec![];
    for screen in screens {
        let document = common::parse_screen(&screen.markup, catalog, tokens);
        // Each form of a screen gets its own type names, so all fit in one module. The shared
        // tokens are checked once, with the data when there is some, rather than in every
        // combination: the two features touch different parts of the file.
        let data = screen.data.as_ref();
        let mut variants = vec![("", false, None)];
        if data.is_some() {
            variants.push(("-sample", false, data));
        }
        variants.push(("-shared", true, data));
        for (suffix, shared_tokens, data) in variants {
            let name = common::type_name(&screen.name) + suffix;
            let options = GenerateOptions {
                catalog,
                tokens,
                name: Some(&name),
                shared_tokens,
                data,
            };
            match generate(&document, &options) {
                Ok(swift) => out.push((name, swift)),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
    }
    let (swift, problems) = generate_tokens(tokens);
    assert!(problems.is_empty(), "{problems:?}");
    out.push(("WeftTokens".to_owned(), swift));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    out
}

fn core_module() -> Vec<(String, String)> {
    module(&common::screens(), &common::catalog(), &common::tokens())
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
            shared_tokens: false,
            data: screen.data.as_ref(),
        };
        let error = generate(&document, &options).unwrap_err().to_string();
        assert!(error.contains("array index"), "{}: {error}", screen.name);
    }
}

#[test]
fn every_screen_generates_deterministically() {
    assert_eq!(core_module(), core_module());
}

#[test]
fn a_custom_kind_names_the_view_the_app_writes() {
    let project = &common::projects()[1];
    let document = common::parse_screen(
        &project.screens[0].markup,
        &project.catalog,
        &project.tokens,
    );
    let options = GenerateOptions {
        catalog: &project.catalog,
        tokens: &project.tokens,
        name: None,
        shared_tokens: true,
        data: None,
    };
    let swift = generate(&document, &options).unwrap();
    assert!(swift.contains(
        "// The app writes the views for its own kinds: `RatingView` (`rating`), `PromoCardView` (`promo-card`)."
    ));
    assert!(swift.contains("var theme = WeftTokens()"));
    assert!(!swift.contains("struct OffersTheme"));
}

fn write(dir: &Path, files: Vec<(String, String)>) -> Vec<PathBuf> {
    std::fs::create_dir_all(dir).unwrap();
    files
        .into_iter()
        .map(|(name, swift)| {
            let file = if name == "WeftTokens" {
                TOKENS_FILE.to_owned()
            } else {
                format!("{name}.swift")
            };
            let path = dir.join(file);
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

fn swiftc(sdk: &str, target: &str, dir: &Path, files: &[PathBuf]) {
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
        .args(files)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "swiftc rejected the generated Swift in {}:\n{}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn typecheck(sdk: &str, target: &str) {
    if !xcrun_available() {
        eprintln!(
            "skipped: `xcrun swiftc` is not available, so the generated Swift is not typechecked for {target}"
        );
        return;
    }
    let fixtures = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures"));
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("swiftui-{sdk}"));
    let mut files = write(&dir, core_module());
    // The hand-written sample the import test reads must be real SwiftUI too.
    files.push(fixtures.join("import/Settings.swift"));
    swiftc(sdk, target, &dir, &files);
    // Each project is its own module: its `WeftTokens` differ, and the app supplies the views of
    // its own kinds.
    for (i, project) in common::projects().iter().enumerate() {
        let dir = dir.join(format!("project-{i}"));
        let mut files = write(
            &dir,
            module(&project.screens, &project.catalog, &project.tokens),
        );
        files.push(fixtures.join("project/Views.swift"));
        swiftc(sdk, target, &dir, &files);
    }
}

#[test]
fn generated_swift_typechecks_for_ios_17() {
    typecheck("iphonesimulator", "arm64-apple-ios17.0-simulator");
}

#[test]
fn generated_swift_typechecks_for_macos_14() {
    typecheck("macosx", "arm64-apple-macos14.0");
}
