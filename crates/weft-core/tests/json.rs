//! Claims about the JSON layer: it must print and read JSON exactly as JavaScript does, because
//! canonical JSON and quoted diagnostic values are compared byte for byte with the TypeScript core.

#![allow(clippy::unwrap_used, clippy::panic)]

use serde_json::{Value as Json, json};
use weft_core::{JsonError, MAX_DEPTH, order_keys, parse_json, to_compact};

fn keys(v: &Json) -> Vec<&str> {
    v.as_object().unwrap().keys().map(String::as_str).collect()
}

#[test]
fn parse_json_reads_every_json_value_kind() {
    assert_eq!(parse_json("null").unwrap(), Json::Null);
    assert_eq!(parse_json(" true ").unwrap(), json!(true));
    assert_eq!(
        parse_json("\"a\\u00e9\\ud83d\\ude00\"").unwrap(),
        json!("aé😀")
    );
    assert_eq!(
        parse_json("[1, [2], {\"a\": []}]").unwrap(),
        json!([1, [2], {"a": []}])
    );
}

#[test]
fn parse_json_rejects_trailing_content_and_syntax_errors() {
    for bad in [
        "",
        "{",
        "[1,]",
        "{\"a\":1} x",
        "nul",
        "'a'",
        "{a: 1}",
        "1 2",
    ] {
        assert!(
            matches!(parse_json(bad), Err(JsonError::Syntax(_))),
            "{bad:?}"
        );
    }
}

#[test]
fn parse_json_reads_numbers_exactly_as_json_parse_does() {
    assert_eq!(parse_json("0.1").unwrap().as_f64(), Some(0.1));
    assert_eq!(parse_json("1e21").unwrap().as_f64(), Some(1e21));
    assert_eq!(parse_json("-0").unwrap().as_f64(), Some(-0.0));
    // 2^53 + 1 is not representable; JSON.parse rounds it to 2^53.
    assert_eq!(
        parse_json("9007199254740993").unwrap().as_f64(),
        Some(9_007_199_254_740_992.0)
    );
}

#[test]
fn parse_json_puts_integer_like_keys_first_in_ascending_order() {
    let v =
        parse_json(r#"{"b":1,"2":2,"a":3,"10":4,"01":5,"4294967295":6,"4294967294":7}"#).unwrap();
    // 4294967295 is not an array index in JavaScript, 4294967294 is the largest one.
    assert_eq!(
        keys(&v),
        ["2", "10", "4294967294", "b", "a", "01", "4294967295"]
    );
}

#[test]
fn order_keys_reorders_nested_objects_and_leaves_arrays_in_place() {
    let v = order_keys(json!({"x": [{"b": 1, "1": 2}], "0": {"z": 1, "7": 2}}));
    assert_eq!(keys(&v), ["0", "x"]);
    assert_eq!(keys(&v["0"]), ["7", "z"]);
    assert_eq!(keys(&v["x"][0]), ["1", "b"]);
}

#[test]
fn parse_json_accepts_nesting_up_to_a_full_document_and_refuses_hostile_depth() {
    // A valid document nests three JSON levels per element level, so the limit is above serde's 128.
    let ok = MAX_DEPTH * 3;
    assert!(parse_json(&format!("{}{}", "[".repeat(ok), "]".repeat(ok))).is_ok());
    let hostile = 1_000_000;
    let text = format!("{}{}", "[".repeat(hostile), "]".repeat(hostile));
    assert!(matches!(parse_json(&text), Err(JsonError::TooDeep)));
    assert!(JsonError::TooDeep.to_string().contains("deeper"));
}

#[test]
fn brackets_inside_strings_do_not_count_towards_the_depth() {
    let text = format!("[\"{}\"]", "[".repeat(10_000));
    assert!(parse_json(&text).is_ok());
    let escaped = format!("[\"\\\"{}\"]", "[".repeat(10_000));
    assert!(parse_json(&escaped).is_ok());
}

#[test]
fn to_compact_matches_json_stringify() {
    assert_eq!(
        to_compact(&json!({"a": [1, 2.5, "x"], "b": null})),
        r#"{"a":[1,2.5,"x"],"b":null}"#
    );
    assert_eq!(to_compact(&json!([])), "[]");
    assert_eq!(to_compact(&json!({})), "{}");
}

#[test]
fn to_compact_prints_numbers_as_ecmascript_does() {
    for (n, text) in [
        (json!(1.0), "1"),
        (json!(-0.0), "0"),
        (json!(0.000001), "0.000001"),
        (json!(1e-7), "1e-7"),
        (json!(1e21), "1e+21"),
        (json!(123456789012345680000.0), "123456789012345680000"),
        (json!(0.1 + 0.2), "0.30000000000000004"),
        (json!(5e-324), "5e-324"),
        (json!(f64::MAX), "1.7976931348623157e+308"),
    ] {
        assert_eq!(to_compact(&n), text);
    }
}

#[test]
fn to_compact_prints_numbers_that_are_not_finite_as_null() {
    assert_eq!(to_compact(&f64::NAN), "null");
    assert_eq!(to_compact(&f64::INFINITY), "null");
}

#[test]
fn to_compact_escapes_strings_as_json_stringify_does() {
    assert_eq!(
        to_compact("a\"b\\c\n\t\u{1}\u{7f}é😀"),
        "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}é😀\""
    );
    // Lone surrogates cannot exist in a Rust string, so the escapes JSON.stringify adds for them
    // never apply; U+2028 and U+2029 stay literal in both.
    assert_eq!(to_compact("\u{2028}"), "\"\u{2028}\"");
}

#[test]
fn to_compact_writes_keys_in_javascript_order() {
    let v = json!({"b": 1, "2": 2, "a": 3});
    // serde_json keeps insertion order; JavaScript would list "2" first.
    assert_eq!(to_compact(&v), r#"{"2":2,"b":1,"a":3}"#);
}

#[test]
fn to_compact_survives_values_that_cannot_serialize() {
    use std::collections::HashMap;
    // A map with a non-string key has no JSON form; the quoted value is empty, not a panic.
    let mut m = HashMap::new();
    m.insert((1, 2), 3);
    assert_eq!(to_compact(&m), "");
}
