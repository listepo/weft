//! `weft validate` with a project file (SPEC §10): discovery, explicit arguments and what the
//! project adds to the checks.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

const EXAMPLE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/project");

struct Scratch(PathBuf);

impl Scratch {
    /// A copy of the example project, so a test can break it.
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("weft-project-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        copy(Path::new(EXAMPLE), &dir);
        Scratch(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
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

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
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

const BROKEN: &str = r#"<screen id="s" weft="0.1">
  <rating id="r" label="Score" value="{$.cart.totl}" color="{token.color.sun}"/>
  <button id="b" variant="ghost" disabled="{$.cart.total}" on-press="cart.delete">Go</button>
</screen>"#;

#[test]
fn every_example_screen_is_valid_in_strict_mode_with_its_project() {
    let screens = Path::new(EXAMPLE).join("screens");
    let mut seen = 0;
    for entry in std::fs::read_dir(screens).unwrap() {
        let path = entry.unwrap().path();
        let r = run(&[&"validate", &"--strict", &path]);
        assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", path.display());
        seen += 1;
    }
    assert!(seen >= 2);
}

#[test]
fn the_project_above_the_screen_supplies_catalog_tokens_actions_and_data() {
    let s = Scratch::new("found");
    let screen = s.write("screens/deep/broken.weft", BROKEN);
    let r = run(&[&"validate", &screen]);
    assert_eq!(r.code, 1);
    let shown = screen.display();
    assert_eq!(
        r.stdout,
        format!(
            "{shown}:2:54 W306 Token \"color.sun\" does not exist. — did you mean \"color.star\"?\n\
             {shown}:3:60 W308 Action \"cart.delete\" is not provided by the host.\n\
             {shown}:2:32 W315 The data schema does not declare \"totl\" in $.cart. — did you mean \"total\"?\n\
             {shown}:3:34 W316 The data at $.cart.total is string; this attribute takes boolean.\n"
        )
    );
}

#[test]
fn project_problems_point_into_the_project_file_and_fail_the_run() {
    let s = Scratch::new("problems");
    let project = s.write(
        "weft.json",
        r#"{"tokens": ["tokens/base.tokens.json", "../outside.json"], "catalog": "catalog.json", "extra": 1}"#,
    );
    let r = run(&[&"validate", &s.path("screens/cart.weft")]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    let shown = project.display();
    assert_eq!(
        r.stdout,
        format!(
            "{shown}:#/extra W702 Unknown member \"extra\" in the project file.\n\
             {shown}:#/tokens/1 W703 The file name \"../outside.json\" is absolute, leaves the project directory or is malformed.\n"
        )
    );
}

#[test]
fn explicit_arguments_win_over_the_project_found() {
    let s = Scratch::new("explicit");
    let screen = s.write("screens/broken.weft", BROKEN);
    // A project file elsewhere replaces the search.
    let other = s.write("other/weft.json", "{}");
    let r = run(&[&"validate", &screen, &"--project", &other]);
    assert!(r.stdout.contains("W401 "), "{}", r.stdout);
    assert!(!r.stdout.contains("W315"), "{}", r.stdout);
    // `--catalog` replaces only the catalog: `rating` is unknown again, the data check stays.
    let core = s.write(
        "core.json",
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/catalog/catalog.json"
        ))
        .unwrap(),
    );
    let r = run(&[&"validate", &screen, &"--catalog", &core]);
    assert!(r.stdout.contains("W401 "), "{}", r.stdout);
    assert!(r.stdout.contains("W315 "), "{}", r.stdout);
    // `--no-project` leaves only the syntax layer.
    let r = run(&[&"validate", &screen, &"--no-project"]);
    assert_eq!((r.code, r.stdout.as_str()), (0, ""));
    assert!(r.stderr.contains("no project"), "{}", r.stderr);
}

#[test]
fn an_unreadable_project_argument_is_a_usage_error() {
    let s = Scratch::new("missing");
    let r = run(&[
        &"validate",
        &s.path("screens/cart.weft"),
        &"--project",
        &s.path("nope/weft.json"),
    ]);
    assert_eq!(r.code, 2);
}

/// The example project file with `settings` added (SPEC §10.6).
fn with_settings(s: &Scratch, settings: serde_json::Value) -> PathBuf {
    let mut project: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(s.path("weft.json")).unwrap()).unwrap();
    for (key, value) in settings.as_object().unwrap() {
        project[key] = value.clone();
    }
    s.write("weft.json", &project.to_string())
}

#[test]
fn the_project_sets_the_mode_and_a_flag_overrides_it() {
    let s = Scratch::new("mode");
    with_settings(&s, serde_json::json!({ "validate": { "mode": "strict" } }));
    let screen = s.write(
        "screens/extra.weft",
        r#"<screen id="s" weft="0.1"><button id="b" bogus="1">Go</button></screen>"#,
    );
    let strict = run(&[&"validate", &screen]);
    assert_eq!(strict.code, 1, "{}", strict.stdout);
    assert!(strict.stdout.contains("W402 "), "{}", strict.stdout);
    let lenient = run(&[&"validate", &screen, &"--lenient"]);
    assert_eq!(lenient.code, 0, "{}", lenient.stdout);
    assert!(lenient.stdout.contains("W402 "), "{}", lenient.stdout);
    let none = run(&[&"validate", &screen, &"--no-project", &"--strict"]);
    assert_eq!(none.code, 0, "the syntax layer alone has no W402");
}

#[test]
fn the_project_makes_fmt_write_and_print_overrides_it() {
    let s = Scratch::new("fmt");
    with_settings(&s, serde_json::json!({ "format": { "write": true } }));
    let messy = "<screen weft=\"0.1\"   id=\"s\"><text id=\"t\">Hi</text></screen>";
    let screen = s.write("screens/messy.weft", messy);
    let printed = run(&[&"fmt", &screen, &"--print"]);
    assert_eq!(printed.code, 0, "{}", printed.stderr);
    assert!(printed.stdout.starts_with("<screen id=\"s\" weft=\"0.1\">"));
    assert_eq!(std::fs::read_to_string(&screen).unwrap(), messy);
    let written = run(&[&"fmt", &screen]);
    assert_eq!((written.code, written.stdout.as_str()), (0, ""));
    assert_eq!(std::fs::read_to_string(&screen).unwrap(), printed.stdout);
}

#[test]
fn explain_reads_the_catalog_of_the_project() {
    let s = Scratch::new("explain");
    let screen = s.path("screens/review.weft");
    let with = run(&[&"explain", &screen]);
    assert_eq!(with.code, 0, "{}{}", with.stdout, with.stderr);
    assert!(!with.stderr.contains("no project"), "{}", with.stderr);
    let without = run(&[&"explain", &screen, &"--no-project"]);
    assert!(without.stderr.contains("no project"), "{}", without.stderr);
}

#[test]
fn bad_settings_are_reported_and_the_defaults_apply() {
    let s = Scratch::new("settings");
    let project = with_settings(
        &s,
        serde_json::json!({ "validate": { "mode": "loose" }, "render": { "outdir": "x" } }),
    );
    let r = run(&[&"validate", &s.path("screens/cart.weft")]);
    let shown = project.display();
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(
        r.stdout.contains(&format!("{shown}:#/validate/mode W701 ")),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains(&format!("{shown}:#/render/outdir W702 ")),
        "{}",
        r.stdout
    );
}
