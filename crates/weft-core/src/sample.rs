//! Sample data as the reference renderer (`@weft/render-react`, `values.ts`) reads it, for the
//! Rust generators that render or embed data at generation time: a binding path resolved over
//! JSON, and the two readings of a resolved value that SPEC §2.1 defines, truthiness and text.
//! The data is untrusted: lookups follow own keys only and nothing here panics.

use serde_json::Value as Json;

use crate::json::js_number;
use crate::rules::is_binding;

/// The value at `path` (`$.a.0.b` or `$item.b`), or `None` when the path is malformed, names a
/// loop variable that is not in `vars`, or steps past the data. `vars` holds the loop variables
/// in scope, innermost last.
pub fn resolve_path<'a>(data: &'a Json, vars: &[(&str, &'a Json)], path: &str) -> Option<&'a Json> {
    if !is_binding(path) {
        return None;
    }
    let rest = path.strip_prefix('$')?;
    let (mut value, segments) = match rest.strip_prefix('.') {
        Some(segments) => (data, segments),
        None => {
            let (name, segments) = rest.split_once('.').unwrap_or((rest, ""));
            let (_, item) = vars.iter().rev().find(|(n, _)| *n == name)?;
            (*item, segments)
        }
    };
    if segments.is_empty() {
        return Some(value);
    }
    for segment in segments.split('.') {
        value = match value {
            // `is_binding` admits only canonical indices, so parsing is the whole check.
            Json::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            Json::Object(map) => map.get(segment)?,
            _ => return None,
        };
    }
    Some(value)
}

/// SPEC §2.1 truthiness: JavaScript's, so an empty array or object is true, except that the
/// string `"false"` is false, as a lenient reader of an untyped attribute means it.
pub fn truthy(value: Option<&Json>) -> bool {
    match value {
        None | Some(Json::Null) => false,
        Some(Json::Bool(b)) => *b,
        Some(Json::Number(n)) => n.as_f64().is_some_and(|n| n != 0.0 && !n.is_nan()),
        Some(Json::String(s)) => !s.is_empty() && s != "false",
        Some(Json::Array(_) | Json::Object(_)) => true,
    }
}

/// The text a value shows: a string as it is, a finite number and a boolean printed as
/// JavaScript prints them, anything else nothing.
pub fn text(value: Option<&Json>) -> String {
    match value {
        Some(Json::String(s)) => s.clone(),
        Some(Json::Number(n)) => n
            .as_f64()
            .filter(|n| n.is_finite())
            .map(js_number)
            .unwrap_or_default(),
        Some(Json::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn paths_resolve_from_the_root_and_from_loop_variables() {
        let data = json!({ "a": { "b": [10, { "c": "x" }] }, "n": null });
        let item = json!({ "name": "Ada" });
        let vars = [("item", &item)];
        assert_eq!(resolve_path(&data, &[], "$.a.b.0"), Some(&json!(10)));
        assert_eq!(resolve_path(&data, &[], "$.a.b.1.c"), Some(&json!("x")));
        assert_eq!(
            resolve_path(&data, &vars, "$item.name"),
            Some(&json!("Ada"))
        );
        assert_eq!(resolve_path(&data, &vars, "$item"), Some(&item));
        assert_eq!(resolve_path(&data, &[], "$item.name"), None);
        assert_eq!(resolve_path(&data, &[], "$.a.b.2"), None);
        assert_eq!(resolve_path(&data, &[], "$.a.b.01"), None);
        assert_eq!(resolve_path(&data, &[], "$.n.x"), None);
        assert_eq!(resolve_path(&data, &[], "$."), None);
        assert_eq!(resolve_path(&data, &[], "$.a.length"), None);
    }

    #[test]
    fn truthiness_follows_spec_2_1() {
        for value in [json!([]), json!({}), json!("0"), json!(-1), json!(true)] {
            assert!(truthy(Some(&value)), "{value}");
        }
        for value in [
            json!(""),
            json!("false"),
            json!(0),
            json!(false),
            json!(null),
        ] {
            assert!(!truthy(Some(&value)), "{value}");
        }
        assert!(!truthy(None));
    }

    #[test]
    fn text_prints_scalars_only() {
        assert_eq!(text(Some(&json!(2.5))), "2.5");
        assert_eq!(text(Some(&json!(1e21))), "1e+21");
        assert_eq!(text(Some(&json!(false))), "false");
        assert_eq!(text(Some(&json!("a"))), "a");
        assert_eq!(text(Some(&json!([1]))), "");
        assert_eq!(text(Some(&json!(null))), "");
        assert_eq!(text(None), "");
    }
}
