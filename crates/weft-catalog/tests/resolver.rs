//! Project tokens from a DTCG resolver document (SPEC §10.3): sets and modifiers flattened in
//! `resolutionOrder`, aliases resolved after the merge, every context kept, and every problem a
//! diagnostic.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{ProjectLoad, ProjectOptions, appearance, load_project};
use weft_core::Code;

fn load_with(project: &Json, files: &Json) -> ProjectLoad {
    let read = |name: &str| files.get(name).map(|f| f.to_string());
    let options = ProjectOptions {
        read: Some(&read),
        ..Default::default()
    };
    load_project(project, &options).unwrap()
}

fn codes(load: &ProjectLoad) -> Vec<(Code, String)> {
    load.diagnostics
        .iter()
        .map(|d| (d.code, d.path.clone()))
        .collect()
}

fn color(hex: &str) -> Json {
    json!({ "$type": "color", "$value": hex })
}

/// The module's own shape: a foundation set, a theme with light and dark, and a semantic alias
/// in the foundation that reads the theme's value after the merge.
fn theme_files() -> Json {
    json!({
        "tokens/theme.resolver.json": {
            "version": "2025.10",
            "sets": {
                "foundation": { "sources": [{ "$ref": "base.tokens.json" }] }
            },
            "modifiers": {
                "theme": {
                    "contexts": {
                        "light": [{ "$ref": "light.tokens.json" }],
                        "dark": [{ "$ref": "dark.tokens.json" }]
                    },
                    "default": "light"
                }
            },
            "resolutionOrder": [
                { "$ref": "#/sets/foundation" },
                { "$ref": "#/modifiers/theme" }
            ]
        },
        "tokens/base.tokens.json": {
            "space": { "sm": { "$type": "dimension", "$value": { "value": 8, "unit": "px" } } },
            "color": { "text": { "$value": "{color.ink}" }, "ink": color("#000000") }
        },
        "tokens/light.tokens.json": { "color": { "ink": color("#111111") } },
        "tokens/dark.tokens.json": { "color": { "ink": color("#eeeeee") } }
    })
}

#[test]
fn a_resolver_gives_the_default_context_and_keeps_every_context() {
    let load = load_with(
        &json!({ "tokens": "tokens/theme.resolver.json" }),
        &theme_files(),
    );
    assert_eq!(codes(&load), []);
    let tokens = load.project.tokens.as_ref().unwrap();
    assert_eq!(tokens["color.text"].value, json!("#111111"));
    assert_eq!(tokens["space.sm"].kind, "dimension");
    let [theme] = load.project.modifiers.as_slice() else {
        panic!("{:?}", load.project.modifiers);
    };
    assert_eq!(theme.name, "theme");
    assert_eq!(theme.default, "light");
    assert_eq!(theme.contexts.keys().collect::<Vec<_>>(), ["light", "dark"]);
    // The alias in the foundation resolves after the merge, so it reads the dark value.
    assert_eq!(theme.contexts["dark"]["color.text"].value, json!("#eeeeee"));
    let pair = appearance(&load.project.modifiers).unwrap();
    assert_eq!(pair.light["color.ink"].value, json!("#111111"));
    assert_eq!(pair.dark["color.ink"].value, json!("#eeeeee"));
}

#[test]
fn without_a_default_the_first_context_applies() {
    let mut files = theme_files();
    let resolver = &mut files["tokens/theme.resolver.json"]["modifiers"]["theme"];
    resolver.as_object_mut().unwrap().remove("default");
    resolver["contexts"] = json!({
        "dark": [{ "$ref": "dark.tokens.json" }],
        "light": [{ "$ref": "light.tokens.json" }]
    });
    let load = load_with(&json!({ "tokens": "tokens/theme.resolver.json" }), &files);
    assert_eq!(codes(&load), []);
    assert_eq!(load.project.modifiers[0].default, "dark");
    let tokens = load.project.tokens.unwrap();
    assert_eq!(tokens["color.ink"].value, json!("#eeeeee"));
}

#[test]
fn inline_entries_pointers_into_files_and_overrides_work() {
    let files = json!({
        "r.resolver.json": {
            "version": "2025.10",
            "resolutionOrder": [
                { "type": "set", "name": "base", "sources": [
                    { "$ref": "lib.json#/brand" },
                    { "size": { "$type": "number", "gap": { "$value": 2 } } }
                ] },
                { "type": "modifier", "name": "mode", "contexts": {
                    "light": [],
                    "dark": [{ "$ref": "lib.json#/night", "accent": color("#ff0000") }]
                } }
            ]
        },
        "lib.json": {
            "brand": { "accent": color("#0000ff") },
            "night": { "accent": color("#00ff00"), "muted": color("#333333") }
        }
    });
    let load = load_with(&json!({ "tokens": "r.resolver.json" }), &files);
    assert_eq!(codes(&load), []);
    let tokens = load.project.tokens.as_ref().unwrap();
    assert_eq!(tokens["accent"].value, json!("#0000ff"));
    assert_eq!(tokens["size.gap"].value, json!(2));
    let dark = &load.project.modifiers[0].contexts["dark"];
    // The key beside `$ref` replaces the referenced one shallowly.
    assert_eq!(dark["accent"].value, json!("#ff0000"));
    assert_eq!(dark["muted"].value, json!("#333333"));
}

#[test]
fn project_content_takes_the_resolver_itself_and_no_files() {
    let project = json!({
        "tokens": {
            "version": "2025.10",
            "sets": { "s": { "sources": [{ "a": color("#123456") }] } },
            "modifiers": { "theme": {
                "contexts": { "light": [], "dark": [{ "a": color("#654321") }] }
            } },
            "resolutionOrder": [
                { "$ref": "#/sets/s" },
                { "$ref": "#/modifiers/theme" },
                { "$ref": "elsewhere.json" }
            ]
        }
    });
    let load = load_project(&project, &ProjectOptions::default()).unwrap();
    assert_eq!(
        codes(&load),
        [(Code::W704, "#/tokens/resolutionOrder/2/$ref".to_owned())]
    );
    assert_eq!(
        load.project.modifiers[0].contexts["dark"]["a"].value,
        json!("#654321")
    );
}

#[test]
fn every_rule_of_the_module_is_a_diagnostic() {
    let files = json!({
        "r.resolver.json": {
            "version": "2025-10",
            "sets": {
                "loop": { "sources": [{ "$ref": "#/sets/loop" }] },
                "bad": { "sources": [{ "$ref": "#/modifiers/one" }] }
            },
            "modifiers": {
                "none": { "contexts": {} },
                "one": { "contexts": { "only": [] } },
                "wrong": { "contexts": { "a": [], "b": [] }, "default": "c" }
            },
            "resolutionOrder": [
                { "$ref": "#/sets/loop" },
                { "$ref": "#/sets/bad" },
                { "$ref": "#/modifiers/missing" },
                { "$ref": "#/resolutionOrder/0" },
                { "type": "set", "sources": [] },
                { "type": "set", "name": "x", "sources": [] },
                { "type": "set", "name": "x", "sources": [] },
                { "$ref": "../outside.json" },
                { "$ref": "https://example.com/t.json" },
                { "$ref": "missing.json" },
                { "$ref": "#/modifiers/wrong" }
            ]
        }
    });
    let load = load_with(&json!({ "tokens": "r.resolver.json" }), &files);
    let got = codes(&load);
    let expect = [
        (Code::W705, "#/tokens/version"),
        (Code::W705, "#/tokens/modifiers/none/contexts"),
        (Code::W705, "#/tokens/modifiers/one/contexts"),
        (Code::W705, "#/tokens/modifiers/wrong/default"),
        (Code::W705, "#/tokens/sets/loop/sources/0/$ref"),
        (Code::W705, "#/tokens/sets/bad/sources/0/$ref"),
        (Code::W705, "#/tokens/resolutionOrder/2/$ref"),
        (Code::W705, "#/tokens/resolutionOrder/3/$ref"),
        (Code::W705, "#/tokens/resolutionOrder/4"),
        (Code::W705, "#/tokens/resolutionOrder/6/name"),
        (Code::W703, "#/tokens/resolutionOrder/7/$ref"),
        (Code::W703, "#/tokens/resolutionOrder/8/$ref"),
        (Code::W704, "#/tokens/resolutionOrder/9/$ref"),
    ];
    for (code, path) in expect {
        assert!(
            got.contains(&(code, path.to_owned())),
            "missing {code:?} at {path}: {got:#?}"
        );
    }
    assert_eq!(got.len(), expect.len(), "{got:#?}");
    // The bad default falls back to the first context; the project still loads.
    assert_eq!(load.project.modifiers[0].default, "a");
    assert!(load.project.tokens.is_some());
}

#[test]
fn a_problem_only_one_context_has_names_the_context() {
    let mut files = theme_files();
    files["tokens/dark.tokens.json"] =
        json!({ "color": { "ink": { "$value": "{color.nothing}" } } });
    let load = load_with(&json!({ "tokens": "tokens/theme.resolver.json" }), &files);
    let messages: Vec<&str> = load
        .diagnostics
        .iter()
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.starts_with("theme=dark: color.ink: Alias {color.nothing}")),
        "{messages:#?}"
    );
    assert!(load.diagnostics.iter().all(|d| d.code == Code::W705));
}

#[test]
fn a_token_that_changes_type_between_contexts_is_reported() {
    let mut files = theme_files();
    files["tokens/dark.tokens.json"] =
        json!({ "color": { "ink": { "$type": "number", "$value": 1 } } });
    let load = load_with(&json!({ "tokens": "tokens/theme.resolver.json" }), &files);
    assert!(
        load.diagnostics
            .iter()
            .any(|d| d.message.contains("is a number here but a color")),
        "{:#?}",
        load.diagnostics
    );
}

#[test]
fn token_files_still_layer_and_have_no_modifiers() {
    let files = json!({ "a.json": { "x": color("#000000") }, "b.json": { "x": color("#ffffff") } });
    let load = load_with(&json!({ "tokens": ["a.json", "b.json"] }), &files);
    assert_eq!(codes(&load), []);
    assert!(load.project.modifiers.is_empty());
    assert!(appearance(&load.project.modifiers).is_none());
    assert_eq!(load.project.tokens.unwrap()["x"].value, json!("#ffffff"));
}
