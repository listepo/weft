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
