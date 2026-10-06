//! Weft to and from the other agent UI formats (SPEC §9): A2UI messages and json-render specs.
//! Both are data constrained by a catalog, so documents convert by value and never run anything;
//! what a format cannot say comes back as the losses every Weft importer reports.

pub mod a2ui;
mod paths;

pub use weft_import::{ImportResult, Loss, LossKind};
