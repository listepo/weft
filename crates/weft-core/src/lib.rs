//! Weft core: the semantic model of SPEC.md and everything that reads, writes, checks and edits
//! it. Pure functions over strings and values; no I/O, so the same crate builds for the CLI,
//! WebAssembly and native Node addons.

mod canonical;
mod context;
mod context_check;
mod context_patch;
pub mod controls;
mod data;
mod diagnostics;
mod explain;
mod json;
mod model;
mod parse;
mod patch;
mod rules;
mod sample;
mod serialize;
mod shape;
mod source;
mod syntax;
mod validate;
mod values;

pub use canonical::{canonicalize, stringify};
pub use context_patch::Author;
pub use data::{
    DataCheckOptions, DataSchema, DataSchemaProblem, Shape, check_data, check_data_json,
    compile_data_schema, data_schema_diagnostics,
};
pub use diagnostics::{
    Code, Diagnostic, Mode, Position, Severity, did_you_mean, has_errors, one_of,
};
pub use explain::{Change, ChangeKind, Readback, explain, explain_changes};
pub use json::{
    JSON_DEPTH_LIMIT, JsonError, js_number, js_round, order_keys, parse_json, to_compact,
};
pub use model::{
    Catalog, Child, ComponentDef, Content, Document, Entry, Map, Node, PropDef, PropDefault,
    PropType, SlotDef, Value, WEFT_VERSION,
};
pub use parse::{ParseOptions, ParseResult, parse, parse_partial};
pub use patch::{ApplyOptions, PatchResult, apply_patches};
pub use rules::{ARIA_ROLES, MAX_DEPTH, MODEL_ASSETS, TILT_PROPS, asset_problem};
/// The grammars of SPEC §2–§4, for importers, generators and project settings that build or check
/// names, ids and references outside the parser.
pub use rules::{
    embedded_reference, has_non_xml_char, is_action, is_binding, is_extension_name, is_id,
    is_loop_variable, is_name, is_non_xml_char, is_token,
};
/// The catalog-independent props every element takes (`hidden`, `label`, `state`, …).
pub use rules::{universal_prop, universal_props};
pub use sample::{resolve_path, text, truthy};
pub use serialize::serialize;
pub use shape::{to_document, to_value};
pub use source::{ListSource, NodeSource};
pub use validate::{ValidateOptions, validate, validate_document};
pub use values::{BadValue, format_value, read_value};
