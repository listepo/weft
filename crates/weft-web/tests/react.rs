//! The React generator against `fixtures/react.json` (written by packages/to-jsx/test/
//! differential.ts through this crate). The fixture began as the output of the TypeScript
//! generator this crate replaced, which the port reproduces byte for byte.

// A test crate: a failed unwrap or panic is a failed test, which is the point.
#![allow(clippy::unwrap_used, clippy::panic)]

use weft_catalog::core_catalog;
use weft_core::parse_json;
use weft_web::{Framework, JsxOptions, to_jsx};

const FIXTURE: &str = include_str!("fixtures/react.json");
const SHOWN: usize = 5;

/// Unoptimized builds spend several kilobytes of stack per nesting level, and the fixture holds a
/// document nested past the depth limit.
fn with_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn output_matches_the_typescript_generator() {
    with_stack(compare_with_fixture);
}

fn compare_with_fixture() {
    let catalog = core_catalog().unwrap();
    let cases = parse_json(FIXTURE).unwrap();
    let cases = cases.as_array().unwrap();
    let mut failures = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let options = JsxOptions {
            catalog: &catalog,
            component_name: case.get("componentName").and_then(|n| n.as_str()),
            framework: Framework::React,
            typescript: false,
            source: false,
        };
        let got = to_jsx(&case["document"], &options).unwrap();
        let expected = case["output"].as_str().unwrap();
        if got != expected {
            let at = got
                .chars()
                .zip(expected.chars())
                .take_while(|(a, b)| a == b)
                .count();
            let from = at.saturating_sub(200);
            failures.push(format!(
                "case {i} differs at char {at}\n  expected: …{}\n  got:      …{}",
                expected.chars().skip(from).take(400).collect::<String>(),
                got.chars().skip(from).take(400).collect::<String>()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ from the fixture:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(SHOWN)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
}

#[test]
fn an_invalid_component_name_is_refused() {
    let catalog = core_catalog().unwrap();
    for name in ["x; alert(1)", "lower", "", "A-b"] {
        let options = JsxOptions {
            catalog: &catalog,
            component_name: Some(name),
            framework: Framework::React,
            typescript: false,
            source: false,
        };
        assert!(
            to_jsx(&serde_json::Value::Null, &options).is_err(),
            "{name}"
        );
    }
}
