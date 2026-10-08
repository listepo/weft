//! Weft to and from the other agent UI formats (SPEC §9): A2UI messages and json-render specs.
//! Both are data constrained by a catalog, so documents convert by value and never run anything;
//! what a format cannot say comes back as the losses every Weft importer reports.

pub mod a2ui;
mod ids;
pub mod json_render;
mod paths;

use serde_json::{Value as Json, json};
pub use weft_import::{ImportResult, Loss, LossKind};

/// A whole number as a JSON integer, as the formats write one.
pub(crate) fn number(n: f64) -> Json {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}
