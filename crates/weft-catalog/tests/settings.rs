//! The Open Design plugin's setting is checked by the settings table like any other section
//! (SPEC §10.6); the plugins the table does not list stay unchecked objects. Text settings
//! (`import.cem.name`) keep only a non-empty string.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_catalog::{ProjectOptions, load_project};

/// The codes and the settings that survive loading `plugins`.
fn load(plugins: &Json) -> (Vec<String>, Json) {
    let loaded = load_project(&json!({ "plugins": plugins }), &ProjectOptions::default()).unwrap();
    let codes = loaded
        .diagnostics
        .iter()
        .map(|d| d.code.as_str().to_owned())
        .collect();
    (codes, loaded.project.settings["plugins"].clone())
}

#[test]
fn a_valid_tokens_dir_is_kept() {
    let plugins = json!({ "open-design": { "tokensDir": "tokens/imported" } });
    assert_eq!(load(&plugins), (vec![], plugins));
}

#[test]
fn a_bad_tokens_dir_is_w703_and_left_out() {
    for bad in ["../escape", "/etc", "a\\b", ""] {
        let (codes, kept) = load(&json!({ "open-design": { "tokensDir": bad } }));
        assert_eq!(codes, ["W703"], "{bad:?}");
        assert_eq!(kept, json!({ "open-design": {} }), "{bad:?}");
    }
    let (codes, kept) = load(&json!({ "open-design": { "tokensDir": 3 } }));
    assert_eq!(codes, ["W701"]);
    assert_eq!(kept, json!({ "open-design": {} }));
}

#[test]
fn an_unknown_key_in_the_open_design_section_is_w702() {
    let (codes, kept) = load(&json!({ "open-design": { "tokenDir": "tokens" } }));
    assert_eq!(codes, ["W702"]);
    assert_eq!(kept, json!({ "open-design": {} }));
}

#[test]
fn other_plugins_stay_unchecked_objects() {
    let plugins = json!({ "my-plugin": { "tokensDir": "../anything", "x": [1] } });
    assert_eq!(load(&plugins), (vec![], plugins));
    let (codes, kept) = load(&json!({ "my-plugin": 1 }));
    assert_eq!(codes, ["W701"]);
    assert_eq!(kept, json!({}));
}

#[test]
fn a_text_setting_keeps_a_non_empty_string_only() {
    let import = json!({ "cem": { "outDir": "catalogs", "name": "acme-ui", "version": "2.1.0" } });
    let loaded = load_project(&json!({ "import": import }), &ProjectOptions::default()).unwrap();
    assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
    assert_eq!(loaded.project.settings["import"], import);
    for bad in [json!(""), json!(1), json!(null)] {
        let loaded = load_project(
            &json!({ "import": { "cem": { "name": bad } } }),
            &ProjectOptions::default(),
        )
        .unwrap();
        let codes: Vec<_> = loaded.diagnostics.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["W701"], "{bad}");
        assert_eq!(
            loaded.project.settings["import"],
            json!({ "cem": {} }),
            "{bad}"
        );
    }
}
