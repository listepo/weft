//! The conversions to and from other agent UI formats (SPEC §9) over every corpus screen and
//! catalog example: what A2UI export writes and what it loses are pinned as reviewed snapshots,
//! and the messages are checked against the A2UI v0.9 JSON Schemas.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use weft_core::{Catalog, Document, Mode, ValidateOptions, has_errors, validate_document};
use weft_interop::ImportResult;
use weft_interop::a2ui::{from_a2ui, to_a2ui};

fn screens() -> Vec<(String, Document)> {
    let catalog = common::catalog();
    let tokens = common::tokens();
    let mut out = Vec::new();
    for screen in common::corpus().into_iter().chain(common::examples()) {
        let (document, diagnostics) = common::parse_strict(&screen.markup, &catalog, &tokens);
        let document = document.unwrap_or_else(|| panic!("{}: {diagnostics:?}", screen.name));
        out.push((screen.name, document));
    }
    assert!(out.len() > 40, "found only {} screens", out.len());
    out
}

fn snapshot(target: &str, name: &str, output: &str) {
    insta::with_settings!({
        snapshot_path => format!("snapshots/{target}"),
        prepend_module_to_snapshot => false,
        omit_expression => true,
        description => format!("{name} as {target}"),
    }, {
        insta::assert_snapshot!(name.to_owned(), output);
    });
}

#[test]
fn a2ui_export_snapshots() {
    for (name, document) in screens() {
        let exported = to_a2ui(&document);
        let messages = serde_json::to_string_pretty(&exported.messages).unwrap();
        snapshot("a2ui", &name, &messages);
        let losses: String = exported
            .losses
            .iter()
            .map(|l| format!("{:?} {}: {}\n", l.kind, l.path, l.note))
            .collect();
        snapshot("a2ui-losses", &name, &losses);
    }
}

/// The A2UI v0.9 schemas of `tests/a2ui-schemas`, copied from `specification/v0_9` of
/// `a2ui-project/a2ui` at commit `4787774` (2026-10-05, Apache-2.0): the message envelope, the
/// common types and the basic catalog. The common types name their catalog `catalog.json`, which
/// a client supplies; here it is the basic catalog.
fn a2ui_registry() -> jsonschema::Registry<'static> {
    let read = |name: &str| {
        let dir = common::root().join("crates/weft-snapshots/tests/a2ui-schemas");
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(dir.join(name)).unwrap())
            .unwrap()
    };
    let base = "https://a2ui.org/specification/v0_9";
    jsonschema::Registry::new()
        .add(
            format!("{base}/common_types.json"),
            read("common_types.json"),
        )
        .unwrap()
        .add(format!("{base}/catalog.json"), read("basic_catalog.json"))
        .unwrap()
        .add(
            format!("{base}/catalogs/basic/catalog.json"),
            read("basic_catalog.json"),
        )
        .unwrap()
        .add(
            format!("{base}/server_to_client.json"),
            read("server_to_client.json"),
        )
        .unwrap()
        .prepare()
        .unwrap()
}

/// The first errors of `value` against the schema at `reference`.
fn a2ui_errors(
    registry: &jsonschema::Registry<'_>,
    reference: &str,
    value: &serde_json::Value,
) -> Vec<String> {
    let schema =
        serde_json::json!({ "$ref": format!("https://a2ui.org/specification/v0_9/{reference}") });
    let validator = jsonschema::options()
        .with_registry(registry)
        .should_validate_formats(true)
        .build(&schema)
        .unwrap();
    validator
        .iter_errors(value)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .take(5)
        .collect()
}

/// Every message against the envelope, and every component against its basic-catalog schema,
/// which says more than the envelope's `oneOf` does.
fn a2ui_problems(
    registry: &jsonschema::Registry<'_>,
    messages: &[serde_json::Value],
) -> Vec<String> {
    let mut problems = Vec::new();
    for message in messages {
        let errors = a2ui_errors(registry, "server_to_client.json", message);
        if errors.is_empty() {
            continue;
        }
        for c in message["updateComponents"]["components"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let kind = c["component"].as_str().unwrap_or_default();
            for e in a2ui_errors(
                registry,
                &format!("catalogs/basic/catalog.json#/components/{kind}"),
                c,
            ) {
                problems.push(format!("{} {kind}: {e}", c["id"]));
            }
        }
        if problems.is_empty() {
            problems.extend(errors);
        }
    }
    problems
}

#[test]
fn a2ui_export_validates() {
    let registry = a2ui_registry();
    for (name, document) in screens() {
        let problems = a2ui_problems(&registry, &to_a2ui(&document).messages);
        assert!(problems.is_empty(), "{name}: {problems:#?}");
    }
}

fn assert_lenient_valid(name: &str, result: &ImportResult, catalog: &Catalog) {
    let options = ValidateOptions {
        catalog: Some(catalog),
        mode: Mode::Lenient,
        tokens: None,
        actions: None,
    };
    let diagnostics = validate_document(&result.document, &options);
    assert!(!has_errors(&diagnostics), "{name}: {diagnostics:#?}");
    assert!(
        !has_errors(&result.diagnostics),
        "{name}: {:#?}",
        result.diagnostics
    );
}

/// Every screen goes to A2UI and back to a valid document. What comes back exports to messages
/// that read back to the same document, so a second trip loses nothing more: the first export
/// already dropped what A2UI cannot hold, and a few kinds (an alert is a Card, which is read as a
/// stack) come back as a neighbour.
#[test]
fn a2ui_round_trip_is_stable() {
    let catalog = common::catalog();
    let trip = |name: &str, messages: &[serde_json::Value]| {
        let back = from_a2ui(&serde_json::to_string(messages).unwrap(), &catalog);
        assert_lenient_valid(name, &back, &catalog);
        back.document
    };
    for (name, document) in screens() {
        let first = trip(&name, &to_a2ui(&document).messages);
        let again = to_a2ui(&first).messages;
        let second = trip(&name, &again);
        assert_eq!(first, second, "{name}: a second trip changed the document");
        assert_eq!(
            again,
            to_a2ui(&second).messages,
            "{name}: a second trip changed the messages"
        );
    }
}

/// The hand-made A2UI renditions of the corpus (written from the spec, not by the exporter).
#[test]
fn a2ui_corpus_renditions_import() {
    let catalog = common::catalog();
    let mut count = 0;
    for screen in common::corpus() {
        let path = screen.path.with_file_name("screen.a2ui.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let result = from_a2ui(&text, &catalog);
        assert_lenient_valid(&screen.name, &result, &catalog);
        let losses: String = result
            .losses
            .iter()
            .map(|l| format!("{:?} {}: {}\n", l.kind, l.path, l.note))
            .collect();
        snapshot("a2ui-import-losses", &screen.name, &losses);
        snapshot(
            "a2ui-import",
            &screen.name,
            &weft_core::serialize(&result.document),
        );
        count += 1;
    }
    assert!(count >= 10, "found only {count} A2UI renditions");
}

#[test]
fn a2ui_import_reports_what_it_cannot_read() {
    let catalog = common::catalog();
    let codes = |text: &str| {
        from_a2ui(text, &catalog)
            .diagnostics
            .iter()
            .map(|d| format!("{:?}", d.code))
            .collect::<Vec<_>>()
    };
    assert_eq!(codes("not json"), ["W601"]);
    assert_eq!(codes("[]"), ["W601"]);
    let long = format!(
        "[\"{}\"]",
        "x".repeat(weft_interop::a2ui::MAX_SOURCE_LENGTH)
    );
    assert_eq!(codes(&long), ["W602"]);
    // A component that is used but never defined is a loss, not a failure.
    let lone = r#"{"version":"v0.9","updateComponents":{"surfaceId":"s","components":[
        {"id":"root","component":"Column","children":["gone","chart"]},
        {"id":"chart","component":"Video","url":"https://example.com/v.mp4"}]}}"#;
    let result = from_a2ui(lone, &catalog);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.losses.len(), 2, "{:?}", result.losses);
}
