//! Weft core: the semantic model of SPEC.md and everything that reads, writes, checks and edits
//! it. Pure functions over strings and values; no I/O, so the same crate builds for the CLI,
//! WebAssembly and native Node addons.

mod canonical;
mod diagnostics;
mod json;
mod model;
mod parse;
mod patch;
mod rules;
mod serialize;
mod shape;
mod source;
mod syntax;
mod validate;
mod values;

pub use canonical::{canonicalize, stringify};
pub use diagnostics::{Code, Diagnostic, Mode, Position, Severity, did_you_mean, has_errors};
pub use json::{JsonError, parse_json};
pub use model::{
    Catalog, Child, ComponentDef, Content, Document, Map, Node, PropDef, PropDefault, PropType,
    SlotDef, Value, WEFT_VERSION,
};
pub use parse::{ParseOptions, ParseResult, parse};
pub use patch::{ApplyOptions, PatchResult, apply_patches};
pub use rules::{ARIA_ROLES, MAX_DEPTH};
pub use serialize::serialize;
pub use source::{ListSource, NodeSource};
pub use validate::{ValidateOptions, validate, validate_document};
