//! The React and SolidJS importers over the corpus: generated components come back exactly, with
//! their `weft:source` comment and without it.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::Path;

use weft_catalog::{DEFAULT_TOKENS_JSON, core_catalog, load_tokens};
use weft_core::{Document, ParseOptions, WEFT_VERSION, has_errors, parse, parse_json, serialize};
use weft_import::LossKind;
use weft_web::{Framework, ImportOptions, JsxOptions, import_jsx, to_jsx};

fn corpus() -> Vec<(String, Document)> {
    let catalog = core_catalog().unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if let Ok(text) = std::fs::read_to_string(path.join("screen.weft")) {
            let parsed = parse(
                &text,
                &ParseOptions {
                    catalog: Some(&catalog),
                    ..Default::default()
                },
            );
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            out.push((name, parsed.document.unwrap()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

const FLAVOURS: [(Framework, bool); 4] = [
    (Framework::React, false),
    (Framework::React, true),
    (Framework::Solid, false),
    (Framework::Solid, true),
];

fn generate(document: &Document, framework: Framework, typescript: bool, source: bool) -> String {
    let catalog = core_catalog().unwrap();
    to_jsx(
        &serde_json::to_value(document).unwrap(),
        &JsxOptions {
            catalog: &catalog,
            component_name: None,
            framework,
            typescript,
            source,
        },
    )
    .unwrap()
}

#[test]
fn generated_components_come_back_exactly() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    for (name, document) in corpus() {
        for (framework, typescript) in FLAVOURS {
            let code = generate(&document, framework, typescript, true);
            assert!(code.starts_with("/* weft:source "), "{name}");
            let back = import_jsx(&code, typescript, &options);
            let what = format!("{name} {framework:?} ts={typescript}");
            assert!(back.losses.is_empty(), "{what}: {:?}", back.losses);
            assert!(
                !has_errors(&back.diagnostics),
                "{what}: {:?}",
                back.diagnostics
            );
            assert_eq!(serialize(&back.document), serialize(&document), "{what}");
        }
    }
}

/// The document with the ids of its repetitions left out: JSX has no element for an `<each>`, so
/// only the `weft:source` comment carries those ids.
fn without_each_ids(markup: &str) -> String {
    let mut out = String::with_capacity(markup.len());
    let mut rest = markup;
    while let Some(at) = rest.find("<each id=\"") {
        let after = &rest[at + 10..];
        out.push_str(&rest[..at + 10]);
        let end = after.find('"').unwrap();
        out.push('?');
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Known gaps of the convention import, found by the coverage screens (T36). Each screen here
/// must still differ, so a fix fails the test until its entry is removed.
const JSX_CONVENTION_GAPS: &[(&str, &str)] = &[
    (
        "account",
        "explicit type=\"text\" is dropped; radio-group on-change is lost; a radio's bound text comes back as label; a literal \
         hidden element is lost",
    ),
    (
        "appearance",
        "explicit type=\"date\" is dropped; a segment's bound text comes back as label",
    ),
    (
        "booking",
        "the options of a bound <each> in a combobox are lost; the segmented-control on-change is lost",
    ),
    ("dashboard", "an explicit direction=\"column\" is dropped"),
    ("glass", "an explicit direction=\"column\" is dropped"),
    (
        "inbox",
        "the tabs on-change is lost; dialog modal=\"false\" is dropped",
    ),
    (
        "orders",
        "the row selected binding is lost; a table empty slot comes back wrapped in a cell",
    ),
    (
        "layout",
        "`justify` and `grow` are neither drawn nor carried until the web layout tasks (T16.3, T16.8)",
    ),
    (
        "receipt",
        "an inline fragment is a component of its own, which the importer drops as a `kinds` loss \
         with its call sites: an importer never produces a `<use>` (SPEC §10.7)",
    ),
];

#[test]
fn generated_components_without_their_source_come_back_by_convention() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let mut failures = Vec::new();
    for (name, mut document) in corpus() {
        // Importers stamp the current version; the corpus stays at 0.1.
        document.weft = WEFT_VERSION.into();
        for (framework, typescript) in FLAVOURS {
            let code = generate(&document, framework, typescript, false);
            let back = import_jsx(&code, typescript, &options);
            let what = format!("{name} {framework:?} ts={typescript}");
            let got = without_each_ids(&serialize(&back.document));
            let want = without_each_ids(&serialize(&document));
            let other_losses = back.losses.iter().any(|l| l.kind != LossKind::Ids);
            let differs = other_losses || got != want || has_errors(&back.diagnostics);
            let known = JSX_CONVENTION_GAPS.iter().any(|(gap, _)| *gap == name);
            if known && !differs {
                failures.push(format!("{what}: the known gap is fixed; remove it"));
            } else if differs && !known {
                failures.push(format!(
                    "##### {what}\n{got}\n--- want\n{want}\n{:#?}\n{:?}",
                    back.losses, back.diagnostics
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_bound_field_error_comes_back_as_the_fields_error() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let screen = r#"<screen id="s" weft="0.1">
  <field id="age" error="{$.errors.age}" label="Age" value="{$.age}"/>
</screen>"#;
    let parsed = parse(
        screen,
        &ParseOptions {
            catalog: Some(&catalog),
            ..Default::default()
        },
    );
    let document = parsed.document.unwrap();
    for (framework, typescript) in FLAVOURS {
        let code = generate(&document, framework, typescript, false);
        let back = import_jsx(&code, typescript, &options);
        let text = serialize(&back.document);
        assert!(
            text.contains(r#"error="{$.errors.age}""#) && !text.contains("<text"),
            "{framework:?}: {text}"
        );
    }
}

const NOTED: &str = r#"<screen id="s" label="Notes" weft="0.3">
  <context>
    <entry id="why" by="human" kind="intent" name="Ivan">Sign in quickly. @preserve */ end</entry>
    <entry id="go-why" by="agent" for="go" kind="question" name="m" status="open">Primary?</entry>
  </context>
  <button id="go" on-press="auth.go">Go</button>
</screen>
"#;

#[test]
fn context_notes_are_readable_comments_that_importers_ignore() {
    let catalog = core_catalog().unwrap();
    let tokens = load_tokens(&parse_json(DEFAULT_TOKENS_JSON).unwrap()).tokens;
    let options = ImportOptions {
        catalog: &catalog,
        tokens: &tokens,
    };
    let parsed = parse(
        NOTED,
        &ParseOptions {
            catalog: Some(&catalog),
            ..Default::default()
        },
    );
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    let document = parsed.document.unwrap();
    let screen_note = "{/* intent (human Ivan): Sign in quickly. @\\preserve *\\/ end */}";
    let button_note = "{/* question open (agent m): Primary? */}";
    for (framework, typescript) in FLAVOURS {
        let what = format!("{framework:?} ts={typescript}");
        let code = generate(&document, framework, typescript, false);
        let at = |needle: &str| {
            code.find(needle)
                .unwrap_or_else(|| panic!("{what}: {needle}\n{code}"))
        };
        assert!(at("<main") < at(screen_note), "{what}");
        assert!(at(screen_note) < at(button_note), "{what}");
        assert!(at(button_note) < at("<button"), "{what}");
        assert!(!code.contains("@preserve"), "{what}");

        let back = import_jsx(&code, typescript, &options);
        assert!(back.losses.is_empty(), "{what}: {:?}", back.losses);
        let mut bare = document.clone();
        bare.context.clear();
        assert_eq!(serialize(&back.document), serialize(&bare), "{what}");

        let code = generate(&document, framework, typescript, true);
        let back = import_jsx(&code, typescript, &options);
        assert_eq!(serialize(&back.document), NOTED, "{what}");
    }
    let lit = generate(&document, Framework::Lit, false, false);
    assert!(!lit.contains("Primary?"), "{lit}");
}
