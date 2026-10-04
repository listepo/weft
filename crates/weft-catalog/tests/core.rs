//! Claims about the embedded core catalog of SPEC §5.1.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::Value as Json;
use weft_catalog::{CORE_CATALOG_JSON, CatalogError, ChangeLevel, core_catalog, diff_catalogs};
use weft_core::{ARIA_ROLES, ParseOptions, WEFT_VERSION, parse, parse_json, to_compact};

#[test]
fn the_embedded_json_is_the_catalog_the_core_reads() {
    let catalog = core_catalog().unwrap();
    assert_eq!(catalog.name, "weft-core");
    assert_eq!(catalog.weft, WEFT_VERSION);
    assert_eq!(catalog.components.len(), 29);
    for name in [
        "screen", "stack", "text", "button", "form", "field", "list", "item", "dialog",
    ] {
        assert!(catalog.components.contains_key(name), "{name}");
    }
}

#[test]
fn reading_the_catalog_into_the_model_and_writing_it_back_loses_nothing() {
    let written = serde_json::to_value(core_catalog().unwrap()).unwrap();
    let embedded = parse_json(CORE_CATALOG_JSON).unwrap();
    fn first_difference(path: &str, a: &Json, b: &Json) -> Option<String> {
        match (a, b) {
            (Json::Object(x), Json::Object(y)) => {
                x.keys()
                    .chain(y.keys())
                    .find_map(|k| match (x.get(k), y.get(k)) {
                        (Some(p), Some(q)) => first_difference(&format!("{path}.{k}"), p, q),
                        (p, q) => Some(format!("{path}.{k}: {p:?} vs {q:?}")),
                    })
            }
            // 1 and 1.0 are the same JSON number to JavaScript, and to the printer.
            _ => (to_compact(a) != to_compact(b)).then(|| format!("{path}: {a} vs {b}")),
        }
    }
    assert_eq!(first_difference("", &written, &embedded), None);
    assert_eq!(diff_catalogs(&embedded, &written).level, ChangeLevel::None);
}

#[test]
fn every_component_is_described_and_has_a_real_role() {
    for (name, component) in &core_catalog().unwrap().components {
        assert!(!component.description.trim().is_empty(), "{name}");
        assert!(
            component.role == "none" || ARIA_ROLES.contains(&component.role.as_str()),
            "{name}: {}",
            component.role
        );
        for (prop, def) in component.props.iter().flatten() {
            assert!(!def.description.trim().is_empty(), "{name}.{prop}");
        }
    }
}

#[test]
fn every_declared_child_and_parent_kind_is_a_component_of_the_catalog() {
    let catalog = core_catalog().unwrap();
    for (name, component) in &catalog.components {
        let slots = component.slots.iter().flat_map(|m| m.values());
        let lists = component
            .allowed_children
            .iter()
            .chain(&component.allowed_parents)
            .chain(slots.filter_map(|s| s.allowed_children.as_ref()));
        for list in lists {
            for kind in list {
                // `each` is structure of the format, not a component.
                assert!(
                    kind == "each" || catalog.components.contains_key(kind),
                    "{name} mentions {kind}"
                );
            }
        }
    }
}

#[test]
fn the_core_catalog_checks_the_markup_written_for_it() {
    let catalog = core_catalog().unwrap();
    let options = ParseOptions {
        catalog: Some(&catalog),
        ..ParseOptions::default()
    };
    let ok =
        "<screen id=\"s\" weft=\"0.1\"><stack id=\"a\"><text id=\"t\">Hi</text></stack></screen>";
    assert!(parse(ok, &options).diagnostics.is_empty());
    let bad = "<screen id=\"s\" weft=\"0.1\"><stack id=\"a\" direction=\"diagonal\"/></screen>";
    assert_eq!(parse(bad, &options).diagnostics[0].code.as_str(), "W203");
}

#[test]
fn a_catalog_that_does_not_parse_is_an_error_that_says_so() {
    let source = serde_json::from_str::<Json>("{").unwrap_err();
    let message = CatalogError::from(source).to_string();
    assert!(
        message.starts_with("the embedded core catalog does not parse: "),
        "{message}"
    );
}
