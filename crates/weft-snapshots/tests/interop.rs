//! The conversions to and from other agent UI formats (SPEC §9) over every corpus screen and
//! catalog example: what A2UI and json-render export write and what they lose are pinned as
//! reviewed snapshots, the A2UI messages are checked against the A2UI v0.9 JSON Schemas, and the
//! json-render specs against the structural rules of json-render's `validateSpec`.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use std::collections::HashSet;

use serde_json::{Value as Json, json};
use weft_core::{Catalog, Document, Mode, ValidateOptions, has_errors, validate_document};
use weft_interop::ImportResult;
use weft_interop::a2ui::{from_a2ui, to_a2ui};
use weft_interop::json_render::{MAX_SOURCE_LENGTH, from_json_render, to_json_render};

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
/// `a2ui-project/a2ui` at commit `4787774` (2026-10-05, Apache-2.0; whitespace reformatted by the project's formatter): the message envelope, the
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

fn losses_text(losses: &[weft_interop::Loss]) -> String {
    losses
        .iter()
        .map(|l| format!("{:?} {}: {}\n", l.kind, l.path, l.note))
        .collect()
}

/// The sample data next to a corpus screen, which a spec carries as its state.
fn sample(path: &std::path::Path) -> Option<serde_json::Map<String, Json>> {
    let text = std::fs::read_to_string(path.with_file_name("data.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// The structural rules of `validateSpec` in `@json-render/core` 0.21.0 (`spec-validator.ts` at
/// commit `fc2a696` of `vercel-labs/json-render`): the root and every child exist, every element
/// is reachable and placed once, a repeat has children, reads an item only inside another repeat
/// and, with a state, names an array there; `visible`, `on`, `repeat` and `watch` are not props.
/// Every type is a kind of the catalog or `each`.
fn json_render_problems(spec: &Json, catalog: &Catalog) -> Vec<String> {
    let mut problems = Vec::new();
    let elements = spec["elements"].as_object().unwrap();
    let mut seen = HashSet::new();
    let mut todo = vec![(spec["root"].as_str().unwrap().to_owned(), false)];
    while let Some((key, in_repeat)) = todo.pop() {
        let Some(el) = elements.get(&key) else {
            problems.push(format!("missing_child {key}"));
            continue;
        };
        if !seen.insert(key.clone()) {
            problems.push(format!("{key} is placed twice"));
            continue;
        }
        let ty = el["type"].as_str().unwrap();
        if ty != "each" && !catalog.components.contains_key(ty) {
            problems.push(format!("{key}: type {ty} is not in the catalog"));
        }
        for field in ["visible", "on", "repeat", "watch"] {
            if !el["props"][field].is_null() {
                problems.push(format!("{field}_in_props {key}"));
            }
        }
        let repeat = &el["repeat"];
        let children = el["children"].as_array().unwrap();
        if !repeat.is_null() {
            if children.is_empty() {
                problems.push(format!("repeat_without_children {key}"));
            }
            match &repeat["statePath"] {
                Json::Object(_) if !in_repeat => {
                    problems.push(format!("repeat_item_outside_scope {key}"));
                }
                Json::String(p)
                    if !spec["state"].is_null()
                        && !spec["state"].pointer(p).is_some_and(Json::is_array) =>
                {
                    problems.push(format!("repeat_state_mismatch {key} {p}"));
                }
                _ => {}
            }
        }
        let slots = el["slots"].as_object().into_iter().flatten();
        for kid in children
            .iter()
            .chain(slots.flat_map(|(_, l)| l.as_array().unwrap()))
        {
            todo.push((
                kid.as_str().unwrap().to_owned(),
                in_repeat || !repeat.is_null(),
            ));
        }
    }
    for key in elements.keys().filter(|k| !seen.contains(*k)) {
        problems.push(format!("orphaned_element {key}"));
    }
    problems
}

#[test]
fn json_render_export_snapshots() {
    let catalog = common::catalog();
    let mut losses = String::new();
    for (name, document) in screens() {
        let exported = to_json_render(&document, &catalog, None);
        if name.starts_with("corpus-") {
            let spec = serde_json::to_string_pretty(&exported.spec).unwrap();
            snapshot("json-render", &name, &spec);
        }
        if !exported.losses.is_empty() {
            losses.push_str(&format!("{name}\n{}", losses_text(&exported.losses)));
        }
    }
    snapshot("json-render-losses", "all", &losses);
}

#[test]
fn json_render_export_validates() {
    let catalog = common::catalog();
    let corpus = common::corpus();
    for (name, document) in screens() {
        let path = corpus.iter().find(|s| s.name == name).map(|s| &s.path);
        let state = path.and_then(|p| sample(p));
        let spec = to_json_render(&document, &catalog, state.as_ref()).spec;
        let problems = json_render_problems(&spec, &catalog);
        assert!(problems.is_empty(), "{name}: {problems:#?}");
    }
}

/// Every screen goes to json-render and back to a valid document: the same document when the
/// export lost nothing, and in any case one that a second trip leaves as it is.
#[test]
fn json_render_round_trip() {
    let catalog = common::catalog();
    let trip = |document: &Document| {
        let spec = to_json_render(document, &catalog, None);
        let text = serde_json::to_string(&spec.spec).unwrap();
        (spec.losses, from_json_render(&text, &catalog))
    };
    let mut exact = 0;
    for (name, document) in screens() {
        let (lost, back) = trip(&document);
        assert_lenient_valid(&name, &back, &catalog);
        let markup = weft_core::serialize(&back.document);
        if lost.is_empty() {
            assert_eq!(weft_core::serialize(&document), markup, "{name}");
            assert!(back.losses.is_empty(), "{name}: {:#?}", back.losses);
            exact += 1;
        }
        let (_, again) = trip(&back.document);
        assert_eq!(markup, weft_core::serialize(&again.document), "{name}");
    }
    assert!(exact > 40, "only {exact} screens came back unchanged");
}

#[test]
fn json_render_import_reports_what_it_cannot_read() {
    let catalog = common::catalog();
    let codes = |text: &str| {
        from_json_render(text, &catalog)
            .diagnostics
            .iter()
            .map(|d| format!("{:?}", d.code))
            .collect::<Vec<_>>()
    };
    assert_eq!(codes("not json"), ["W601"]);
    assert_eq!(codes("[]"), ["W601"]);
    assert_eq!(codes(r#"{"root":"a","elements":{}}"#), ["W601"]);
    let long = format!("[\"{}\"]", "x".repeat(MAX_SOURCE_LENGTH));
    assert_eq!(codes(&long), ["W602"]);
    // A spec written for another catalog, with json-render's own idioms: what Weft cannot say is
    // a loss, never a failure.
    let foreign = r#"{"root":"page","state":{"todos":[]},"elements":{
        "page":{"type":"Card","props":{"title":"Todos"},"children":["list","gone","name","save"]},
        "list":{"type":"list","props":{},"repeat":{"statePath":"/todos","key":"id"},
            "children":["row"],"visible":{"$state":"/todos/length","gt":0}},
        "row":{"type":"item","props":{"text":{"$item":"title"}},"children":[],
            "on":{"press":[{"action":"todo.open"},{"action":"todo.log"}]}},
        "name":{"type":"text","props":{"text":{"$template":"Hi ${/name}"}},"children":[]},
        "save":{"type":"button","props":{"text":"Save","size":"lg"},"children":["save"],
            "on":{"press":{"action":"save","params":{"draft":true}}}}}}"#;
    let result = from_json_render(foreign, &catalog);
    assert_lenient_valid("foreign", &result, &catalog);
    let kinds: HashSet<String> = result
        .losses
        .iter()
        .map(|l| format!("{:?}", l.kind))
        .collect();
    for kind in [
        "Values",
        "Kinds",
        "Repetition",
        "Hidden",
        "Actions",
        "Props",
        "Structure",
    ] {
        assert!(
            kinds.contains(kind),
            "no {kind} loss in {:#?}",
            result.losses
        );
    }
    let markup = weft_core::serialize(&result.document);
    assert!(markup.contains(r#"as="item" in="{$.todos}">"#), "{markup}");
    assert!(markup.contains(r#"text="{$item.title}""#), "{markup}");
}

/// Malformed graphs are read, not followed: a cycle, a missing element, a bad key and a chain
/// deeper than the import limit each end in a loss or `W602`, never in a panic.
#[test]
fn json_render_import_survives_malformed_graphs() {
    let catalog = common::catalog();
    let kinds = |text: &str| {
        let result = from_json_render(text, &catalog);
        assert_lenient_valid("malformed", &result, &catalog);
        let mut kinds: Vec<String> = result
            .losses
            .iter()
            .map(|l| format!("{:?}", l.kind))
            .collect();
        kinds.sort();
        kinds.dedup();
        (kinds, result)
    };
    let (lost, _) = kinds(
        r#"{"root":"a","elements":{"a":{"type":"screen","props":{},"children":["a","b","c d"]},
        "c d":{"type":"text","props":{"text":"Hi"},"children":[]},
        "e":{"type":"each","props":{},"children":["f"]},"f":7}}"#,
    );
    assert_eq!(lost, ["Ids", "Structure"]);
    let (lost, back) = kinds(
        r#"{"root":"a","elements":{"a":{"type":"screen","props":{},"children":["e"]},
        "e":{"type":"each","props":{},"children":["t"],"on":{"press":{"action":"go"}}},
        "t":{"type":"text","props":{"text":"Hi"},"children":[]}}}"#,
    );
    assert_eq!(lost, ["Actions", "Repetition"], "{:#?}", back.losses);
    assert!(weft_core::serialize(&back.document).contains(r#"<text id="t">Hi</text>"#));
    // Deeper than a document may nest, so past every import limit too.
    let depth = weft_core::MAX_DEPTH + 1;
    let elements: Vec<String> = (0..depth)
        .map(|i| {
            format!(
                r#""e{i}":{{"type":"stack","props":{{}},"children":["e{}"]}}"#,
                i + 1
            )
        })
        .collect();
    let deep = format!(r#"{{"root":"e0","elements":{{{}}}}}"#, elements.join(","));
    // A debug build needs more than the 2 MiB of a test thread at the import limit.
    let codes = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || {
            let result = from_json_render(&deep, &common::catalog());
            result
                .diagnostics
                .iter()
                .map(|d| format!("{:?}", d.code))
                .collect::<Vec<_>>()
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(codes.contains(&"W602".to_owned()), "{codes:?}");
}

#[test]
fn json_render_export_reports_its_losses() {
    let catalog = common::catalog();
    let markup = r#"<screen id="s" label="Groups" weft="0.1">
  <list id="groups">
    <each id="group-each" as="group" in="{$.groups}">
      <item id="group">Tags <stack id="tags">
        <each id="tag-each" as="tag" in="{$group.tags}">
          <text id="tag" hidden="{!$tag.shown}" text="{$group.name}"/>
        </each>
      </stack></item>
    </each>
  </list>
  <text id="note" text="Literal"/>
</screen>
"#;
    let (document, diagnostics) = common::parse_strict(markup, &catalog, &common::tokens());
    let document = document.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let exported = to_json_render(&document, &catalog, None);
    assert!(json_render_problems(&exported.spec, &catalog).is_empty());
    let kinds: Vec<String> = exported
        .losses
        .iter()
        .map(|l| format!("{:?}", l.kind))
        .collect();
    assert_eq!(kinds, ["Text", "Bindings"], "{:#?}", exported.losses);
    let tag = &exported.spec["elements"]["tag"];
    assert_eq!(tag["visible"], serde_json::json!({ "$item": "shown" }));
    // A literal `text` prop and text content are one thing to json-render; content comes back.
    let note = &exported.spec["elements"]["note"]["props"];
    assert_eq!(note["text"], "Literal");
    let back = from_json_render(&exported.spec.to_string(), &catalog);
    assert_lenient_valid("losses", &back, &catalog);
    let markup = weft_core::serialize(&back.document);
    assert!(
        markup.contains(r#"<text id="note">Literal</text>"#),
        "{markup}"
    );
}

fn screen_named(name: &str) -> Document {
    screens()
        .into_iter()
        .find(|(n, _)| n == name)
        .unwrap_or_else(|| panic!("no screen {name}"))
        .1
}

fn parsed(markup: &str) -> Document {
    let (document, diagnostics) =
        common::parse_strict(markup, &common::catalog(), &common::tokens());
    document.unwrap_or_else(|| panic!("{diagnostics:#?}"))
}

/// `layout` keeps `justify` and `grow` through A2UI. `start` is the default, so it is omitted
/// on the way out and not written back.
#[test]
fn a2ui_layout_round_trip_keeps_mapped_forms() {
    let catalog = common::catalog();
    let document = screen_named("corpus-layout");
    let exported = to_a2ui(&document);
    let text = serde_json::to_string(&exported.messages).unwrap();
    let messages: Json = serde_json::from_str(&text).unwrap();
    let components = messages[1]["updateComponents"]["components"]
        .as_array()
        .unwrap();
    let component = |id: &str| components.iter().find(|c| c["id"] == id).unwrap();
    assert!(component("toolbar").get("justify").is_none(), "{text}");
    assert_eq!(component("pager")["justify"], "center");
    assert_eq!(component("actions")["justify"], "end");
    assert_eq!(component("search")["weight"], 1);
    assert!(
        exported
            .losses
            .iter()
            .all(|l| !l.note.contains("justify") && !l.note.contains("grow")),
        "{:#?}",
        exported.losses
    );
    let back = from_a2ui(&text, &catalog);
    assert_lenient_valid("layout", &back, &catalog);
    let markup = weft_core::serialize(&back.document);
    assert!(markup.contains(r#"grow="true""#), "{markup}");
    assert!(markup.contains(r#"justify="center""#), "{markup}");
    assert!(markup.contains(r#"justify="end""#), "{markup}");
    assert!(!markup.contains(r#"justify="start""#), "{markup}");
    assert!(
        back.losses
            .iter()
            .all(|l| !l.note.contains("justify") && !l.note.contains("weight")),
        "{:#?}",
        back.losses
    );
}

/// Each layout form A2UI cannot say is one `layout` loss, and the forms it can say come back.
#[test]
fn a2ui_layout_reports_each_loss() {
    let catalog = common::catalog();
    let document = parsed(
        r#"<screen id="s" weft="0.1">
  <stack id="bar" direction="row" justify="space-between" max-width="{token.size.md}" padding="{token.space.md}" wrap="true">
    <text id="a">A</text>
    <text id="b" grow="true">B</text>
  </stack>
  <stack id="col" justify="end">
    <text id="c">C</text>
  </stack>
  <grid id="g" columns="3" min-column-width="{token.size.sm}" padding="{token.space.sm}">
    <text id="t">Hi</text>
  </grid>
</screen>"#,
    );
    let exported = to_a2ui(&document);
    let text = serde_json::to_string(&exported.messages).unwrap();
    assert!(text.contains(r#""justify":"spaceBetween""#), "{text}");
    assert!(text.contains(r#""justify":"end""#), "{text}");
    assert!(text.contains(r#""weight":1"#), "{text}");
    let notes: Vec<&str> = exported
        .losses
        .iter()
        .filter(|l| format!("{:?}", l.kind) == "Layout")
        .map(|l| l.note.as_str())
        .collect();
    for prop in [
        "wrap",
        "padding",
        "max-width",
        "columns",
        "min-column-width",
    ] {
        assert!(
            notes.iter().any(|n| n.starts_with(prop)),
            "{prop} missing from {notes:?}"
        );
    }
    assert!(
        !exported
            .losses
            .iter()
            .any(|l| l.note.contains("justify") || l.note.contains("grow")),
        "{:#?}",
        exported.losses
    );

    let surface = |components: Json| {
        serde_json::to_string(&json!([
            { "version": "v0.9", "createSurface": { "surfaceId": "s", "catalogId": weft_interop::a2ui::CATALOG_ID } },
            { "version": "v0.9", "updateComponents": { "surfaceId": "s", "components": components } }
        ]))
        .unwrap()
    };
    let row = |justify: &str, a_weight: Option<Json>, b_weight: Option<Json>| {
        let mut a = json!({"id": "a", "component": "Text", "text": "A"});
        let mut b = json!({"id": "b", "component": "Text", "text": "B"});
        if let Some(w) = a_weight {
            a["weight"] = w;
        }
        if let Some(w) = b_weight {
            b["weight"] = w;
        }
        surface(json!([
            {"id": "root", "component": "Column", "children": ["row"]},
            {"id": "row", "component": "Row", "justify": justify, "children": ["a", "b"]},
            a,
            b
        ]))
    };
    for justify in ["spaceAround", "spaceEvenly", "stretch"] {
        let result = from_a2ui(&row(justify, None, None), &catalog);
        assert_lenient_valid(justify, &result, &catalog);
        let markup = weft_core::serialize(&result.document);
        assert!(!markup.contains("justify="), "{justify}: {markup}");
        assert!(
            result
                .losses
                .iter()
                .any(|l| format!("{:?}", l.kind) == "Layout" && l.note.contains(justify)),
            "{justify}: {:#?}",
            result.losses
        );
    }
    let unequal = from_a2ui(&row("center", Some(json!(1)), Some(json!(2))), &catalog);
    assert_lenient_valid("unequal", &unequal, &catalog);
    let markup = weft_core::serialize(&unequal.document);
    assert_eq!(markup.matches(r#"grow="true""#).count(), 2, "{markup}");
    assert!(markup.contains(r#"justify="center""#), "{markup}");
    assert!(
        unequal
            .losses
            .iter()
            .any(|l| l.note.contains("unequal weights")),
        "{:#?}",
        unequal.losses
    );
    let equal = from_a2ui(&row("start", Some(json!(2)), Some(json!(2))), &catalog);
    assert_lenient_valid("equal", &equal, &catalog);
    let markup = weft_core::serialize(&equal.document);
    assert_eq!(markup.matches(r#"grow="true""#).count(), 2, "{markup}");
    assert!(
        !equal.losses.iter().any(|l| l.note.contains("unequal")),
        "{:#?}",
        equal.losses
    );
    let negative = from_a2ui(&row("start", Some(json!(-1)), None), &catalog);
    assert_lenient_valid("negative", &negative, &catalog);
    assert!(
        !weft_core::serialize(&negative.document).contains("grow="),
        "{}",
        weft_core::serialize(&negative.document)
    );
    assert!(
        negative
            .losses
            .iter()
            .any(|l| l.note.contains("below zero")),
        "{:#?}",
        negative.losses
    );
    let text_weight = from_a2ui(&row("start", Some(json!("fill")), None), &catalog);
    assert_lenient_valid("text-weight", &text_weight, &catalog);
    assert!(!weft_core::serialize(&text_weight.document).contains("grow="));
    assert!(
        text_weight
            .losses
            .iter()
            .any(|l| l.note.contains("weight has no Weft form")),
        "{:#?}",
        text_weight.losses
    );
    // A plain root column folds into the screen, so a weight there is not `grow`.
    let folded = from_a2ui(
        &surface(json!([
            {"id": "root", "component": "Column", "children": ["a"]},
            {"id": "a", "component": "Text", "text": "A", "weight": 1}
        ])),
        &catalog,
    );
    assert_lenient_valid("folded", &folded, &catalog);
    assert!(!weft_core::serialize(&folded.document).contains("grow="));
    assert!(
        folded
            .losses
            .iter()
            .any(|l| l.note.contains("weight has no Weft form")),
        "{:#?}",
        folded.losses
    );
}

/// json-render keeps every layout form, including the ones A2UI loses.
#[test]
fn json_render_layout_round_trip_keeps_every_form() {
    let catalog = common::catalog();
    let want = [
        ("corpus-layout", r#"justify="start""#),
        ("corpus-layout", r#"grow="true""#),
        ("corpus-layout", r#"justify="center""#),
        ("corpus-layout", r#"justify="end""#),
        ("corpus-dashboard", r#"justify="space-between""#),
        ("corpus-dashboard", r#"min-column-width="{token.size.sm}""#),
        ("corpus-dashboard", r#"max-width="{token.size.lg}""#),
        ("corpus-glass", r#"padding="{token.space.lg}""#),
        ("corpus-glass", r#"max-width="{token.size.md}""#),
        ("corpus-glass", r#"padding="{token.space.xl}""#),
    ];
    let mut seen = HashSet::new();
    for (name, needle) in want {
        let document = screen_named(name);
        let exported = to_json_render(&document, &catalog, None);
        assert!(exported.losses.is_empty(), "{name}: {:#?}", exported.losses);
        let back = from_json_render(&exported.spec.to_string(), &catalog);
        assert_lenient_valid(name, &back, &catalog);
        let markup = weft_core::serialize(&back.document);
        assert_eq!(weft_core::serialize(&document), markup, "{name}");
        assert!(markup.contains(needle), "{name} missing {needle}: {markup}");
        seen.insert(name);
    }
    assert_eq!(seen.len(), 3);
}
/// A literal `href` or `src` is `http`, `https`, `mailto` or relative. `javascript:` and `data:`
/// are left out on the way out and on the way in; a relative URL is kept (SPEC §9 Trust).
#[test]
fn json_render_keeps_only_trusted_urls() {
    let catalog = common::catalog();
    let markup = r#"<screen id="s" weft="0.1">
  <link id="js" href="javascript:alert(1)">Go</link>
  <image id="pic" src="data:text/html,x" label="pic"/>
  <link id="rel" href="/docs">Docs</link>
</screen>"#;
    let (document, diagnostics) = common::parse_strict(markup, &catalog, &common::tokens());
    let document = document.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let exported = to_json_render(&document, &catalog, None);
    let text = exported.spec.to_string();
    assert!(!text.contains("javascript:"), "{text}");
    assert!(!text.contains("data:"), "{text}");
    assert!(text.contains("/docs"), "{text}");
    let notes: Vec<&str> = exported.losses.iter().map(|l| l.note.as_str()).collect();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("href") && n.contains("left out")),
        "{notes:?}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.contains("src") && n.contains("left out")),
        "{notes:?}"
    );
    assert!(
        exported
            .losses
            .iter()
            .all(|l| l.kind == weft_interop::LossKind::Props),
        "{:#?}",
        exported.losses
    );

    let foreign = r#"{
      "root": "s",
      "elements": {
        "s": {"type": "screen", "props": {"weft": "0.1"}, "children": ["js", "pic", "rel"]},
        "js": {"type": "link", "props": {"href": "javascript:alert(1)", "text": "Go"}, "children": []},
        "pic": {"type": "image", "props": {"src": "data:text/html,x", "label": "pic"}, "children": []},
        "rel": {"type": "link", "props": {"href": "/docs", "text": "Docs"}, "children": []}
      }
    }"#;
    let back = from_json_render(foreign, &catalog);
    assert_lenient_valid("trusted-urls", &back, &catalog);
    let markup = weft_core::serialize(&back.document);
    assert!(!markup.contains("javascript:"), "{markup}");
    assert!(!markup.contains("data:"), "{markup}");
    assert!(markup.contains(r#"href="/docs""#), "{markup}");
    assert!(
        back.losses
            .iter()
            .any(|l| l.kind == weft_interop::LossKind::Props && l.note.contains("href")),
        "{:#?}",
        back.losses
    );
    assert!(
        back.losses
            .iter()
            .any(|l| l.kind == weft_interop::LossKind::Props && l.note.contains("src")),
        "{:#?}",
        back.losses
    );
}
