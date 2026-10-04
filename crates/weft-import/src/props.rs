//! Typing imported values by the catalog, and stand-ins for required props the input lacks.

use serde::{Deserialize, Serialize};
use weft_core::{ComponentDef, Map, PropDef, PropDefault, PropType, Value, js_number, to_compact};

use crate::loss::{LossKind, Losses};
use crate::text::{js_number_from, js_trim, slug};

/// A value as a source states it, before the catalog types it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Bool(bool),
    Number(f64),
    String(String),
}

impl Scalar {
    /// ECMAScript `String(value)`.
    pub fn text(&self) -> String {
        match self {
            Scalar::Bool(b) => b.to_string(),
            Scalar::Number(n) => js_number(*n),
            Scalar::String(s) => s.clone(),
        }
    }
}

impl From<&str> for Scalar {
    fn from(s: &str) -> Self {
        Scalar::String(s.to_owned())
    }
}

/// The value of `raw` for a prop of this definition, or `None` when it is not a valid value.
/// A rendered UI shows resolved values, so token props never take one.
pub fn coerce(def: &PropDef, raw: &Scalar) -> Option<Value> {
    match def.kind {
        PropType::String => Some(Value::String(raw.text())),
        PropType::Boolean => match raw {
            Scalar::Bool(b) => Some(Value::Bool(*b)),
            Scalar::String(s) if s == "true" => Some(Value::Bool(true)),
            Scalar::String(s) if s == "false" => Some(Value::Bool(false)),
            _ => None,
        },
        PropType::Number => {
            let n = match raw {
                Scalar::Number(n) => *n,
                Scalar::Bool(b) => f64::from(u8::from(*b)),
                Scalar::String(s) if js_trim(s).is_empty() => f64::NAN,
                Scalar::String(s) => js_number_from(s),
            };
            if !n.is_finite() || (def.integer == Some(true) && n.fract() != 0.0) {
                return None;
            }
            if def.min.is_some_and(|min| n < min) || def.max.is_some_and(|max| n > max) {
                return None;
            }
            Some(Value::Number(n))
        }
        PropType::Enum => match raw {
            Scalar::String(s) if def.values.as_ref().is_some_and(|v| v.contains(s)) => {
                Some(Value::String(s.clone()))
            }
            _ => None,
        },
        PropType::Token => None,
    }
}

fn default_value(d: &PropDefault) -> Value {
    match d {
        PropDefault::Bool(b) => Value::Bool(*b),
        PropDefault::Number(n) => Value::Number(*n),
        PropDefault::String(s) => Value::String(s.clone()),
    }
}

/// Stand-ins for required props the input has no value for, each one a `values` loss. `name` is
/// the element's text or name, used for a value its parent selects by; `parent` is the parent's
/// component (absent at the root and under extensions).
pub fn fill_required(
    losses: &mut Losses,
    props: &mut Map<Value>,
    def: &ComponentDef,
    parent: Option<&ComponentDef>,
    root: bool,
    name: &str,
    path: &str,
) {
    let Some(defs) = &def.props else {
        return;
    };
    for (prop, pd) in defs {
        if pd.required != Some(true) || props.contains_key(prop) {
            continue;
        }
        // The root's `weft` is `Document.weft`, never a prop (SPEC §3).
        if prop == "weft" && root {
            continue;
        }
        // A value the parent selects by (radio and option `value`) must tell the children apart.
        let selects = parent
            .and_then(|p| p.prop(prop))
            .is_some_and(|p| p.writable == Some(true));
        let value = match pd.kind {
            PropType::Number => pd
                .default
                .as_ref()
                .map_or_else(|| Value::Number(pd.min.unwrap_or(0.0)), default_value),
            PropType::Boolean => pd
                .default
                .as_ref()
                .map_or(Value::Bool(false), default_value),
            PropType::Enum => pd.default.as_ref().map_or_else(
                || {
                    Value::String(
                        pd.values
                            .as_ref()
                            .and_then(|v| v.first())
                            .cloned()
                            .unwrap_or_default(),
                    )
                },
                default_value,
            ),
            PropType::String | PropType::Token => {
                let s = slug(name);
                Value::String(if !selects {
                    String::new()
                } else if s.is_empty() {
                    prop.clone()
                } else {
                    s
                })
            }
        };
        losses.push(
            LossKind::Values,
            path,
            format!(
                "required {prop} is not in the input; {} stands in",
                to_compact(&value)
            ),
        );
        props.insert(prop.clone(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_typed_by_the_catalog() {
        let mut n = PropDef::new("", PropType::Number);
        n.min = Some(1.0);
        n.max = Some(6.0);
        n.integer = Some(true);
        assert_eq!(coerce(&n, &"3".into()), Some(Value::Number(3.0)));
        assert_eq!(coerce(&n, &Scalar::Bool(true)), Some(Value::Number(1.0)));
        assert_eq!(coerce(&n, &"2.5".into()), None);
        assert_eq!(coerce(&n, &" ".into()), None);
        let b = PropDef::new("", PropType::Boolean);
        assert_eq!(coerce(&b, &"true".into()), Some(Value::Bool(true)));
        assert_eq!(coerce(&b, &"yes".into()), None);
        let s = PropDef::new("", PropType::String);
        assert_eq!(
            coerce(&s, &Scalar::Number(1e21)),
            Some(Value::String("1e+21".into()))
        );
        assert_eq!(
            coerce(&PropDef::new("", PropType::Token), &"x".into()),
            None
        );
    }
}
