//! A call's arguments, checked against what its declaration promises. The MCP SDK checks them
//! before a call reaches Rust; cox does not, and arguments are model output, so the same bounds
//! are enforced here. Unknown members are ignored, as the SDK's object schemas ignore them.

use serde_json::{Map, Value as Json};
use weft_core::{JsonError, parse_json};

pub(crate) struct Args(Map<String, Json>);

fn invalid(message: String) -> String {
    format!("Invalid arguments: {message}")
}

pub(crate) fn too_deep() -> String {
    invalid("they nest too deep.".to_owned())
}

/// JSON values in a tree: objects, arrays and scalars alike.
fn elements(value: &Json) -> usize {
    1 + match value {
        Json::Array(items) => items.iter().map(elements).sum(),
        Json::Object(members) => members.values().map(elements).sum(),
        _ => 0,
    }
}

impl Args {
    /// `Err` is the text of a failure result.
    pub fn read(text: &str, max_elements: usize) -> Result<Args, String> {
        let value = match parse_json(if text.trim().is_empty() { "{}" } else { text }) {
            Ok(v) => v,
            Err(JsonError::TooDeep) => return Err(too_deep()),
            Err(JsonError::Syntax(e)) => return Err(invalid(format!("not JSON: {e}."))),
        };
        if elements(&value) > max_elements {
            return Err(invalid(format!("more than {max_elements} JSON values.")));
        }
        match value {
            Json::Object(members) => Ok(Args(members)),
            Json::Null => Ok(Args(Map::new())),
            _ => Err(invalid("not a JSON object.".to_owned())),
        }
    }

    /// Any JSON value, present or not (`project`, `data`).
    pub fn any(&self, name: &str) -> Option<&Json> {
        self.0.get(name)
    }

    /// A string of at most `max` UTF-16 code units, as the declaration's `maxLength` counts.
    pub fn string(&self, name: &str, max: usize) -> Result<Option<&str>, String> {
        match self.0.get(name) {
            None => Ok(None),
            Some(Json::String(s)) if s.encode_utf16().count() <= max => Ok(Some(s)),
            Some(Json::String(_)) => {
                Err(invalid(format!("{name} is longer than {max} characters.")))
            }
            Some(_) => Err(invalid(format!("{name} must be a string."))),
        }
    }

    pub fn required_string(&self, name: &str, max: usize) -> Result<&str, String> {
        self.string(name, max)?
            .ok_or_else(|| invalid(format!("{name} is required.")))
    }

    pub fn bool(&self, name: &str) -> Result<Option<bool>, String> {
        match self.0.get(name) {
            None => Ok(None),
            Some(Json::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(invalid(format!("{name} must be true or false."))),
        }
    }

    /// One of `allowed`, as the declaration's `enum` lists them.
    pub fn one_of(&self, name: &str, allowed: &[&str]) -> Result<Option<&str>, String> {
        match self.string(name, usize::MAX)? {
            Some(s) if !allowed.contains(&s) => Err(invalid(format!(
                "{name} must be one of {}.",
                allowed.join(", ")
            ))),
            other => Ok(other),
        }
    }

    pub fn required_array(&self, name: &str, max: usize) -> Result<&Json, String> {
        match self.0.get(name) {
            None => Err(invalid(format!("{name} is required."))),
            Some(v @ Json::Array(items)) if items.len() <= max => Ok(v),
            Some(Json::Array(_)) => Err(invalid(format!("{name} has more than {max} items."))),
            Some(_) => Err(invalid(format!("{name} must be an array."))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_are_held_to_their_declaration() {
        let args =
            Args::read(r#"{"markup":"😀😀","strict":1,"patches":[1,2],"x":0}"#, 100).unwrap();
        assert_eq!(args.string("markup", 4).unwrap(), Some("😀😀"));
        assert!(
            args.string("markup", 3)
                .unwrap_err()
                .contains("longer than 3")
        );
        assert!(args.bool("strict").is_err());
        assert!(args.required_array("patches", 1).is_err());
        assert!(
            args.required_string("nothing", 1)
                .unwrap_err()
                .contains("required")
        );
        assert!(args.one_of("markup", &["open"]).is_err());
    }

    #[test]
    fn the_whole_call_is_bounded() {
        assert!(Args::read("[1]", 10).is_err());
        assert!(Args::read("", 10).is_ok());
        assert!(
            Args::read(r#"{"a":[1,2,3,4]}"#, 5)
                .err()
                .unwrap()
                .contains("more than 5")
        );
    }
}
