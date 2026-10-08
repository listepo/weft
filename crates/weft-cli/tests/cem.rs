//! `weft import-cem` end to end (SPEC §9, "From a Custom Elements Manifest", and §10.6): the
//! catalog it prints is one `weft validate --catalog` checks documents against, the project's
//! `import.cem` settings stand in for missing flags, and an oversized manifest is not read.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

use weft_import::MAX_MANIFEST_LENGTH;

const MANIFEST: &str = include_str!("../../weft-import/tests/fixtures/cem/acme-ui.json");

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-cem-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&dyn AsRef<std::ffi::OsStr>]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(args.iter().map(|a| a.as_ref()))
        .output()
        .unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn catalog(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn an_imported_catalog_is_one_validate_accepts() {
    let s = Scratch::new("validate");
    let manifest = s.write("custom-elements.json", MANIFEST);
    let r = run(&[
        &"import-cem",
        &manifest,
        &"--no-project",
        &"--name",
        &"acme-ui",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stderr.contains(" loss props: "), "{}", r.stderr);
    let imported = s.write("acme-ui.json", &r.stdout);
    let json = catalog(&imported);
    assert_eq!(
        (&json["name"], &json["version"]),
        (&"acme-ui".into(), &"0.0.0".into())
    );

    let card = s.write(
        "card.weft",
        "<acme-card id=\"card\" elevated=\"true\" weft=\"0.1\">\n  <acme-button id=\"buy\" variant=\"primary\" on-acme-click=\"cart.add\">Buy</acme-button>\n  <acme-rating id=\"stars\" value=\"4\"/>\n</acme-card>\n",
    );
    let r = run(&[
        &"validate",
        &card,
        &"--strict",
        &"--no-project",
        &"--catalog",
        &imported,
    ]);
    assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);

    let misuse = s.write(
        "misuse.weft",
        "<acme-card id=\"card\" weft=\"0.1\"><acme-button id=\"buy\" variant=\"primay\">Buy</acme-button></acme-card>",
    );
    let r = run(&[
        &"validate",
        &misuse,
        &"--no-project",
        &"--catalog",
        &imported,
    ]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains(" W203 "), "{}", r.stdout);
}

#[test]
fn project_settings_stand_in_for_missing_flags() {
    let s = Scratch::new("project");
    s.write(
        "weft.json",
        r#"{ "import": { "cem": { "outDir": "catalogs", "name": "acme", "version": "3.0.0" } } }"#,
    );
    let manifest = s.write("vendor/custom-elements.json", MANIFEST);
    let r = run(&[&"import-cem", &manifest]);
    assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);
    let written = s.0.join("catalogs/custom-elements.catalog.json");
    let json = catalog(&written);
    assert_eq!(
        (&json["name"], &json["version"]),
        (&"acme".into(), &"3.0.0".into())
    );

    let r = run(&[&"import-cem", &manifest]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("--force"), "{}", r.stderr);

    let r = run(&[&"import-cem", &manifest, &"--force", &"--version", &"3.1.0"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(catalog(&written)["version"], "3.1.0");

    // A project whose catalog already has the kinds: they are left out, as an extension may only
    // widen its base.
    s.write(
        "weft.json",
        r#"{ "catalog": "catalogs/custom-elements.catalog.json" }"#,
    );
    let r = run(&[&"import-cem", &manifest]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let again: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(again["components"], serde_json::json!({}));
    assert_eq!(again["name"], "custom-elements");
}

#[test]
fn a_manifest_over_the_limit_is_not_read() {
    let s = Scratch::new("limit");
    let big = s.write("big.json", &" ".repeat(MAX_MANIFEST_LENGTH + 1));
    let r = run(&[&"import-cem", &big, &"--no-project"]);
    assert_eq!((r.code, r.stdout.as_str()), (2, ""));
    assert!(
        r.stderr.contains("longer than 10000000 bytes"),
        "{}",
        r.stderr
    );
}

#[test]
fn a_manifest_that_is_not_json_is_w601() {
    let s = Scratch::new("broken");
    let broken = s.write("broken.json", "{ not json");
    let r = run(&[&"import-cem", &broken, &"--no-project"]);
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains(" W601 "), "{}", r.stderr);
    assert_eq!(
        catalog(&s.write("out.json", &r.stdout))["components"],
        serde_json::json!({})
    );
}
