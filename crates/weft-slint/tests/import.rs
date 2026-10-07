//! Slint the generator did not print, or that was edited: `read_slint` returns a document and
//! the losses of SPEC §9. An unmodified golden still belongs to `import_slint`.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use indexmap::IndexMap;
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens};
use weft_core::{Code, serialize};
use weft_slint::{ImportOptions, LossKind, MAX_SOURCE_LENGTH, import_slint, read_slint};

fn here(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn options() -> (weft_core::Catalog, IndexMap<String, Token>) {
    let tokens = load_tokens(&weft_core::parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    (core_catalog().unwrap(), tokens)
}

fn import_options<'a>(
    catalog: &'a weft_core::Catalog,
    tokens: &'a IndexMap<String, Token>,
) -> ImportOptions<'a> {
    ImportOptions { catalog, tokens }
}

#[test]
fn an_edited_label_is_the_file_text_and_a_text_loss() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/login.slint")).unwrap();
    let edited = source.replacen(
        r#"Text { text: "Forgot password?";"#,
        r#"Text { text: "Reset password";"#,
        1,
    );
    assert_ne!(edited, source);
    let result = read_slint(&edited, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    assert!(markup.contains("Reset password"), "{markup}");
    assert!(!markup.contains("Forgot password?"), "{markup}");
    assert!(
        result.losses.iter().any(|loss| {
            loss.kind == LossKind::Text
                && loss.note.contains("Reset password")
                && loss.note.contains("Forgot password?")
        }),
        "{:?}",
        result.losses
    );
    assert!(result.diagnostics.iter().all(|d| d.code != Code::W601));
}

#[test]
fn a_deleted_element_is_absent() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/login.slint")).unwrap();
    let edited = source.replacen(
        "signup := TouchArea {\n                    clicked => { root.perform(\"nav.signup\", \"signup\"); }\n                    Text { text: \"Create an account\"; color: Palette.accent-background; }\n                }\n",
        "",
        1,
    );
    assert_ne!(edited, source);
    let result = read_slint(&edited, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    assert!(!markup.contains("signup"), "{markup}");
    assert!(!markup.contains("Create an account"), "{markup}");
    assert!(markup.contains("reset"), "{markup}");
    assert!(markup.contains("Forgot password?"), "{markup}");
}

#[test]
fn a_hand_written_component_imports_with_generated_ids() {
    let (catalog, tokens) = options();
    let source = r#"export component Hello inherits Window {
    VerticalBox {
        Text { text: "Hello"; }
        Button { text: "Go"; }
    }
}
"#;
    let result = read_slint(source, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    assert!(markup.contains("Hello"), "{markup}");
    assert!(markup.contains("Go"), "{markup}");
    assert!(markup.contains("<text"), "{markup}");
    assert!(markup.contains("<button"), "{markup}");
    assert!(!result.losses.is_empty());
    assert!(
        result.losses.iter().all(|loss| loss.kind == LossKind::Ids),
        "{:?}",
        result.losses
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn an_unknown_element_is_a_kinds_loss_and_its_children_stay() {
    let (catalog, tokens) = options();
    let source = r#"export component Hello inherits Window {
    VerticalBox {
        Rectangle {
            Text { text: "Inside"; }
        }
        Button { text: "Go"; }
    }
}
"#;
    let result = read_slint(source, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    assert!(markup.contains("Inside"), "{markup}");
    assert!(markup.contains("Go"), "{markup}");
    assert!(
        result
            .losses
            .iter()
            .any(|loss| loss.kind == LossKind::Kinds && loss.note.contains("Rectangle")),
        "{:?}",
        result.losses
    );
}

#[test]
fn input_over_the_length_limit_is_a_diagnostic() {
    let (catalog, tokens) = options();
    let source = "x".repeat(MAX_SOURCE_LENGTH + 1);
    let result = read_slint(&source, &import_options(&catalog, &tokens));
    assert!(result.diagnostics.iter().any(|d| d.code == Code::W602));
}

fn without_source_comment(source: &str) -> String {
    let mut out = Vec::new();
    let mut dropping = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if !dropping && trimmed.starts_with("// weft:source") {
            dropping = true;
            continue;
        }
        if dropping && (trimmed.starts_with("//") || trimmed.is_empty()) {
            continue;
        }
        dropping = false;
        out.push(line);
    }
    out.join("\n")
}

#[test]
fn login_without_the_source_comment_keeps_bindings_and_submit() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/login.slint")).unwrap();
    let source = without_source_comment(&source);
    assert!(!source.contains("weft:source"), "{source}");
    let result = read_slint(&source, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    for needle in [
        "value=\"{$.email}\"",
        "value=\"{$.password}\"",
        "label=\"Email\"",
        "label=\"Password\"",
        "disabled=\"{!$.email}\"",
        "on-submit=\"auth.submit\"",
        "on-press=\"nav.reset\"",
        "on-press=\"nav.signup\"",
        "level=\"1\"",
        "<heading",
        "submit=\"true\"",
    ] {
        assert!(markup.contains(needle), "{needle} missing in {markup}");
    }
    assert!(
        !markup.contains("gap="),
        "16px matches more than one dimension token\n{markup}"
    );
    let kept = [
        LossKind::Bindings,
        LossKind::Actions,
        LossKind::Values,
        LossKind::Names,
        LossKind::Text,
        LossKind::Hidden,
        LossKind::Repetition,
        LossKind::Structure,
        LossKind::Props,
    ];
    assert!(
        result.losses.iter().all(|loss| !kept.contains(&loss.kind)),
        "{markup}\n{:?}",
        result.losses
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn signup_and_settings_without_the_source_comment_keep_bindings() {
    let (catalog, tokens) = options();
    let signup = recovered("tests/fixtures/signup.slint", &catalog, &tokens);
    for needle in [
        "value=\"{$.name}\"",
        "value=\"{$.email}\"",
        "value=\"{$.password}\"",
        "value=\"{$.confirm}\"",
        "checked=\"{$.acceptTerms}\"",
        "label=\"Full name\"",
        "label=\"Confirm password\"",
        "label=\"I accept the terms of service\"",
        "disabled=\"{!$.acceptTerms}\"",
        "on-submit=\"auth.signup\"",
        "on-press=\"nav.signin\"",
        "level=\"1\"",
        ">Create your account<",
        "submit=\"true\"",
    ] {
        assert!(signup.contains(needle), "{needle} missing in {signup}");
    }
    assert!(
        !signup.contains("gap="),
        "16px matches more than one dimension token\n{signup}"
    );

    let settings = recovered("tests/fixtures/settings.slint", &catalog, &tokens);
    for needle in [
        "checked=\"{$.emailAlerts}\"",
        "checked=\"{$.pushAlerts}\"",
        "checked=\"{$.darkMode}\"",
        "value=\"{$.language}\"",
        "label=\"Email alerts\"",
        "label=\"Push notifications\"",
        "label=\"Language\"",
        "label=\"Dark mode\"",
        "disabled=\"{!$.dirty}\"",
        "on-press=\"settings.save\"",
        "level=\"1\"",
        "level=\"2\"",
        ">Settings<",
        ">Notifications<",
        "value=\"en\"",
        "value=\"de\"",
        "value=\"fr\"",
        ">English<",
        ">Deutsch<",
        ">Français<",
    ] {
        assert!(settings.contains(needle), "{needle} missing in {settings}");
    }
}

fn recovered(path: &str, catalog: &weft_core::Catalog, tokens: &IndexMap<String, Token>) -> String {
    let source = std::fs::read_to_string(here(path)).unwrap();
    let source = without_source_comment(&source);
    assert!(!source.contains("weft:source"), "{source}");
    let result = read_slint(&source, &import_options(catalog, tokens));
    let markup = serialize(&result.document);
    let kept = [
        LossKind::Bindings,
        LossKind::Actions,
        LossKind::Values,
        LossKind::Names,
        LossKind::Text,
        LossKind::Hidden,
        LossKind::Repetition,
        LossKind::Structure,
        LossKind::Props,
    ];
    assert!(
        result.losses.iter().all(|loss| !kept.contains(&loss.kind)),
        "{markup}\n{:?}",
        result.losses
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    markup
}

#[test]
fn generated_expressions_round_trip() {
    let (catalog, tokens) = options();
    let source = r#"export component Hello inherits Window {
    in-out property <string> user-email;
    in-out property <string> title-data;
    in-out property <string> language;
    in-out property <bool> busy;
    in-out property <int> count;
    VerticalLayout {
        spacing: 24px;
        title := Text {
            text: root.user-email;
            font-size: 24px;
            font-weight: 700;
            visible: !root.busy;
        }
        kept := Text { text: root.title-data; }
        VerticalLayout {
            Text { text: "Language"; }
            language := ComboBox {
                model: ["English", "Deutsch"];
                current-index: root.language == "de" ? 1 : 0;
                selected => { root.language = ["en", "de"][self.current-index]; root.perform("settings.save", "language"); }
            }
        }
        go := Button {
            text: "Go";
            enabled: root.count != 0;
            clicked => { root.perform("settings.save", "go"); }
        }
    }
}
"#;
    let result = read_slint(source, &import_options(&catalog, &tokens));
    let markup = serialize(&result.document);
    for needle in [
        "gap=\"{token.space.lg}\"",
        "text=\"{$.user.email}\"",
        "text=\"{$.title}\"",
        "hidden=\"{$.busy}\"",
        "level=\"2\"",
        "label=\"Language\"",
        "value=\"{$.language}\"",
        "value=\"en\"",
        "value=\"de\"",
        ">English<",
        ">Deutsch<",
        "on-change=\"settings.save\"",
        "disabled=\"{!$.count}\"",
        "on-press=\"settings.save\"",
    ] {
        assert!(markup.contains(needle), "{needle} missing in {markup}");
    }
    assert!(
        result
            .losses
            .iter()
            .all(|loss| { matches!(loss.kind, LossKind::Ids | LossKind::Kinds) }),
        "{markup}\n{:?}",
        result.losses
    );
}

#[test]
fn an_unmodified_golden_round_trips_on_the_comment_path() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/login.slint")).unwrap();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let back = import_slint(&source, &import_options(&catalog, &tokens)).unwrap();
    assert_eq!(serialize(&back), markup);
}
