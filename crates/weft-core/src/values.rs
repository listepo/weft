//! Attribute value forms of SPEC §2.1 in both directions, so the parser and the serializer share
//! one definition of the `{`-escape and of reference syntax.

use crate::json::js_number;
use crate::model::{PropType, Value};
use crate::rules::is_json_number;

pub struct BadValue {
    pub message: &'static str,
    pub hint: String,
}

/// Reads a decoded attribute value; `ty` comes from the catalog and only shapes literals.
pub fn read_value(raw: &str, ty: Option<PropType>) -> Result<Value, BadValue> {
    if let Some(rest) = raw.strip_prefix("{{") {
        return Ok(Value::String(format!("{{{rest}")));
    }
    if raw.starts_with('{') {
        let inner = raw.strip_prefix('{').and_then(|r| r.strip_suffix('}'));
        if let Some(inner) = inner {
            if inner.starts_with('$') {
                return Ok(Value::Bind {
                    bind: inner.to_owned(),
                    not: false,
                });
            }
            if let Some(bind) = inner.strip_prefix('!').filter(|b| b.starts_with('$')) {
                return Ok(Value::Bind {
                    bind: bind.to_owned(),
                    not: true,
                });
            }
            if let Some(token) = inner.strip_prefix("token.") {
                return Ok(Value::Token(token.to_owned()));
            }
        }
        return Err(BadValue {
            message: "A value starting with `{` must be a whole `{$…}`, `{!$…}` or `{token.…}` reference.",
            hint: format!("write \"{{{raw}\" if the text itself starts with \"{{\""),
        });
    }
    if ty == Some(PropType::Number)
        && is_json_number(raw)
        && let Ok(n) = raw.parse::<f64>()
        && n.is_finite()
    {
        return Ok(Value::Number(if n == 0.0 { 0.0 } else { n }));
    }
    if ty == Some(PropType::Boolean) && (raw == "true" || raw == "false") {
        return Ok(Value::Bool(raw == "true"));
    }
    Ok(Value::String(raw.to_owned()))
}

/// Writes a value as attribute text, before XML escaping.
pub fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) if s.starts_with('{') => format!("{{{s}"),
        Value::String(s) => s.clone(),
        Value::Number(n) => js_number(*n),
        Value::Bool(b) => b.to_string(),
        Value::Bind { bind, not } => format!("{{{}{bind}}}", if *not { "!" } else { "" }),
        Value::Token(token) => format!("{{token.{token}}}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(raw: &str, ty: Option<PropType>) -> Option<Value> {
        read_value(raw, ty).ok()
    }

    #[test]
    fn references_and_escapes_read_back_as_written() {
        for raw in ["{$.a}", "{!$.a}", "{token.space.md}", "{{x}", "plain"] {
            let value = read(raw, None);
            assert_eq!(value.as_ref().map(format_value).as_deref(), Some(raw));
        }
    }

    #[test]
    fn literals_are_typed_by_the_catalog() {
        assert_eq!(read("2", Some(PropType::Number)), Some(Value::Number(2.0)));
        assert_eq!(read("-0", Some(PropType::Number)), Some(Value::Number(0.0)));
        assert_eq!(
            read("1e400", Some(PropType::Number)),
            Some(Value::String("1e400".into()))
        );
        assert_eq!(read("2", None), Some(Value::String("2".into())));
        assert_eq!(
            read("true", Some(PropType::Boolean)),
            Some(Value::Bool(true))
        );
        assert_eq!(
            read("yes", Some(PropType::Boolean)),
            Some(Value::String("yes".into()))
        );
    }

    #[test]
    fn a_brace_that_is_not_a_reference_is_rejected() {
        assert!(read_value("{oops}", None).is_err());
        assert!(read_value("{", None).is_err());
        assert!(read_value("{!x}", None).is_err());
    }
}
