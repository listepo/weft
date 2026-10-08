//! Weft to and from the other agent UI formats (SPEC §9): A2UI messages and json-render specs.
//! Both are data constrained by a catalog, so documents convert by value and never run anything;
//! what a format cannot say comes back as the losses every Weft importer reports.

pub mod a2ui;
mod ids;
pub mod json_render;
mod paths;

use serde_json::{Value as Json, json};
pub use weft_import::{ImportResult, Loss, LossKind};
use weft_import::{Sem, empty_result, limit_reached};

/// Longest input either importer reads, in bytes.
pub const MAX_SOURCE_LENGTH: usize = 2_000_000;

/// The empty result with `W602` for input longer than `MAX_SOURCE_LENGTH`, which is not read.
pub(crate) fn too_long(text: &str) -> Option<ImportResult> {
    (text.len() > MAX_SOURCE_LENGTH).then(|| {
        let mut diagnostics = Vec::new();
        let what = format!("is longer than {MAX_SOURCE_LENGTH} bytes");
        limit_reached(&mut diagnostics, "#", &what);
        empty_result(diagnostics)
    })
}

/// An element of a named Weft kind. The role is unused once the kind is named, except that role
/// `text` is a bare text run, which would not carry the element's id.
pub(crate) fn sem(kind: &str, name: impl Into<String>) -> Sem {
    let mut s = Sem::new(if kind == "text" { "paragraph" } else { kind }, name);
    s.kind = Some(kind.to_owned());
    s
}

/// A whole number as a JSON integer, as the formats write one.
pub(crate) fn number(n: f64) -> Json {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}
