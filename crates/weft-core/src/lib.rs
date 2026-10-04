//! Weft core: the semantic model of SPEC.md and everything that reads, writes, checks and edits
//! it. Pure functions over strings and values; no I/O, so the same crate builds for the CLI,
//! WebAssembly and native Node addons.

mod canonical;
mod data;
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
pub use data::{
    DataCheckOptions, DataSchema, DataSchemaProblem, Shape, check_data, check_data_json,
    compile_data_schema,
};
pub use diagnostics::{
    Code, Diagnostic, Mode, Position, Severity, did_you_mean, has_errors, one_of,
};
pub use json::{JsonError, order_keys, parse_json, to_compact};
pub use model::{
    Catalog, Child, ComponentDef, Content, Document, Map, Node, PropDef, PropDefault, PropType,
    SlotDef, Value, WEFT_VERSION,
};
pub use parse::{ParseOptions, ParseResult, parse};
pub use patch::{ApplyOptions, PatchResult, apply_patches};
pub use rules::{ARIA_ROLES, MAX_DEPTH, is_action};
pub use serialize::serialize;
pub use source::{ListSource, NodeSource};
pub use validate::{ValidateOptions, validate, validate_document};
