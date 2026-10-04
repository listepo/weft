//! JSON as JavaScript writes and reads it: numbers in ECMAScript's shortest form, the layout of
//! `JSON.stringify`, and keys in JavaScript's own-property order. Canonical JSON (SPEC §3) and
//! the quoted values in diagnostics must match the TypeScript core byte for byte.

use serde::Serialize;
use serde_json::Value as Json;
use thiserror::Error;

use crate::rules::MAX_DEPTH;

#[derive(Debug, Error)]
pub enum JsonError {
    #[error("{0}")]
    Syntax(#[from] serde_json::Error),
    /// Deeper than any valid document; not parsed, so hostile input cannot exhaust the stack.
    #[error("JSON nests deeper than {JSON_DEPTH_LIMIT} levels")]
    TooDeep,
}

/// `Number.prototype.toString`: `1` for 1.0, `1e+21`, `-0` printed as `0`.
pub fn js_number(n: f64) -> String {
    ryu_js::Buffer::new().format(n).to_owned()
}

/// A node level costs at most three JSON levels: node → slots → list → node.
pub const JSON_DEPTH_LIMIT: usize = MAX_DEPTH * 3 + 4;

/// Parses JSON text as `JSON.parse` would, keys in JavaScript order. serde_json's own recursion
/// limit is lower than a valid document can nest, so it is lifted after a bracket scan has
/// bounded the depth.
pub fn parse_json(text: &str) -> Result<Json, JsonError> {
    if bracket_depth(text) > JSON_DEPTH_LIMIT + 1 {
        return Err(JsonError::TooDeep);
    }
    let mut de = serde_json::Deserializer::from_str(text);
    de.disable_recursion_limit();
    let v = <Json as serde::Deserialize>::deserialize(&mut de)?;
    de.end()?;
    Ok(order_keys(v))
}

/// Deepest bracket nesting outside strings; iterative, so it is safe on any input.
fn bracket_depth(text: &str) -> usize {
    let (mut depth, mut max, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for b in text.bytes() {
        if in_string {
            match b {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                max = max.max(depth);
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// JavaScript lists integer-like keys first, in ascending order, then the rest in insertion
/// order. Diagnostics follow object order, so JSON input is reordered the same way.
pub fn order_keys(v: Json) -> Json {
    match v {
        Json::Array(items) => Json::Array(items.into_iter().map(order_keys).collect()),
        Json::Object(map) => {
            let mut entries: Vec<(String, Json)> =
                map.into_iter().map(|(k, v)| (k, order_keys(v))).collect();
            if entries.iter().any(|(k, _)| array_index(k).is_some()) {
                let (mut ints, rest): (Vec<_>, Vec<_>) = entries
                    .into_iter()
                    .partition(|(k, _)| array_index(k).is_some());
                ints.sort_by_key(|(k, _)| array_index(k));
                ints.extend(rest);
                entries = ints;
            }
            Json::Object(entries.into_iter().collect())
        }
        other => other,
    }
}

/// A canonical array index: `0` or a number without leading zeros below 2³² − 1.
pub fn array_index(k: &str) -> Option<u32> {
    if k != "0" && (k.starts_with('0') || k.is_empty()) {
        return None;
    }
    k.parse::<u32>()
        .ok()
        .filter(|&n| n != u32::MAX && k.chars().all(|c| c.is_ascii_digit()))
}

/// `JSON.stringify(value)`. Keys are written in JavaScript order, so that a model built from
/// sorted entries prints as `Object.fromEntries` would.
pub fn to_compact<T: Serialize + ?Sized>(value: &T) -> String {
    let mut out = String::new();
    if let Ok(v) = serde_json::to_value(value) {
        write(&order_keys(v), None, 0, &mut out);
    }
    out
}

/// `JSON.stringify(value, null, 2)`.
pub fn to_pretty<T: Serialize + ?Sized>(value: &T) -> String {
    let mut out = String::new();
    if let Ok(v) = serde_json::to_value(value) {
        write(&order_keys(v), Some(2), 0, &mut out);
    }
    out
}

pub fn quote_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn write(v: &Json, indent: Option<usize>, level: usize, out: &mut String) {
    let newline = |out: &mut String, level: usize| {
        if let Some(n) = indent {
            out.push('\n');
            out.push_str(&" ".repeat(n * level));
        }
    };
    match v {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) => match n.as_f64() {
            Some(f) if f.is_finite() => out.push_str(&js_number(f)),
            _ => out.push_str("null"),
        },
        Json::String(s) => out.push_str(&quote_str(s)),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                write(item, indent, level + 1, out);
            }
            newline(out, level);
            out.push(']');
        }
        Json::Object(map) if map.is_empty() => out.push_str("{}"),
        Json::Object(map) => {
            out.push('{');
            for (i, (k, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, level + 1);
                out.push_str(&quote_str(k));
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write(item, indent, level + 1, out);
            }
            newline(out, level);
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_print_as_javascript_prints_them() {
        assert_eq!(js_number(1.0), "1");
        assert_eq!(js_number(-0.0), "0");
        assert_eq!(js_number(2.5), "2.5");
        assert_eq!(js_number(1e21), "1e+21");
        assert_eq!(js_number(1e-7), "1e-7");
        assert_eq!(js_number(123456789012345680000.0), "123456789012345680000");
    }

    #[test]
    fn layout_matches_json_stringify() {
        let v = json!({"a": [1, {"b": []}], "c": {}, "d": "x\u{1}"});
        assert_eq!(to_compact(&v), r#"{"a":[1,{"b":[]}],"c":{},"d":"x\u0001"}"#);
        assert_eq!(
            to_pretty(&v),
            "{\n  \"a\": [\n    1,\n    {\n      \"b\": []\n    }\n  ],\n  \"c\": {},\n  \"d\": \"x\\u0001\"\n}"
        );
    }

    #[test]
    fn integer_keys_come_first_as_in_javascript() {
        let v = parse_json(r#"{"b":1,"2":2,"a":3,"10":4,"01":5}"#).ok();
        let keys: Vec<String> = v
            .as_ref()
            .and_then(Json::as_object)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        assert_eq!(keys, ["2", "10", "b", "a", "01"]);
    }

    #[test]
    fn deep_json_parses_past_serde_default_limit() {
        let text = format!("{}{}", "[".repeat(500), "]".repeat(500));
        assert!(parse_json(&text).is_ok());
    }

    #[test]
    fn hostile_depth_is_refused_before_parsing() {
        let text = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert!(matches!(parse_json(&text), Err(JsonError::TooDeep)));
    }
}
