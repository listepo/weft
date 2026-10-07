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

#[test]
fn an_unmodified_golden_round_trips_on_the_comment_path() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/login.slint")).unwrap();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let back = import_slint(&source, &import_options(&catalog, &tokens)).unwrap();
    assert_eq!(serialize(&back), markup);
}
