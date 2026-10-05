//! What crosses between JavaScript and Rust: JSON text both ways, plus the options object.
//! JSON text keeps JavaScript's key order and own-property semantics on both sides (the reasons
//! are in plan.md T22), so this module is the only place that knows the wire format.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use thiserror::Error;
use weft_core::{Catalog, Document, JSON_DEPTH_LIMIT, JsonError, Mode, parse_json, to_document};

#[derive(Debug, Error)]
pub enum BindingError {
    /// The JavaScript side wrote something this module cannot read; a bug in the wrapper.
    #[error("weft-binding: malformed {what}: {source}")]
    Wire {
        what: &'static str,
        source: serde_json::Error,
    },
    #[error("weft-binding: the catalog is not a Weft catalog: {0}")]
    Catalog(serde_json::Error),
    /// A document argument that cannot be JSON (a cycle) or nests past any valid document.
    #[error("weft-binding: the document nests deeper than any valid document")]
    TooDeep,
    #[error("weft-binding: cannot write the result: {0}")]
    Output(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BindingError>;

/// The options of parse, validate and applyPatches, minus the catalog, which has its own handle.
#[derive(Default)]
pub struct Options {
    pub mode: Mode,
    pub tokens: Option<IndexMap<String, String>>,
    pub actions: Option<Vec<String>>,
    /// `parse` only: the markup may stop anywhere (SPEC §6.3).
    pub partial: bool,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOptions {
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    partial: bool,
    /// Map entries, so that the order of the TypeScript `Map` survives.
    tokens: Option<Vec<(String, String)>>,
    actions: Option<Vec<String>>,
}

impl Options {
    pub fn read(text: &str) -> Result<Options> {
        let wire: WireOptions =
            serde_json::from_str(text).map_err(|source| BindingError::Wire {
                what: "options",
                source,
            })?;
        Ok(Options {
            mode: if wire.strict {
                Mode::Strict
            } else {
                Mode::Lenient
            },
            tokens: wire.tokens.map(|entries| entries.into_iter().collect()),
            actions: wire.actions,
            partial: wire.partial,
        })
    }
}

/// The options of `load_project`.
pub struct ProjectWire {
    pub mode: Mode,
    pub prefix: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireProjectOptions {
    #[serde(default)]
    strict: bool,
    prefix: Option<String>,
}

impl ProjectWire {
    pub fn read(text: &str) -> Result<ProjectWire> {
        let wire: WireProjectOptions =
            serde_json::from_str(text).map_err(|source| BindingError::Wire {
                what: "project options",
                source,
            })?;
        Ok(ProjectWire {
            mode: if wire.strict {
                Mode::Strict
            } else {
                Mode::Lenient
            },
            prefix: wire.prefix.unwrap_or_else(|| "#".to_owned()),
        })
    }
}

/// A value nested one level past the depth limit: the core answers it the way it answers any
/// input that deep, which is what the TypeScript core did for cyclic input.
fn too_deep() -> Json {
    let mut value = Json::Array(vec![]);
    for _ in 0..=JSON_DEPTH_LIMIT {
        value = Json::Array(vec![value]);
    }
    value
}

/// Untrusted input such as a document to validate or a patch list. `None` means `JSON.stringify`
/// gave up on it (a cycle, or deeper than the JavaScript stack), which only an endlessly deep
/// value does.
pub fn read_input(text: Option<&str>) -> Result<Json> {
    match text.map(parse_json) {
        Some(Ok(value)) => Ok(value),
        None | Some(Err(JsonError::TooDeep)) => Ok(too_deep()),
        Some(Err(JsonError::Syntax(source))) => Err(BindingError::Wire {
            what: "JSON input",
            source,
        }),
    }
}

/// A document argument of serialize, canonicalize, stringify and applyPatches: typed as a
/// `Document` on the TypeScript side, so it is read leniently, as the TypeScript core did.
pub fn read_document(text: Option<&str>) -> Result<Document> {
    match text.map(parse_json) {
        Some(Ok(value)) => Ok(to_document(&value)),
        None | Some(Err(JsonError::TooDeep)) => Err(BindingError::TooDeep),
        Some(Err(JsonError::Syntax(source))) => Err(BindingError::Wire {
            what: "document",
            source,
        }),
    }
}

/// The catalog behind a `Catalog` handle, parsed once from the wrapper's JSON text.
pub fn read_catalog(text: &str) -> Result<Catalog> {
    serde_json::from_str(text).map_err(BindingError::Catalog)
}

pub fn read_list(text: &str, what: &'static str) -> Result<Vec<String>> {
    serde_json::from_str(text).map_err(|source| BindingError::Wire { what, source })
}

pub fn write<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(BindingError::Output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_lenient_without_tokens_or_actions() {
        let options = Options::read("{}").unwrap();
        assert_eq!(options.mode, Mode::Lenient);
        assert!(options.tokens.is_none() && options.actions.is_none());
    }

    #[test]
    fn token_entries_keep_their_order() {
        let options = Options::read(r#"{"strict":true,"tokens":[["b","x"],["a","y"]]}"#).unwrap();
        assert_eq!(options.mode, Mode::Strict);
        let keys: Vec<_> = options.tokens.unwrap().into_keys().collect();
        assert_eq!(keys, ["b", "a"]);
    }

    #[test]
    fn unknown_option_fields_are_a_wire_error() {
        assert!(matches!(
            Options::read(r#"{"catalog":{}}"#),
            Err(BindingError::Wire { .. })
        ));
    }

    #[test]
    fn input_json_cannot_write_reads_as_too_deep() {
        let deep = read_input(None).unwrap();
        let text = deep.to_string();
        assert_eq!(text.matches('[').count(), JSON_DEPTH_LIMIT + 2);
    }

    #[test]
    fn a_document_json_cannot_write_is_an_error() {
        assert!(matches!(read_document(None), Err(BindingError::TooDeep)));
    }

    #[test]
    fn a_document_with_missing_members_reads_as_empty() {
        let doc = read_document(Some(r#"{"root":{"kind":"screen"}}"#)).unwrap();
        assert_eq!(doc.weft, "");
        assert_eq!(doc.root.kind, "screen");
    }
}
