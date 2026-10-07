//! Slint the generator did not print, or that was edited: `read_slint` returns a document and
//! the losses of SPEC §9. An unmodified golden still belongs to `import_slint`.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::PathBuf;

use indexmap::IndexMap;
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, core_catalog, load_tokens};
use weft_core::{Child, Code, Node, Value, serialize};
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

fn without_source_comment(source: &str) -> String {
    let mut out = Vec::new();
    let mut skipping = false;
    let mut seen = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !seen && trimmed.is_empty() {
            continue;
        }
        if !seen && trimmed.starts_with("// weft:source slint") {
            seen = true;
            skipping = true;
            continue;
        }
        if skipping {
            if trimmed.starts_with("//") {
                continue;
            }
            skipping = false;
        }
        seen = true;
        out.push(line);
    }
    let mut text = out.join("\n");
    if source.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn read_stripped(path: &str) -> weft_core::Document {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here(path)).unwrap();
    let source = without_source_comment(&source);
    assert!(
        !source.contains("weft:source"),
        "the source comment is still in {path}"
    );
    let result = read_slint(&source, &import_options(&catalog, &tokens));
    assert!(
        result.diagnostics.is_empty(),
        "{path}: {:?}",
        result.diagnostics
    );
    result.document
}

fn find<'a>(node: &'a Node, kind: &str, id: &str) -> Option<&'a Node> {
    if node.kind == kind && node.id.as_deref() == Some(id) {
        return Some(node);
    }
    for child in &node.children {
        if let Child::Node(child) = child
            && let Some(found) = find(child, kind, id)
        {
            return Some(found);
        }
    }
    for list in node.slots.values() {
        for child in list {
            if let Child::Node(child) = child
                && let Some(found) = find(child, kind, id)
            {
                return Some(found);
            }
        }
    }
    None
}

fn assert_kind(doc: &weft_core::Document, kind: &str, id: &str) {
    assert!(
        find(&doc.root, kind, id).is_some(),
        "missing {kind}#{id}\n{}",
        serialize(doc)
    );
}

#[test]
fn a_stripped_list_keeps_each_and_the_ids() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/todo-list.slint")).unwrap();
    let source = without_source_comment(&source);
    let result = read_slint(&source, &import_options(&catalog, &tokens));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .losses
            .iter()
            .any(|loss| loss.kind == LossKind::Repetition),
        "{:?}",
        result.losses
    );
    let doc = &result.document;
    for (kind, id) in [
        ("screen", "todos"),
        ("list", "list"),
        ("field", "draft"),
        ("button", "add"),
        ("checkbox", "todo-done"),
        ("button", "todo-remove"),
        ("text", "remaining"),
    ] {
        assert_kind(doc, kind, id);
    }
    let list = find(&doc.root, "list", "list").unwrap();
    let each = list.children.iter().find_map(|child| match child {
        Child::Node(node) if node.kind == "each" => Some(node.as_ref()),
        _ => None,
    });
    let each = each.expect("the list's for is an each");
    assert_eq!(
        each.props.get("as"),
        Some(&Value::String("todo".into())),
        "the loop variable is as"
    );
    assert!(
        each.children
            .iter()
            .any(|child| matches!(child, Child::Node(node) if node.kind == "item")),
        "the repeated element is an item\n{}",
        serialize(doc)
    );
}

#[test]
fn a_stripped_tabs_screen_keeps_tabs_list_and_ids() {
    let doc = read_stripped("tests/fixtures/tabs.slint");
    for (kind, id) in [
        ("screen", "account"),
        ("tabs", "tabs"),
        ("tab", "tab-overview"),
        ("tab", "tab-billing"),
        ("tab", "tab-team"),
        ("text", "account-name"),
        ("text", "account-email"),
        ("text", "plan"),
        ("button", "change-plan"),
        ("list", "members"),
        ("text", "member-name"),
    ] {
        assert_kind(&doc, kind, id);
    }
    let list = find(&doc.root, "list", "members").unwrap();
    let each = list.children.iter().find_map(|child| match child {
        Child::Node(node) if node.kind == "each" => Some(node.as_ref()),
        _ => None,
    });
    let each = each.expect("the member for is an each");
    assert_eq!(each.props.get("as"), Some(&Value::String("member".into())));
    assert!(
        each.children
            .iter()
            .any(|child| matches!(child, Child::Node(node) if node.kind == "item"))
    );
}

#[test]
fn stripped_screens_keep_menu_dialog_table_radio_and_image() {
    let menu = read_stripped("tests/fixtures/menu.slint");
    assert_kind(&menu, "screen", "account-menu");
    assert_kind(&menu, "menu", "menu");
    assert_kind(&menu, "menu-item", "item-profile");
    assert_kind(&menu, "menu-item", "item-settings");
    assert_kind(&menu, "menu-item", "item-billing");
    assert_kind(&menu, "menu-item", "item-signout");
    let menu_node = find(&menu.root, "menu", "menu").unwrap();
    assert_eq!(
        menu_node.props.get("label"),
        Some(&Value::String("Account".into()))
    );

    let dialog = read_stripped("tests/fixtures/confirm-dialog.slint");
    assert_kind(&dialog, "dialog", "confirm");
    assert_kind(&dialog, "text", "warning");
    assert_kind(&dialog, "text", "filename");
    assert_kind(&dialog, "button", "cancel");
    assert_kind(&dialog, "button", "confirm-delete");
    let popup = find(&dialog.root, "dialog", "confirm").unwrap();
    assert_eq!(
        popup.props.get("label"),
        Some(&Value::String("Delete file?".into()))
    );

    let wizard = read_stripped("tests/fixtures/wizard-step.slint");
    assert_kind(&wizard, "radio-group", "plan");
    assert_kind(&wizard, "radio", "plan-free");
    assert_kind(&wizard, "radio", "plan-pro");
    assert_kind(&wizard, "radio", "plan-team");

    let appearance = read_stripped("tests/fixtures/appearance.slint");
    assert_kind(&appearance, "segmented-control", "theme");
    assert_kind(&appearance, "segment", "theme-light");
    assert_kind(&appearance, "segment", "theme-dark");
    assert_kind(&appearance, "segment", "theme-auto");

    let table = read_stripped("tests/fixtures/data-table.slint");
    assert_kind(&table, "table", "table");
    assert!(
        find(&table.root, "column", "col-name").is_none(),
        "columns stay unread"
    );

    let orders = read_stripped("tests/fixtures/orders.slint");
    assert_kind(&orders, "table", "open-orders");
    assert_kind(&orders, "table", "archived");
    assert_kind(&orders, "table", "pending");
    assert_kind(&orders, "text", "no-orders");

    let profile = read_stripped("tests/fixtures/profile.slint");
    assert_kind(&profile, "image", "avatar");
}

#[test]
fn an_image_is_a_model_only_when_the_comment_says_so() {
    let (catalog, tokens) = options();
    let source = std::fs::read_to_string(here("tests/fixtures/showroom.slint")).unwrap();
    let with = read_slint(&source, &import_options(&catalog, &tokens));
    assert_kind(&with.document, "model", "gem");
    assert_kind(&with.document, "model", "turned");
    let stripped = without_source_comment(&source);
    let without = read_slint(&stripped, &import_options(&catalog, &tokens));
    assert_kind(&without.document, "image", "gem");
    assert_kind(&without.document, "image", "turned");
}

#[test]
fn an_alert_and_a_combobox_follow_the_source_comment() {
    let (catalog, tokens) = options();
    let error = std::fs::read_to_string(here("tests/fixtures/error-state.slint")).unwrap();
    let with_alert = read_slint(&error, &import_options(&catalog, &tokens));
    assert_kind(&with_alert.document, "alert", "alert");
    let stripped_error = without_source_comment(&error);
    let without_alert = read_slint(&stripped_error, &import_options(&catalog, &tokens));
    assert_kind(&without_alert.document, "stack", "alert");

    let booking = std::fs::read_to_string(here("tests/fixtures/booking.slint")).unwrap();
    let with_box = read_slint(&booking, &import_options(&catalog, &tokens));
    assert_kind(&with_box.document, "combobox", "city");
    let stripped_booking = without_source_comment(&booking);
    let without_box = read_slint(&stripped_booking, &import_options(&catalog, &tokens));
    assert_kind(&without_box.document, "select", "city");
}
