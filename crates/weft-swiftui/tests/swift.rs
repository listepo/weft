//! Every corpus screen, catalog example and example project screen generates, without and with its
//! sample data, with the tokens in the screen file and in the shared `WeftTokens`, and the
//! generated Swift typechecks with the installed Xcode for iOS 17 and macOS 14. Without `xcrun` (Linux, CI without Xcode) the
//! typecheck is skipped with a message; generation is still checked.
//!
//! `xcrun swiftc` is slow (minutes per SDK), so the typecheck keeps a module cache and a stamp of
//! the toolchain-and-sources key under `~/Library/Caches/weft-swiftui/` (see `cache_root`): a warm
//! run of the same sources skips `swiftc` entirely. `.config/nextest.toml` terminates a run that
//! still overruns.

#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use indexmap::IndexMap;
use weft_catalog::{Appearance, Token, appearance};
use weft_core::Catalog;
use weft_swiftui::{GenerateOptions, TOKENS_FILE, generate, generate_tokens};

/// One Swift module: the screens in both token forms, and the shared tokens file.
fn module(
    screens: &[common::Screen],
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
    appearance: Option<Appearance<'_>>,
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
                appearance,
            };
            match generate(&document, &options) {
                Ok(swift) => out.push((name, swift)),
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
    }
    let (swift, problems) = generate_tokens(tokens, appearance);
    assert!(problems.is_empty(), "{problems:?}");
    out.push(("WeftTokens".to_owned(), swift));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    out
}

fn core_module() -> Vec<(String, String)> {
    module(
        &common::screens(),
        &common::catalog(),
        &common::tokens(),
        None,
    )
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
            appearance: None,
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
        appearance: None,
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

/// Where the Swift typecheck cache lives: the module cache `swiftc` fills, and a stamp of the
/// last toolchain-and-sources key that typechecked. `~/Library/Caches` (as `@weft/visual` uses)
/// so `cargo clean` does not drop a warm run; `CARGO_TARGET_TMPDIR` when home is not writable.
fn cache_root() -> PathBuf {
    let home =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Caches/weft-swiftui"));
    if let Some(dir) = home.filter(|d| std::fs::create_dir_all(d).is_ok()) {
        return dir;
    }
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("weft-swiftui-cache");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The key of one typecheck: the toolchain, the target and every source it will see. Two runs
/// with equal keys answer the same, so a stamp of a previous success can stand in for `swiftc`.
fn typecheck_key(sdk: &str, target: &str, sources: &[(String, String)]) -> Vec<u8> {
    fn push(key: &mut Vec<u8>, part: &[u8]) {
        key.extend_from_slice(&(part.len() as u64).to_le_bytes());
        key.extend_from_slice(part);
    }
    let mut key = Vec::new();
    push(&mut key, sdk.as_bytes());
    push(&mut key, target.as_bytes());
    // The compiler that will read the sources; a new Xcode must typecheck again.
    let version = Command::new("xcrun")
        .args(["swiftc", "--version"])
        .output()
        .unwrap();
    push(&mut key, &version.stdout);
    push(&mut key, &version.stderr);
    for (name, content) in sources {
        push(&mut key, name.as_bytes());
        push(&mut key, content.as_bytes());
    }
    key
}

fn swiftc(sdk: &str, target: &str, dir: &Path, files: &[PathBuf]) {
    let module_cache = cache_root().join("modules");
    std::fs::create_dir_all(&module_cache).unwrap();
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
        .arg("-module-cache-path")
        .arg(&module_cache)
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
    // Everything the two kinds of module below will typecheck, collected first so the stamp can
    // key on it: the core screens in one module, then each project as its own.
    let core = core_module();
    // Each project is its own module: its `WeftTokens` differ, and the app supplies the views of
    // its own kinds.
    let project_modules: Vec<_> = common::projects()
        .iter()
        .map(|project| {
            module(
                &project.screens,
                &project.catalog,
                &project.tokens,
                appearance(&project.modifiers),
            )
        })
        .collect();
    let mut sources: Vec<(String, String)> = core
        .iter()
        .map(|(name, swift)| (format!("{name}.swift"), swift.clone()))
        .collect();
    sources.push((
        "import/Settings.swift".to_owned(),
        std::fs::read_to_string(fixtures.join("import/Settings.swift")).unwrap(),
    ));
    for (i, project) in project_modules.iter().enumerate() {
        for (name, swift) in project {
            sources.push((format!("project-{i}/{name}.swift"), swift.clone()));
        }
        sources.push((
            format!("project-{i}/Views.swift"),
            std::fs::read_to_string(fixtures.join("project/Views.swift")).unwrap(),
        ));
    }
    let cache = cache_root();
    let stamp = cache.join(format!("typecheck-{sdk}.stamp"));
    let key = typecheck_key(sdk, target, &sources);
    if std::fs::read(&stamp).is_ok_and(|prev| prev == key) {
        eprintln!(
            "cached: the generated Swift for {target} already typechecked with this toolchain and these sources ({}); delete that file to typecheck again",
            stamp.display()
        );
        return;
    }
    let mut files = write(&dir, core);
    // The hand-written sample the import test reads must be real SwiftUI too.
    files.push(fixtures.join("import/Settings.swift"));
    swiftc(sdk, target, &dir, &files);
    for (i, project) in project_modules.iter().enumerate() {
        let dir = dir.join(format!("project-{i}"));
        let mut files = write(&dir, project.clone());
        files.push(fixtures.join("project/Views.swift"));
        swiftc(sdk, target, &dir, &files);
    }
    std::fs::write(&stamp, &key).unwrap();
}

#[test]
fn generated_swift_typechecks_for_ios_17() {
    typecheck("iphonesimulator", "arm64-apple-ios17.0-simulator");
}

#[test]
fn generated_swift_typechecks_for_macos_14() {
    typecheck("macosx", "arm64-apple-macos14.0");
}
