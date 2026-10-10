//! The JSON Schema a catalog gives its documents (SPEC §3.1): every corpus screen and catalog
//! example passes it, mistakes it can express are rejected by it and by the validator alike, and
//! the generated text is stable.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use indexmap::IndexMap;
use serde_json::{Value as Json, json};
use weft_catalog::{DocumentSchemaOptions, Token, document_schema};
use weft_core::{
    Catalog, Code, Mode, ValidateOptions, canonicalize, is_action, is_binding, is_id,
    is_loop_variable, is_token, stringify, validate,
};

fn schema(catalog: &Catalog) -> Json {
    document_schema(catalog, &DocumentSchemaOptions::default())
}

fn text(schema: &Json) -> String {
    serde_json::to_string_pretty(schema).unwrap()
}

/// The canonical JSON of every screen, parsed strictly against `catalog`.
fn canonical(
    screens: Vec<common::Screen>,
    catalog: &Catalog,
    tokens: &IndexMap<String, Token>,
) -> Vec<(String, Json)> {
    screens
        .into_iter()
        .map(|screen| {
            let (document, diagnostics) = common::parse_strict(&screen.markup, catalog, tokens);
            let document = document.unwrap_or_else(|| panic!("{}: {diagnostics:?}", screen.name));
            let json = serde_json::from_str(&stringify(&canonicalize(&document))).unwrap();
            (screen.name, json)
        })
        .collect()
}

fn assert_all_pass(schema: &Json, screens: &[(String, Json)]) {
    let validator = jsonschema::validator_for(schema).unwrap();
    for (name, document) in screens {
        let errors: Vec<String> = validator
            .iter_errors(document)
            .map(|e| format!("{e} at {}", e.instance_path()))
            .take(5)
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:#?}");
    }
}

fn project_screens() -> Vec<common::Screen> {
    let dir = common::root().join("examples/project/screens");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "weft"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| common::Screen {
            name: format!("project-{}", path.file_stem().unwrap().to_string_lossy()),
            markup: std::fs::read_to_string(&path).unwrap(),
            path,
        })
        .collect()
}

#[test]
fn the_schemas_are_valid_2020_12() {
    let project = common::project("examples/project");
    for catalog in [common::catalog(), project.catalog] {
        let schema = schema(&catalog);
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        jsonschema::meta::validate(&schema).unwrap();
    }
}

#[test]
fn every_screen_passes_the_core_schema() {
    let catalog = common::catalog();
    let screens = canonical(
        common::corpus()
            .into_iter()
            .chain(common::examples())
            .collect(),
        &catalog,
        &common::tokens(),
    );
    assert!(screens.len() > 40, "found only {} screens", screens.len());
    assert_all_pass(&schema(&catalog), &screens);
}

#[test]
fn every_screen_passes_the_project_schema() {
    let project = common::project("examples/project");
    // The schema admits no `<use>` (SPEC §3.1): fragment parameters are not the catalog's.
    let (uses, plain): (Vec<_>, Vec<_>) = project_screens()
        .into_iter()
        .partition(|screen| screen.markup.contains("<use "));
    let screens = canonical(
        common::corpus()
            .into_iter()
            .chain(common::examples())
            .chain(plain)
            .collect(),
        &project.catalog,
        &project.tokens,
    );
    assert!(screens.iter().any(|(name, _)| name == "project-review"));
    let schema = schema(&project.catalog);
    assert_all_pass(&schema, &screens);
    let validator = jsonschema::validator_for(&schema).unwrap();
    let uses = canonical(uses, &project.catalog, &project.tokens);
    assert!(!uses.is_empty());
    for (name, document) in &uses {
        assert!(!validator.is_valid(document), "{name}");
    }
}

fn base() -> Json {
    json!({
        "weft": "0.1",
        "root": {
            "kind": "screen",
            "id": "s",
            "props": { "label": "Start" },
            "children": [
                {
                    "kind": "form",
                    "id": "f",
                    "on": { "submit": "auth.submit" },
                    "children": [
                        { "kind": "field", "id": "email", "props": { "label": "Email", "type": "email", "value": { "bind": "$.email" } } },
                        { "kind": "button", "id": "go", "props": { "disabled": { "bind": "$.email", "not": true }, "submit": true, "variant": "primary" }, "children": ["Go"] }
                    ]
                },
                { "kind": "heading", "id": "h", "props": { "level": 2 }, "children": ["Hello"] },
                { "kind": "stack", "id": "row", "props": { "direction": "row", "gap": { "token": "space.md" } } },
                {
                    "kind": "list",
                    "id": "todos",
                    "children": [
                        { "kind": "each", "id": "e", "props": { "as": "todo", "in": { "bind": "$.todos" } }, "children": [
                            { "kind": "item", "id": "i", "children": [{ "kind": "text", "id": "t", "props": { "text": { "bind": "$todo.title" } } }] }
                        ] }
                    ]
                }
            ]
        }
    })
}

fn strict(document: &Json, catalog: &Catalog) -> Vec<Code> {
    let options = ValidateOptions {
        catalog: Some(catalog),
        mode: Mode::Strict,
        ..ValidateOptions::default()
    };
    validate(document, &options)
        .iter()
        .map(|d| d.code)
        .collect()
}

/// One mistake per case, made at a JSON Pointer: the value there is replaced, or removed when the
/// replacement is `None`; a pointer ending in `/-` appends to the array.
const NEGATIVES: &[(&str, &str, Option<&str>, Code)] = &[
    (
        "unknown kind",
        "/root/children/1/kind",
        Some(r#""headline""#),
        Code::W401,
    ),
    (
        "undeclared prop",
        "/root/children/0/children/1/props/size",
        Some(r#""large""#),
        Code::W402,
    ),
    (
        "wrong enum value",
        "/root/children/0/children/1/props/variant",
        Some(r#""big""#),
        Code::W203,
    ),
    (
        "child kind not allowed",
        "/root/children/3/children/-",
        Some(r#"{"kind":"button","id":"b2","children":["B"]}"#),
        Code::W302,
    ),
    (
        "child kind not allowed in each",
        "/root/children/3/children/0/children/-",
        Some(r#"{"kind":"button","id":"b2","children":["B"]}"#),
        Code::W302,
    ),
    (
        "parent kind not allowed",
        "/root/children/-",
        Some(r#"{"kind":"item","id":"i2","children":["x"]}"#),
        Code::W303,
    ),
    (
        "undeclared slot",
        "/root/children/0/slots",
        Some(r#"{"header":[{"kind":"text","id":"t2","children":["x"]}]}"#),
        Code::W207,
    ),
    (
        "missing required prop",
        "/root/children/1/props/level",
        None,
        Code::W205,
    ),
    (
        "missing required label",
        "/root/children/0/children/0/props/label",
        None,
        Code::W205,
    ),
    (
        "number out of range",
        "/root/children/1/props/level",
        Some("7"),
        Code::W224,
    ),
    (
        "number not whole",
        "/root/children/1/props/level",
        Some("2.5"),
        Code::W224,
    ),
    (
        "undeclared event",
        "/root/children/1/on",
        Some(r#"{"press":"nav.home"}"#),
        Code::W206,
    ),
    (
        "wrong type",
        "/root/children/1/props/level",
        Some(r#""2""#),
        Code::W204,
    ),
    (
        "raw value for a token prop",
        "/root/children/2/props/gap",
        Some(r#""8px""#),
        Code::W204,
    ),
    (
        "token on a boolean prop",
        "/root/children/0/children/1/props/disabled",
        Some(r#"{"token":"space.md"}"#),
        Code::W204,
    ),
    (
        "negated binding on a two-way prop",
        "/root/children/0/children/0/props/value",
        Some(r#"{"bind":"$.email","not":true}"#),
        Code::W218,
    ),
    (
        "binding on a literal-only prop",
        "/root/children/0/children/1/props/submit",
        Some(r#"{"bind":"$.submit"}"#),
        Code::W217,
    ),
    (
        "undeclared state",
        "/root/children/1/props/state",
        Some(r#""busy""#),
        Code::W203,
    ),
    (
        "role on a component",
        "/root/children/1/props/role",
        Some(r#""heading""#),
        Code::W209,
    ),
    (
        "root kind below the root",
        "/root/children/-",
        Some(r#"{"kind":"screen","id":"s2","props":{"label":"x"}}"#),
        Code::W312,
    ),
    (
        "wrong root kind",
        "/root/kind",
        Some(r#""section""#),
        Code::W201,
    ),
    (
        "text where only elements go",
        "/root/children/3/children/-",
        Some(r#""loose text""#),
        Code::W304,
    ),
    (
        "element where only text goes",
        "/root/children/1/children/-",
        Some(r#"{"kind":"text","id":"t3","children":["x"]}"#),
        Code::W304,
    ),
    (
        "each without an element",
        "/root/children/3/children/0/children",
        Some("[]"),
        Code::W314,
    ),
    (
        "each without a loop variable",
        "/root/children/3/children/0/props/as",
        None,
        Code::W222,
    ),
    ("missing id", "/root/children/1/id", None, Code::W202),
    (
        "id breaks the grammar",
        "/root/children/1/id",
        Some(r#""1st""#),
        Code::W212,
    ),
    (
        "action breaks the grammar",
        "/root/children/0/on/submit",
        Some(r#""Auth Submit""#),
        Code::W216,
    ),
    (
        "binding breaks the grammar",
        "/root/children/0/children/0/props/value",
        Some(r#"{"bind":"email"}"#),
        Code::W214,
    ),
    (
        "token breaks the grammar",
        "/root/children/2/props/gap",
        Some(r#"{"token":"space..md"}"#),
        Code::W215,
    ),
    (
        "newer format version",
        "/weft",
        Some(r#""0.4""#),
        Code::W403,
    ),
];

fn mutate(document: &mut Json, pointer: &str, value: Option<&str>) {
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    let target = document.pointer_mut(parent).unwrap();
    match (target, value) {
        (Json::Array(items), Some(v)) if key == "-" => items.push(serde_json::from_str(v).unwrap()),
        (Json::Object(map), Some(v)) => {
            map.insert(key.to_owned(), serde_json::from_str(v).unwrap());
        }
        (Json::Object(map), None) => {
            map.shift_remove(key).unwrap();
        }
        (other, _) => panic!("cannot apply {pointer} to {other}"),
    }
}

#[test]
fn the_negative_set_is_rejected_by_both() {
    let catalog = common::catalog();
    let validator = jsonschema::validator_for(&schema(&catalog)).unwrap();
    let base = base();
    let errors: Vec<String> = validator
        .iter_errors(&base)
        .map(|e| e.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "the base document fails the schema: {errors:#?}"
    );
    assert_eq!(
        strict(&base, &catalog),
        [],
        "the base document fails validation"
    );
    for (name, pointer, value, code) in NEGATIVES {
        let mut document = base.clone();
        mutate(&mut document, pointer, *value);
        assert!(
            !validator.is_valid(&document),
            "{name}: the schema accepts it"
        );
        let codes = strict(&document, &catalog);
        assert!(
            codes.contains(code),
            "{name}: expected {code:?}, the validator gave {codes:?}"
        );
    }
}

/// The exceptions SPEC §3.1 names: the schema admits no extensions and no bound `state` on a kind
/// without states, though strict validation lets both through.
#[test]
fn the_schema_is_stricter_only_where_the_spec_says() {
    let catalog = common::catalog();
    let validator = jsonschema::validator_for(&schema(&catalog)).unwrap();
    let cases = [
        ("/root/children/1/props/x-acme-tone", r#""loud""#),
        (
            "/root/children/-",
            r#"{"kind":"x-acme-map","id":"m","props":{"role":"img","label":"Map"}}"#,
        ),
        ("/root/children/1/props/state", r#"{"bind":"$.state"}"#),
    ];
    for (pointer, value) in cases {
        let mut document = base();
        mutate(&mut document, pointer, Some(value));
        assert!(
            !validator.is_valid(&document),
            "{pointer}: the schema accepts it"
        );
        assert_eq!(
            strict(&document, &catalog),
            [],
            "{pointer}: validation rejects it"
        );
    }
}

#[test]
fn the_extension_kinds_are_constrained_too() {
    let project = common::project("examples/project");
    let validator = jsonschema::validator_for(&schema(&project.catalog)).unwrap();
    let rating = |value: &str| {
        let mut document = base();
        let node =
            format!(r#"{{"kind":"rating","id":"r","props":{{"label":"Score","value":{value}}}}}"#);
        mutate(&mut document, "/root/children/-", Some(&node));
        document
    };
    assert!(validator.is_valid(&rating("4.5")));
    assert_eq!(strict(&rating("4.5"), &project.catalog), []);
    assert!(!validator.is_valid(&rating("6")));
    assert!(strict(&rating("6"), &project.catalog).contains(&Code::W224));
    // The extension's wider enum is admitted by its schema and not by the core one.
    let mut ghost = base();
    mutate(
        &mut ghost,
        "/root/children/0/children/1/props/variant",
        Some(r#""ghost""#),
    );
    assert!(validator.is_valid(&ghost));
    assert!(!jsonschema::is_valid(&schema(&common::catalog()), &ghost));
}

/// A pattern of the schema, the grammar function it stands for, and strings to try both on.
type Grammar<'a> = (&'a Json, fn(&str) -> bool, &'a [&'a str]);

#[test]
fn the_patterns_match_the_grammars() {
    let schema = schema(&common::catalog());
    let defs = &schema["$defs"];
    let grammars: [Grammar<'_>; 5] = [
        (
            &defs["Id"],
            is_id,
            &["a", "Go-1_x", "1a", "-a", "", "a b", "é"],
        ),
        (
            &defs["Binding"]["properties"]["bind"],
            is_binding,
            &[
                "$.a",
                "$.a.0.b_c",
                "$item",
                "$item.10",
                "$",
                "$.",
                "$.01",
                "$a..b",
                "a",
                "$.a-b",
            ],
        ),
        (
            &defs["Token"]["properties"]["token"],
            is_token,
            &["space.md", "a-b_c.0", "", "a..b", ".a", "a b"],
        ),
        (
            &defs["Action"],
            is_action,
            &["nav.back", "cart2.add", "Nav.back", "nav.", "a.B", "a b"],
        ),
        (
            &defs["each:list"]["properties"]["props"]["properties"]["as"],
            is_loop_variable,
            &["todo", "row2", "Todo", "2x", "a_b", ""],
        ),
    ];
    for (pattern, grammar, samples) in grammars {
        let validator = jsonschema::validator_for(pattern).unwrap();
        for sample in samples {
            assert_eq!(
                validator.is_valid(&json!(sample)),
                grammar(sample),
                "{pattern} on {sample:?}"
            );
        }
    }
}

fn snapshot(name: &str, output: &str) {
    insta::with_settings!({
        snapshot_path => "snapshots/document-schema",
        prepend_module_to_snapshot => false,
        omit_expression => true,
        description => format!("the document JSON Schema of {name}"),
    }, {
        insta::assert_snapshot!(name.to_owned(), output);
    });
}

#[test]
fn the_schema_is_deterministic() {
    let catalog = common::catalog();
    let first = text(&schema(&catalog));
    assert_eq!(first, text(&schema(&catalog)));
    // Equal catalogs give equal bytes, whatever the order of their components.
    let mut reversed = catalog.clone();
    reversed.components.reverse();
    assert_eq!(first, text(&schema(&reversed)));
    snapshot("weft-core", &first);
    let project = common::project("examples/project");
    snapshot("project-shop", &text(&schema(&project.catalog)));
}
