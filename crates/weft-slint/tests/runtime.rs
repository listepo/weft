//! The `runtime` feature: the generated source compiled by the interpreter, and sample data that
//! does not fit reported as problems instead of panics, because the data is untrusted.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use slint_interpreter::{SharedString, Value};
use weft_core::parse_json;
use weft_slint::{GenerateOptions, generate, runtime};

use common::{document, ensure_backend, here, setup};

fn login() -> String {
    let (catalog, tokens) = setup();
    let markup = std::fs::read_to_string(here("../../corpus/login/screen.weft")).unwrap();
    let doc = document(&markup, &catalog, &tokens);
    let options = GenerateOptions {
        catalog: &catalog,
        tokens: &tokens,
        name: None,
    };
    generate(&doc, &options).unwrap()
}

#[test]
fn a_value_of_the_wrong_type_is_a_reported_problem() {
    ensure_backend();
    let source = login();
    let instance = runtime::compile(&source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    let data = parse_json(r#"{ "email": [1, 2], "password": "secret" }"#).unwrap();
    let problems = runtime::apply_data(&instance, &source, &data);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].starts_with("email <string>: "), "{problems:?}");
    // The other properties are still set.
    assert_eq!(
        instance.get_property("password").unwrap(),
        Value::String(SharedString::from("secret"))
    );
}

#[test]
fn data_that_is_not_an_object_sets_nothing() {
    ensure_backend();
    let source = login();
    let instance = runtime::compile(&source, "fluent")
        .unwrap()
        .create()
        .unwrap();
    for data in ["[1, 2]", "null", "\"text\"", "{}"] {
        let problems = runtime::apply_data(&instance, &source, &parse_json(data).unwrap());
        assert!(problems.is_empty(), "{data}: {problems:?}");
    }
}

#[test]
fn a_source_that_does_not_compile_returns_its_errors() {
    let errors = runtime::compile("export component S inherits Window { bogus }", "fluent")
        .map(|definition| definition.name().to_owned())
        .unwrap_err();
    assert!(!errors.is_empty());
}

#[test]
fn a_source_with_two_exported_components_is_refused() {
    let source = "export component A inherits Window {}\nexport component B inherits Window {}";
    let errors = runtime::compile(source, "fluent")
        .map(|definition| definition.name().to_owned())
        .unwrap_err();
    assert_eq!(errors, ["expected one exported component, found 2"]);
}
