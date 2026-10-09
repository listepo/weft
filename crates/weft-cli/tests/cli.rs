//! End-to-end runs of the real `weft` binary against a scratch directory. `validate` and `fmt`
//! make the same claims as packages/core/test/cli.test.ts; `explain` exists only in Rust.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURE: &str = include_str!("../../weft-core/tests/fixtures/differential.json");
const MESSY: &str = r#"<screen   weft="0.1" id="s"><!-- note --><button on-press="go" id="b">  Go  </button></screen>"#;
const CANONICAL: &str =
    "<screen id=\"s\" weft=\"0.1\">\n  <button id=\"b\" on-press=\"go\">Go</button>\n</screen>\n";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        std::fs::write(
            dir.join("catalog.json"),
            fixture["catalogs"]["test"].to_string(),
        )
        .unwrap();
        Scratch(dir)
    }

    fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    fn catalog(&self) -> PathBuf {
        self.0.join("catalog.json")
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

fn shown(p: &Path) -> String {
    p.display().to_string()
}

#[test]
fn validate_prints_one_line_per_diagnostic_and_exits_1_on_errors() {
    let s = Scratch::new("bad");
    let path = s.file("bad.weft", "<screen id=\"s\" weft=\"0.1\">\n  <button id=\"b\" variant=\"primay\">x</button>\n</screen>");
    let r = run(&[&"validate", &path, &"--catalog", &s.catalog()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stdout,
        format!(
            "{}:2:18 W203 \"primay\" is not an allowed value. — did you mean \"primary\"?\n",
            shown(&path)
        )
    );
}

#[test]
fn validate_strict_turns_unknown_content_into_errors() {
    let s = Scratch::new("strict");
    let path = s.file(
        "unknown.weft",
        "<screen id=\"s\" weft=\"0.1\"><fancy id=\"f\"/></screen>",
    );
    assert_eq!(
        run(&[&"validate", &path, &"--catalog", &s.catalog()]).code,
        0
    );
    assert_eq!(
        run(&[&"validate", &path, &"--catalog", &s.catalog(), &"--strict"]).code,
        1
    );
}

#[test]
fn validate_accepts_canonical_json_and_reports_paths_for_it() {
    let s = Scratch::new("json");
    let json =
        r#"{"weft": "0.1", "root": {"kind": "screen", "id": "s", "children": [{"kind": "text"}]}}"#;
    let path = s.file("doc.weft.json", json);
    let r = run(&[&"validate", &path, &"--catalog", &s.catalog()]);
    assert_eq!(r.code, 1);
    assert!(
        r.stdout.contains("doc.weft.json:/screen#s/text[0] W202 "),
        "{}",
        r.stdout
    );
}

#[test]
fn validate_without_a_catalog_checks_syntax_only() {
    let s = Scratch::new("syntax");
    let path = s.file("syntax.weft", "<screen id='s'/>");
    let r = run(&[&"validate", &path]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains(":1:12 W106 "), "{}", r.stdout);
    assert!(r.stderr.contains("no --catalog"), "{}", r.stderr);
}

#[test]
fn fmt_prints_canonical_markup_and_write_rewrites_the_file() {
    let s = Scratch::new("fmt");
    let path = s.file("messy.weft", MESSY);
    let printed = run(&[&"fmt", &path]);
    assert_eq!(printed.code, 0);
    assert_eq!(printed.stdout, CANONICAL);
    assert_eq!(run(&[&"fmt", &path, &"--write"]).code, 0);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CANONICAL);
}

#[test]
fn fmt_refuses_markup_with_syntax_errors() {
    let s = Scratch::new("broken");
    let path = s.file("broken.weft", "<screen id=s>");
    let r = run(&[&"fmt", &path]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains("W106"), "{}", r.stdout);
}

#[test]
fn usage_errors_exit_2() {
    let s = Scratch::new("usage");
    assert_eq!(run(&[]).code, 2);
    assert_eq!(run(&[&"lint", &"x"]).code, 2);
    assert_eq!(run(&[&"validate", &s.0.join("missing.weft")]).code, 2);
    let ok = s.file("ok.weft", CANONICAL);
    let bad = s.file("bad.json", "{}");
    assert_eq!(run(&[&"validate", &ok, &"--catalog", &bad]).code, 2);
    assert_eq!(run(&[&"fmt", &"x", &"--bogus"]).code, 2);
}

fn login(disabled: &str) -> String {
    format!(
        "<screen id=\"login\" weft=\"0.1\">\n  <form id=\"f\" on-submit=\"auth.submit\">\n    \
         <field id=\"email\" label=\"Email\" value=\"{{$.email}}\"/>\n    \
         <button id=\"go\" disabled=\"{disabled}\" submit=\"true\">Sign in</button>\n  \
         </form>\n</screen>\n"
    )
}

#[test]
fn explain_prints_one_readback_per_line() {
    let s = Scratch::new("explain");
    let path = s.file("login.weft", &login("{!$.email}"));
    let r = run(&[&"explain", &path, &"--catalog", &s.catalog()]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert_eq!(
        r.stdout,
        "form#f on-submit: runs action auth.submit\n\
         field#email value: reads and writes $.email\n\
         button#go disabled: true while $.email is falsy (NOT $.email)\n"
    );
}

#[test]
fn explain_against_an_older_version_shows_a_kept_negation() {
    // The benchmark's login.e2: "disable Sign in while $.busy is true" answered with {!$.busy}.
    let s = Scratch::new("against");
    let old = s.file("old.weft", &login("{!$.email}"));
    let inverted = s.file("inverted.weft", &login("{!$.busy}"));
    let intended = s.file("intended.weft", &login("{$.busy}"));
    let r = run(&[
        &"explain",
        &inverted,
        &"--against",
        &old,
        &"--catalog",
        &s.catalog(),
    ]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.stdout,
        "button#go disabled changed: was true while $.email is falsy (NOT $.email); \
         now true while $.busy is falsy (NOT $.busy)\n"
    );
    let r = run(&[
        &"explain",
        &intended,
        &"--against",
        &old,
        &"--catalog",
        &s.catalog(),
    ]);
    assert_eq!(
        r.stdout,
        "button#go disabled changed: was true while $.email is falsy (NOT $.email); \
         now true while $.busy is truthy\n"
    );
}

#[test]
fn explain_against_an_identical_version_prints_nothing_and_says_so() {
    let s = Scratch::new("same");
    let path = s.file("login.weft", &login("{!$.email}"));
    let r = run(&[
        &"explain",
        &path,
        &"--against",
        &path,
        &"--catalog",
        &s.catalog(),
    ]);
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, "");
    assert!(
        r.stderr
            .contains("no prop, event, loop or context entry changed"),
        "{}",
        r.stderr
    );
}

#[test]
fn explain_prints_diagnostics_and_exits_1_when_either_version_has_errors() {
    let s = Scratch::new("explain-bad");
    let good = s.file("good.weft", &login("{!$.email}"));
    let bad = s.file(
        "bad.weft",
        &login("{!$.email}").replace("submit=\"true\"", "submit=\"yes\""),
    );
    let broken = s.file("broken.weft", "<screen id=s>");
    let r = run(&[&"explain", &bad, &"--catalog", &s.catalog()]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains("bad.weft:4:"), "{}", r.stdout);
    let r = run(&[&"explain", &good, &"--against", &broken]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.contains("broken.weft:1:12 W106 "), "{}", r.stdout);
}

#[test]
fn explain_usage_and_io_failures_exit_2() {
    let s = Scratch::new("explain-usage");
    let ok = s.file("ok.weft", CANONICAL);
    let json = s.file("doc.weft.json", "{}");
    assert_eq!(run(&[&"explain"]).code, 2);
    assert_eq!(run(&[&"explain", &s.0.join("missing.weft")]).code, 2);
    assert_eq!(
        run(&[&"explain", &ok, &"--against", &s.0.join("missing.weft")]).code,
        2
    );
    let r = run(&[&"explain", &json]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("not canonical JSON"), "{}", r.stderr);
}
