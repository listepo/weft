//! `weft validate`, the generators (`weft swiftui`, `slint`, `html`, `react`, `solid`, `schema`) and the importers
//! with a project file (SPEC §10): discovery, explicit arguments and what the project adds to the
//! checks and the output.

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
    run_in(Path::new("."), args)
}

fn run_in(dir: &Path, args: &[&dyn AsRef<std::ffi::OsStr>]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_weft"))
        .args(args.iter().map(|a| a.as_ref()))
        .current_dir(dir)
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

#[cfg(unix)]
#[test]
fn a_member_file_that_is_a_symlink_out_of_the_project_is_not_read() {
    let s = Scratch::new("symlink-escape");
    let secret = std::env::temp_dir().join(format!("weft-secret-{}.json", std::process::id()));
    std::fs::write(
        &secret,
        r##"{"leaked":{"$type":"color","$value":"#ff0000"}}"##,
    )
    .unwrap();
    let link = s.path("outside.json");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&secret, &link).unwrap();
    s.write("weft.json", r#"{"tokens":"outside.json"}"#);
    let r = run(&[&"validate", &s.path("screens/cart.weft")]);
    let _ = std::fs::remove_file(&secret);
    assert!(r.stdout.contains("W704"), "{}", r.stdout);
    assert!(r.stdout.contains("#/tokens"), "{}", r.stdout);
}

#[cfg(unix)]
#[test]
fn a_catalog_list_entry_that_is_a_symlink_out_of_the_project_is_not_read() {
    let s = Scratch::new("symlink-catalog");
    let outside =
        std::env::temp_dir().join(format!("weft-outside-catalog-{}.json", std::process::id()));
    let link = s.path("catalogs/acme-ui.catalog.json");
    std::fs::rename(&link, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let r = run(&[&"validate", &s.path("screens/order.weft")]);
    let _ = std::fs::remove_file(&outside);
    assert!(r.stdout.contains("W704"), "{}", r.stdout);
    assert!(r.stdout.contains("#/catalog/0"), "{}", r.stdout);
}

#[test]
fn project_problems_point_into_the_project_file_and_fail_the_run() {
    let s = Scratch::new("problems");
    let project = s.write(
        "weft.json",
        r#"{"tokens": ["tokens/base.tokens.json", "../outside.json"], "catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"], "extra": 1}"#,
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
fn broken_catalog_lists_give_each_catalog_code() {
    let library = "catalogs/acme-ui.catalog.json";
    let shop = read_example("catalog.json");
    let chip = r#""acme-chip": { "description": "A chip.", "role": "status", "content": "text" },"#;
    // A case: scratch name, catalog list, one replaced file, expected lines.
    type Case<'a> = (&'a str, &'a str, Option<(&'a str, String)>, &'a [&'a str]);
    let cases: [Case; 4] = [
        (
            "w711",
            r#"["catalogs/acme-ui.catalog.json", "catalogs/acme-ui.catalog.json", "catalog.json"]"#,
            None,
            &["#/catalog/1/name W711"],
        ),
        (
            "w712",
            r#"["catalogs/acme-ui.catalog.json", "catalog.json"]"#,
            Some((
                library,
                read_example(library).replace(r#""prefix": "acme""#, r#""prefix": "date""#),
            )),
            &["#/catalog/0/prefix W712"],
        ),
        (
            "w713",
            r#"["catalogs/acme-ui.catalog.json", "catalog.json"]"#,
            Some((
                "catalog.json",
                shop.replacen(
                    r#""components": {"#,
                    &format!(r#""components": {{ {chip}"#),
                    1,
                ),
            )),
            &["#/catalog/1/components/acme-chip W713"],
        ),
        (
            "w714",
            r#"["catalog.json"]"#,
            None,
            &[
                "#/catalog/0/requires/acme-ui W714",
                "#/catalog/0/components/acme-button W706",
                " W401 ",
            ],
        ),
    ];
    for (name, list, file, expected) in cases {
        let s = Scratch::new(name);
        let project = read_example("weft.json")
            .replace(r#"["catalogs/acme-ui.catalog.json", "catalog.json"]"#, list);
        s.write("weft.json", &project);
        if let Some((path, content)) = file {
            s.write(path, &content);
        }
        let r = run(&[&"validate", &s.path("screens/order.weft")]);
        for code in expected {
            assert!(r.stdout.contains(code), "{name}: {code}\n{}", r.stdout);
        }
    }
}

fn read_example(name: &str) -> String {
    std::fs::read_to_string(Path::new(EXAMPLE).join(name)).unwrap()
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

const PLAIN: &str = r#"<screen id="plain" label="Plain" weft="0.1">
  <stack id="row" direction="row" gap="{token.space.sm}">
    <button id="back" on-press="nav.back">Back</button>
  </stack>
</screen>
"#;

/// The example project with its own `space.sm` and both SwiftUI output directories set.
fn swiftui_project(name: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write(
        "tokens/swift.tokens.json",
        r#"{ "space": { "$type": "dimension", "sm": { "$value": { "value": 12, "unit": "px" } } } }"#,
    );
    s.write(
        "weft.json",
        r#"{
  "tokens": ["tokens/base.tokens.json", "tokens/swift.tokens.json"],
  "catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"],
  "export": { "swiftui": { "outDir": "ios" } },
  "import": { "swiftui": { "outDir": "imported" } }
}"#,
    );
    s
}

#[test]
fn swiftui_writes_where_the_project_says_with_its_tokens_and_reads_back() {
    let s = swiftui_project("swiftui");
    let screen = s.write("screens/plain.weft", PLAIN);
    let r = run(&[&"swiftui", &screen]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let swift = std::fs::read_to_string(s.path("ios/plain.swift")).unwrap();
    // A project's screens read the one `WeftTokens` file.
    assert!(swift.contains("var theme = WeftTokens()"), "{swift}");
    assert!(!swift.contains("CGFloat"), "{swift}");

    // `swiftui-tokens` finds the project from the working directory.
    let r = run_in(&s.path("screens"), &[&"swiftui-tokens"]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let tokens = std::fs::read_to_string(s.path("ios/WeftTokens.swift")).unwrap();
    assert!(tokens.contains("struct WeftTokens: Sendable {"), "{tokens}");
    assert!(tokens.contains("var sm: CGFloat = 12"), "{tokens}");

    let r = run(&[&"import-swiftui", &s.path("ios/plain.swift")]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let imported = std::fs::read_to_string(s.path("imported/plain.weft")).unwrap();
    assert_eq!(imported, PLAIN);
}

#[test]
fn swiftui_arguments_override_the_project() {
    let s = swiftui_project("swiftui-args");
    let screen = s.write("screens/plain.weft", PLAIN);
    let elsewhere = s.path("elsewhere");
    let r = run(&[&"swiftui", &screen, &"--out-dir", &elsewhere]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(elsewhere.join("plain.swift").is_file());
    assert!(!s.path("ios").exists());

    // Without the project: printed, with the default tokens in the file.
    let r = run(&[&"swiftui", &screen, &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("var sm: CGFloat = 8"), "{}", r.stdout);

    // The flag beats `export.swiftui.sharedTokens`, which beats the default.
    let r = run(&[
        &"swiftui",
        &screen,
        &"--no-shared-tokens",
        &"--out-dir",
        &elsewhere,
        &"--force",
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let swift = std::fs::read_to_string(elsewhere.join("plain.swift")).unwrap();
    assert!(swift.contains("var sm: CGFloat = 12"), "{swift}");
    let r = run(&[&"swiftui", &screen, &"--no-project", &"--shared-tokens"]);
    assert!(
        r.stdout.contains("var theme = WeftTokens()"),
        "{}",
        r.stdout
    );
    let project = std::fs::read_to_string(s.path("weft.json")).unwrap();
    s.write(
        "weft.json",
        &project.replace(
            r#""outDir": "ios" }"#,
            r#""outDir": "ios", "sharedTokens": false }"#,
        ),
    );
    let r = run(&[&"swiftui", &screen, &"--out-dir", &elsewhere, &"--force"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let swift = std::fs::read_to_string(elsewhere.join("plain.swift")).unwrap();
    assert!(swift.contains("var sm: CGFloat = 12"), "{swift}");

    // `--tokens` and `--out-dir` replace the project's.
    let r = run(&[
        &"swiftui-tokens",
        &"--project",
        &s.path("weft.json"),
        &"--tokens",
        &s.path("tokens/swift.tokens.json"),
        &"--out-dir",
        &elsewhere,
    ]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    let tokens = std::fs::read_to_string(elsewhere.join("WeftTokens.swift")).unwrap();
    assert!(
        tokens.contains("var sm: CGFloat = 12") && !tokens.contains("color"),
        "{tokens}"
    );

    let r = run(&[
        &"import-swiftui",
        &elsewhere.join("plain.swift"),
        &"--no-project",
    ]);
    assert_eq!(
        (r.code, r.stdout.as_str(), r.stderr.as_str()),
        (0, PLAIN, "")
    );
}

#[test]
fn swiftui_refuses_what_it_cannot_generate_and_exits_1() {
    let s = Scratch::new("swiftui-refused");
    // `rating` comes from the project's catalog extension: the app writes `RatingView`.
    let screen = s.path("screens/cart.weft");
    let r = run(&[&"swiftui", &screen, &"--out-dir", &s.path("ios")]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    let swift = std::fs::read_to_string(s.path("ios/cart.swift")).unwrap();
    assert!(swift.contains("RatingView(value: "), "{swift}");
    assert!(swift.contains("`RatingView` (`rating`)"), "{swift}");
    // Without the project's catalog `rating` is not a kind at all.
    let r = run(&[&"swiftui", &screen, &"--no-project"]);
    assert_eq!((r.code, r.stdout.as_str()), (1, ""));

    let broken = s.write(
        "screens/broken.weft",
        "<screen id=\"x\" label=\"X\" weft=\"0.1\">",
    );
    let r = run(&[&"swiftui", &broken]);
    assert_eq!((r.code, r.stdout.as_str()), (1, ""));
    assert!(
        r.stderr.starts_with(&broken.display().to_string()),
        "{}",
        r.stderr
    );
}

#[test]
fn import_swiftui_lists_losses_on_stderr() {
    let s = Scratch::new("swiftui-losses");
    let source = s.write(
        "Hello.swift",
        "import SwiftUI\nstruct Hello: View {\n    var body: some View {\n        Text(\"Hi\").padding()\n    }\n}\n",
    );
    let r = run(&[&"import-swiftui", &source, &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.starts_with("<screen "), "{}", r.stdout);
    assert!(r.stderr.contains(" loss layout: "), "{}", r.stderr);

    let none = s.write("None.swift", "let x = 1\n");
    let r = run(&[&"import-swiftui", &none, &"--no-project"]);
    assert_eq!(r.code, 1, "{}", r.stderr);
    assert!(r.stderr.contains("W601"), "{}", r.stderr);
}

/// The example project with every web target's output directory set; Solid writes TSX, and every
/// generator keeps its source so the importers give the screen back exactly.
fn web_project(name: &str) -> Scratch {
    let s = Scratch::new(name);
    s.write(
        "weft.json",
        r#"{
  "tokens": ["tokens/base.tokens.json"],
  "catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"],
  "export": {
    "html": { "outDir": "out/html", "source": true },
    "react": { "outDir": "out/react", "source": true },
    "solid": { "outDir": "out/solid", "source": true, "typescript": true }
  },
  "import": {
    "html": { "outDir": "back/html" },
    "react": { "outDir": "back/react" },
    "solid": { "outDir": "back/solid" }
  }
}"#,
    );
    s
}

#[test]
fn web_targets_write_where_the_project_says_and_read_back_exactly() {
    let s = web_project("web");
    let screen = s.write("screens/plain.weft", PLAIN);
    for (target, file) in [
        ("html", "out/html/plain.html"),
        ("react", "out/react/plain.jsx"),
        ("solid", "out/solid/plain.tsx"),
    ] {
        let r = run(&[&target, &screen]);
        assert_eq!(
            (r.code, r.stdout.as_str(), r.stderr.as_str()),
            (0, "", ""),
            "{target}"
        );
        let r = run(&[&format!("import-{target}"), &s.path(file)]);
        assert_eq!(
            (r.code, r.stdout.as_str(), r.stderr.as_str()),
            (0, "", ""),
            "{target}"
        );
        let back = std::fs::read_to_string(s.path(&format!("back/{target}/plain.weft"))).unwrap();
        assert_eq!(back, PLAIN, "{target}");
    }
}

#[test]
fn web_arguments_override_the_project() {
    let s = web_project("web-args");
    let screen = s.write("screens/plain.weft", PLAIN);
    let r = run(&[
        &"solid",
        &screen,
        &"--javascript",
        &"--no-source",
        &"--out-dir",
        &s.path("js"),
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let jsx = std::fs::read_to_string(s.path("js/plain.jsx")).unwrap();
    assert!(
        !jsx.contains("weft:source") && jsx.contains("function WeftScreen(props)"),
        "{jsx}"
    );

    let r = run(&[&"react", &screen, &"--no-project", &"--typescript"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        r.stdout.contains("export default function WeftScreen("),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains(": WeftProps"), "{}", r.stdout);

    let r = run(&[&"html", &screen, &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.starts_with("<!doctype html>\n"), "{}", r.stdout);
    assert!(!r.stdout.contains("<script"), "{}", r.stdout);
}

const GREETING: &str = r#"<screen id="greeting" label="Greeting" weft="0.1">
  <text id="name" text="{$.name}"/>
</screen>
"#;

#[test]
fn sample_data_comes_from_the_flag_else_the_project_else_none() {
    let s = Scratch::new("sample-data");
    s.write(
        "weft.json",
        r#"{
  "tokens": ["tokens/base.tokens.json"],
  "catalog": ["catalogs/acme-ui.catalog.json", "catalog.json"],
  "export": { "html": { "data": "data/sample.json" }, "swiftui": { "data": "data/sample.json" } }
}"#,
    );
    s.write("data/sample.json", r#"{ "name": "Ada" }"#);
    let other = s.write("other.json", r#"{ "name": "Grace" }"#);
    let screen = s.write("screens/greeting.weft", GREETING);

    let r = run(&[&"html", &screen]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains(">Ada</div>"), "{}", r.stdout);
    let r = run(&[&"html", &screen, &"--data", &other]);
    assert!(r.stdout.contains(">Grace</div>"), "{}", r.stdout);
    let r = run(&[&"html", &screen, &"--no-project"]);
    assert!(!r.stdout.contains("Ada"), "{}", r.stdout);

    let r = run(&[&"swiftui", &screen]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        r.stdout.contains("name: \"Ada\"") && r.stdout.contains("(model: .sample)"),
        "{}",
        r.stdout
    );
    let r = run(&[&"swiftui", &screen, &"--no-project"]);
    assert!(!r.stdout.contains("static var sample"), "{}", r.stdout);

    let r = run(&[&"html", &screen, &"--data", &s.path("missing.json")]);
    assert_ne!(r.code, 0);
}

#[test]
fn web_importers_list_losses_and_refuse_what_they_cannot_read() {
    let s = Scratch::new("web-losses");
    let source = s.write(
        "Hello.tsx",
        "export default function Hello(props: { data: { n: number } }) {\n  return <main aria-label=\"Hi\"><p style={{ color: \"red\" }}>{`n=${props.data.n}`}</p></main>;\n}\n",
    );
    let r = run(&[&"import-react", &source, &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.starts_with("<screen "), "{}", r.stdout);
    assert!(r.stderr.contains(" loss bindings: "), "{}", r.stderr);

    let broken = s.write(
        "Broken.jsx",
        "export default function () { return <main>; }\n",
    );
    let r = run(&[&"import-solid", &broken, &"--no-project"]);
    assert_eq!(r.code, 1, "{}", r.stderr);
    assert!(r.stderr.contains("W601"), "{}", r.stderr);

    let page = s.write(
        "page.html",
        "<main aria-label=\"P\"><marquee>Hi</marquee></main>",
    );
    let r = run(&[&"import-html", &page, &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("label=\"P\""), "{}", r.stdout);
}

#[test]
fn a_resolver_makes_colours_follow_the_appearance() {
    let s = Scratch::new("resolver");
    // The example project's resolver: the star colour differs in the dark theme.
    let r = run_in(&s.path("screens"), &[&"swiftui-tokens"]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    assert!(
        r.stdout
            .contains("var star: Color = WeftTokens.adaptive(light: "),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains("var sm: CGFloat = 8"), "{}", r.stdout);

    // The same resolver as `--tokens`, from elsewhere: its references are relative to it, and
    // its problems point into it.
    s.write(
        "tokens/broken.resolver.json",
        r##"{"version":"2025.10","resolutionOrder":[{"$ref":"#/sets/none"},{"$ref":"base.tokens.json"}]}"##,
    );
    let r = run(&[
        &"swiftui-tokens",
        &"--no-project",
        &"--tokens",
        &s.path("tokens/broken.resolver.json"),
    ]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        r.stderr
            .contains("broken.resolver.json:#/resolutionOrder/0/$ref W705"),
        "{}",
        r.stderr
    );
    assert!(r.stdout.contains("var sm: CGFloat = 8"), "{}", r.stdout);
    assert!(!r.stdout.contains("adaptive"), "{}", r.stdout);
}

#[test]
fn css_base_is_the_same_everywhere_and_goes_where_the_project_says() {
    let s = Scratch::new("css-base");
    let printed = run_in(&s.path("screens"), &[&"css-base"]);
    assert_eq!((printed.code, printed.stderr.as_str()), (0, ""));
    // Its values come from the tokens' custom properties, so it does not read the project's own.
    assert!(
        printed.stdout.contains("var(--weft-space-md, 16px)"),
        "{}",
        printed.stdout
    );
    let alone = run(&[&"css-base", &"--no-project"]);
    assert_eq!(alone.stdout, printed.stdout);

    with_settings(
        &s,
        serde_json::json!({ "export": { "css": { "outDir": "web" } } }),
    );
    let r = run_in(&s.path("screens"), &[&"css-base"]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    assert_eq!(
        std::fs::read_to_string(s.path("web/weft-base.css")).unwrap(),
        printed.stdout
    );
    let elsewhere = s.path("elsewhere");
    let r = run_in(&s.path("screens"), &[&"css-base", &"--out-dir", &elsewhere]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("weft-base.css")).unwrap(),
        printed.stdout
    );
}

#[test]
fn css_tokens_follow_the_appearance_and_go_where_the_project_says() {
    let s = Scratch::new("css-tokens");
    let r = run_in(&s.path("screens"), &[&"css-tokens"]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    assert!(
        r.stdout.contains("color-scheme: light dark;"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("--weft-color-star: #d97706;"),
        "{}",
        r.stdout
    );
    let dark = r.stdout.split("@media (prefers-color-scheme: dark)").nth(1);
    let dark = dark.unwrap_or_else(|| panic!("{}", r.stdout));
    assert!(dark.contains("--weft-color-star: #fbbf24;"), "{dark}");
    assert!(!dark.contains("--weft-space-sm"), "{dark}");
    assert!(
        r.stdout.contains(
            "--weft-font-body: 400 16px/1.5 system-ui;\n  --weft-font-body-letter-spacing: 0.2px;"
        ),
        "{}",
        r.stdout
    );

    // `export.css.outDir` writes the file; `--out-dir` overrides it.
    with_settings(
        &s,
        serde_json::json!({ "export": { "css": { "outDir": "web" } } }),
    );
    let r = run_in(&s.path("screens"), &[&"css-tokens"]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let written = std::fs::read_to_string(s.path("web/weft-tokens.css")).unwrap();
    assert!(
        written.starts_with("/* Generated by `weft css-tokens`"),
        "{written}"
    );
    let elsewhere = s.path("elsewhere");
    let r = run_in(
        &s.path("screens"),
        &[&"css-tokens", &"--out-dir", &elsewhere],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(
        std::fs::read_to_string(elsewhere.join("weft-tokens.css")).unwrap(),
        written
    );

    // Plain token files have no appearance: no color-scheme and no dark block.
    let r = run(&[&"css-tokens", &"--no-project"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(!r.stdout.contains("color-scheme"), "{}", r.stdout);
    assert!(!r.stdout.contains("@media"), "{}", r.stdout);
}

/// T63: `--no-project` on canonical JSON used to skip the shape layer
/// entirely (`vec![]`), exiting 0 where the library, the MCP server and the
/// markup path all report W200 — and the hint claimed "the syntax layer".
#[test]
fn canonical_json_without_a_project_still_gets_its_shape_checked() {
    let s = Scratch::new("json-shape");
    let broken = s.write("broken.json", "{}");
    let r = run(&[&"validate", &broken, &"--no-project"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains("W200 "), "{}", r.stdout);
    assert!(r.stderr.contains("shape layers"), "{}", r.stderr);
}

/// T71: a generator refuses to overwrite its output without `--force` —
/// the same contract the plugin scripts keep — and says so.
#[test]
fn a_generator_refuses_to_overwrite_without_force() {
    let s = swiftui_project("overwrite-refused");
    let screen = s.write("screens/plain.weft", PLAIN);
    let dir = s.path("gen");
    let r = run(&[&"swiftui", &screen, &"--out-dir", &dir]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let hand_edited = "var sm: CGFloat = 99\n";
    std::fs::write(dir.join("plain.swift"), hand_edited).unwrap();

    let r = run(&[&"swiftui", &screen, &"--out-dir", &dir]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--force"), "{}", r.stderr);
    assert_eq!(
        std::fs::read_to_string(dir.join("plain.swift")).unwrap(),
        hand_edited,
        "the refusal must not touch the file"
    );

    let r = run(&[&"swiftui", &screen, &"--out-dir", &dir, &"--force"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_ne!(
        std::fs::read_to_string(dir.join("plain.swift")).unwrap(),
        hand_edited,
        "--force overwrites"
    );
}

fn corpus_screen(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../corpus/{name}/screen.weft"))
}

/// The example project with both Slint output directories set.
fn slint_project(name: &str) -> Scratch {
    let s = Scratch::new(name);
    let text = std::fs::read_to_string(s.path("weft.json")).unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&text).unwrap();
    json["export"] = serde_json::json!({ "slint": { "outDir": "ui" } });
    json["import"] = serde_json::json!({ "slint": { "outDir": "imported" } });
    s.write("weft.json", &serde_json::to_string_pretty(&json).unwrap());
    s
}

#[test]
fn slint_writes_where_the_project_says_and_reads_back() {
    let s = slint_project("slint");
    let screen = s.write("screens/plain.weft", PLAIN);
    let r = run(&[&"slint", &screen]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let slint = std::fs::read_to_string(s.path("ui/plain.slint")).unwrap();
    assert!(slint.starts_with("// weft:source slint\n"), "{slint}");

    let formatted = run(&[&"fmt", &screen]);
    assert_eq!(formatted.code, 0, "{}", formatted.stderr);
    let r = run(&[&"import-slint", &s.path("ui/plain.slint")]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let imported = std::fs::read_to_string(s.path("imported/plain.weft")).unwrap();
    assert_eq!(imported, formatted.stdout);
}

#[test]
fn slint_arguments_override_the_project_and_prints_when_unset() {
    let s = slint_project("slint-args");
    let screen = s.write("screens/plain.weft", PLAIN);
    let elsewhere = s.path("elsewhere");
    let r = run(&[&"slint", &screen, &"--out-dir", &elsewhere]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(elsewhere.join("plain.slint").is_file());
    assert!(!s.path("ui").exists());

    // No project and no `--out-dir`: the component is printed.
    let r = run(&[&"slint", &screen, &"--no-project"]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    assert!(
        r.stdout.contains("export component PlainScreen"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.starts_with("// weft:source slint\n"),
        "{}",
        r.stdout
    );

    let r = run(&[&"slint", &screen, &"--no-project", &"--name", &"sign-in"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(
        r.stdout.contains("export component SignInScreen"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.starts_with("// weft:source slint name=sign-in\n"),
        "{}",
        r.stdout
    );

    // A project that does not set the directory prints too.
    let bare = Scratch::new("slint-stdout");
    let screen = bare.write("screens/plain.weft", PLAIN);
    let r = run(&[&"slint", &screen]);
    assert_eq!((r.code, r.stderr.as_str()), (0, ""));
    assert!(
        r.stdout.contains("export component PlainScreen"),
        "{}",
        r.stdout
    );

    // The file was generated with the project's tokens, so the import uses that project too.
    let r = run(&[&"import-slint", &elsewhere.join("plain.slint")]);
    assert_eq!((r.code, r.stdout.as_str(), r.stderr.as_str()), (0, "", ""));
    let imported = std::fs::read_to_string(s.path("imported/plain.weft")).unwrap();
    let formatted = run(&[&"fmt", &s.path("screens/plain.weft"), &"--print"]);
    assert_eq!(imported, formatted.stdout);
}

#[test]
fn slint_round_trips_login_signup_and_settings() {
    for name in ["login", "signup", "settings"] {
        let screen = corpus_screen(name);
        let formatted = run(&[&"fmt", &screen, &"--no-project"]);
        assert_eq!(
            (formatted.code, formatted.stderr.as_str()),
            (0, ""),
            "{name}"
        );
        let s = Scratch::new(&format!("slint-{name}"));
        let out_dir = s.path("out");
        let generated = run(&[&"slint", &screen, &"--no-project", &"--out-dir", &out_dir]);
        assert_eq!(
            (
                generated.code,
                generated.stdout.as_str(),
                generated.stderr.as_str()
            ),
            (0, "", ""),
            "{name}"
        );
        let imported = run(&[
            &"import-slint",
            &out_dir.join("screen.slint"),
            &"--no-project",
        ]);
        assert_eq!(
            (
                imported.code,
                imported.stdout.as_str(),
                imported.stderr.as_str()
            ),
            (0, formatted.stdout.as_str(), ""),
            "{name}"
        );
    }
}

#[test]
fn slint_refuses_what_it_cannot_generate_and_exits_1() {
    let s = Scratch::new("slint-refused");
    let screen = s.write(
        "screens/list.weft",
        r#"<screen id="s" weft="0.1"><heading id="title" level="{$.n}">Title</heading></screen>"#,
    );
    let r = run(&[&"slint", &screen, &"--out-dir", &s.path("ui")]);
    assert_eq!((r.code, r.stdout.as_str()), (1, ""));
    assert!(r.stderr.contains("heading#title"), "{}", r.stderr);
    assert!(!s.path("ui").exists());

    let broken = s.write(
        "screens/broken.weft",
        "<screen id=\"x\" label=\"X\" weft=\"0.1\">",
    );
    let r = run(&[&"slint", &broken]);
    assert_eq!((r.code, r.stdout.as_str()), (1, ""));
    assert!(
        r.stderr.starts_with(&broken.display().to_string()),
        "{}",
        r.stderr
    );

    let missing = s.path("missing.weft");
    let r = run(&[&"slint", &missing, &"--no-project"]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("cannot read"), "{}", r.stderr);

    let none = s.write("None.slint", "export component X inherits Window {}\n");
    let r = run(&[&"import-slint", &none, &"--no-project"]);
    assert_eq!((r.code, r.stdout.as_str()), (1, ""));
    assert!(r.stderr.contains("weft:source slint"), "{}", r.stderr);
}

/// What `weft schema` must print for `catalog`: the generator's schema, indented, one newline.
fn generated_schema(catalog: &weft_core::Catalog) -> String {
    let schema = weft_catalog::document_schema(catalog, &Default::default());
    format!("{}\n", serde_json::to_string_pretty(&schema).unwrap())
}

#[test]
fn schema_prints_the_generator_s_schema_of_the_project_catalog() {
    let read = |name: &str| std::fs::read_to_string(Path::new(EXAMPLE).join(name)).ok();
    let project = weft_catalog::load_project_text(
        &std::fs::read_to_string(Path::new(EXAMPLE).join("weft.json")).unwrap(),
        &weft_catalog::ProjectOptions {
            read: Some(&read),
            ..Default::default()
        },
    )
    .unwrap()
    .project;
    let shop = generated_schema(&project.catalog);
    let core = generated_schema(&weft_catalog::core_catalog().unwrap());
    assert_ne!(shop, core);

    // Found from the working directory, as for `weft css-base`.
    let r = run_in(Path::new(EXAMPLE), &[&"schema"]);
    assert_eq!(
        (r.code, r.stdout.as_str()),
        (0, shop.as_str()),
        "{}",
        r.stderr
    );
    let screens = Path::new(EXAMPLE).join("screens");
    assert_eq!(run_in(&screens, &[&"schema"]).stdout, shop);
    let r = run(&[
        &"schema",
        &"--project",
        &Path::new(EXAMPLE).join("weft.json"),
    ]);
    assert_eq!(r.stdout, shop);

    let r = run_in(Path::new(EXAMPLE), &[&"schema", &"--no-project"]);
    assert_eq!(
        (r.code, r.stdout.as_str()),
        (0, core.as_str()),
        "{}",
        r.stderr
    );
    let catalog = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../packages/catalog/catalog.json"
    );
    let r = run_in(Path::new(EXAMPLE), &[&"schema", &"--catalog", &catalog]);
    assert_eq!(r.stdout, core);
}

#[test]
fn schema_writes_where_the_project_says_and_keeps_an_existing_file() {
    let s = Scratch::new("schema");
    with_settings(
        &s,
        serde_json::json!({ "export": { "schema": { "outDir": "gen" } } }),
    );
    let r = run_in(&s.0, &[&"schema"]);
    assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);
    let written = s.path("gen/document.schema.json");
    let first = std::fs::read_to_string(&written).unwrap();
    // `--out-dir` wins over the setting and is relative to the working directory.
    let r = run_in(&s.0, &[&"schema", &"--out-dir", &"flag"]);
    assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);
    assert_eq!(
        std::fs::read_to_string(s.path("flag/document.schema.json")).unwrap(),
        first
    );

    let r = run_in(&s.0, &[&"schema"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("--force"), "{}", r.stderr);
    assert_eq!(std::fs::read_to_string(&written).unwrap(), first);

    let r = run_in(
        &s.0,
        &[
            &"schema",
            &"--force",
            &"--no-project",
            &"--out-dir",
            &s.path("gen"),
        ],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let core = generated_schema(&weft_catalog::core_catalog().unwrap());
    assert_eq!(std::fs::read_to_string(&written).unwrap(), core);
}
