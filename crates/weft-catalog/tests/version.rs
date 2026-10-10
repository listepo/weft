//! `version` classification (SPEC §8): fragment signatures, screen host contracts and catalogs.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::json;
use weft_catalog::{VersionLevel, check_catalogs, check_documents, core_catalog};
use weft_core::{Document, ParseOptions, parse};

fn document(markup: &str) -> Document {
    let catalog = core_catalog().unwrap();
    let result = parse(
        markup,
        &ParseOptions {
            catalog: Some(&catalog),
            ..ParseOptions::default()
        },
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.document.unwrap()
}

fn level(old: &str, new: &str) -> (VersionLevel, Option<String>, bool) {
    let catalog = core_catalog().unwrap();
    let check = check_documents(&document(old), &document(new), &catalog);
    (check.level, check.least, check.ok)
}

#[test]
fn a_fragment_interface_change_uses_the_catalog_levels_and_cargo_zero() {
    let base = r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    let optional = r#"<fragment label="Row" version="1.1.0" weft="0.3"><param name="title" required="true" type="string"/><param name="note" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    assert_eq!(
        level(base, optional),
        (VersionLevel::Minor, Some("1.1.0".into()), true)
    );

    let breaking = r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><param name="note" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    assert_eq!(
        level(base, breaking),
        (VersionLevel::Major, Some("2.0.0".into()), false)
    );

    let zero = r#"<fragment label="Row" version="0.2.0" weft="0.3"><param name="title" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    let zero_break = r#"<fragment label="Row" version="0.2.1" weft="0.3"><param name="title" required="true" type="string"/><param name="note" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    assert_eq!(
        level(zero, zero_break),
        (VersionLevel::Major, Some("0.3.0".into()), false)
    );
}

#[test]
fn a_fragment_body_change_is_a_patch_and_a_label_change_is_too() {
    let base = r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    let body = r#"<fragment label="Row" version="1.0.0" weft="0.3"><param name="title" required="true" type="string"/><text id="t" tone="muted" text="{$title}"/></fragment>"#;
    assert_eq!(
        level(base, body),
        (VersionLevel::Patch, Some("1.0.1".into()), false)
    );
    let label = r#"<fragment label="Price row" version="1.0.1" weft="0.3"><param name="title" required="true" type="string"/><text id="t" text="{$title}"/></fragment>"#;
    assert_eq!(
        level(base, label),
        (VersionLevel::Patch, Some("1.0.1".into()), true)
    );
}

#[test]
fn a_screen_is_classified_by_its_host_contract() {
    let base =
        r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><text id="t">Hi</text></screen>"#;
    let renamed =
        r#"<screen id="s" label="S" version="1.0.1" weft="0.3"><text id="t">Hello</text></screen>"#;
    assert_eq!(
        level(base, renamed),
        (VersionLevel::Patch, Some("1.0.1".into()), true)
    );

    let added = r#"<screen id="s" label="S" version="1.1.0" weft="0.3"><text id="t">Hi</text><text id="u">More</text></screen>"#;
    assert_eq!(
        level(base, added),
        (VersionLevel::Minor, Some("1.1.0".into()), true)
    );

    let lost =
        r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><text id="u">Hi</text></screen>"#;
    assert_eq!(
        level(base, lost),
        (VersionLevel::Major, Some("2.0.0".into()), false)
    );

    let action = r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><button id="t" on-press="nav.next">Go</button></screen>"#;
    assert_eq!(level(base, action).0, VersionLevel::Major);

    let read = r#"<screen id="s" label="S" version="2.0.0" weft="0.3"><text id="t" text="{$.title}"/></screen>"#;
    assert_eq!(level(base, read).0, VersionLevel::Major);

    let text = r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><text id="t" text="{$.title}"/></screen>"#;
    let number = r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><slider id="t" label="N" value="{$.title}"/></screen>"#;
    assert_eq!(level(text, number).0, VersionLevel::Major);

    let shown = r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><text id="t" text="{$.draft}"/></screen>"#;
    let written = r#"<screen id="s" label="S" version="1.0.0" weft="0.3"><field id="t" label="Draft" value="{$.draft}"/></screen>"#;
    assert_eq!(level(shown, written).0, VersionLevel::Major);

    let stamped =
        r#"<screen id="s" label="S" version="1.0.0" weft="0.1"><text id="t">Hi</text></screen>"#;
    assert_eq!(
        level(base, stamped),
        (VersionLevel::None, Some("1.0.0".into()), true)
    );
}

#[test]
fn a_library_catalog_raises_its_version_for_kinds_and_fragments() {
    let catalog = core_catalog().unwrap();
    let old = json!({
        "weft": "0.3", "name": "acme", "version": "1.2.0", "prefix": "acme",
        "components": { "acme-button": { "description": "B", "role": "button", "content": "none" } }
    });
    let mut next = old.clone();
    next["components"]["acme-button"]["props"] = json!({
        "label": { "description": "L", "type": "string", "required": true }
    });
    next["version"] = json!("1.2.1");
    let kinds = check_catalogs(&old, &next, &catalog);
    assert_eq!(kinds.level, VersionLevel::Major);
    assert_eq!(kinds.least.as_deref(), Some("2.0.0"));
    assert!(!kinds.ok);

    let fragment = json!({
        "weft": "0.3", "version": "1.0.0",
        "root": { "kind": "fragment", "props": { "label": "Row" }, "children": [
            { "kind": "text", "id": "t", "children": ["Hi"] }
        ]}
    });
    let mut moved = fragment.clone();
    moved["root"]["children"][0]["children"] = json!(["Hello"]);
    let mut with_old = old.clone();
    with_old["fragments"] = json!({ "acme-row": fragment });
    let mut with_new = old.clone();
    with_new["version"] = json!("1.2.0");
    with_new["fragments"] = json!({ "acme-row": moved });
    let both = check_catalogs(&with_old, &with_new, &catalog);
    assert_eq!(both.level, VersionLevel::Patch);
    assert_eq!(both.least.as_deref(), Some("1.2.1"));
    assert!(!both.ok);
}
