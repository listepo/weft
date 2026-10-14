//! What every Weft importer shares (SPEC §9, "From a running UI" and "From source code"): the loss
//! table, the import limits, generated ids, literal text, stand-ins for required props, the
//! Trust URL allowlist, and the role tree (`Sem`) that HTML, accessibility snapshots and other
//! sources are reduced to before the catalog is consulted, and the `weft:source` comment that
//! generators leave for their importers (`provenance`), and the repair that makes a tree a source
//! reader built valid in lenient mode (`repair`). Kept apart from any parser so that
//! importers and generators of other platforms reuse it without pulling in a web parser.

mod build;
mod cem;
mod ids;
mod kinds;
mod limits;
mod loss;
mod props;
pub mod provenance;
mod repair;
mod sem;
mod text;

pub use build::{BuildOptions, Built, build_document};
pub use cem::{CemImport, CemOptions, MAX_KINDS, MAX_MANIFEST_LENGTH, MAX_MEMBERS, import_cem};
pub use ids::IdState;
pub use kinds::{
    DISSOLVED_ROLES, KindIndex, ROLE_DEFAULTS, ROLE_REFINEMENTS, Refinement, Resolved, dissolved,
};
pub use limits::{MAX_DEPTH, MAX_NODES, limit_reached, on_reader_stack};
pub use loss::{ImportResult, Loss, LossKind, Losses, empty_result};
pub use props::{Scalar, coerce, fill_required};
pub use repair::{element_path, repair};
pub use sem::{Note, Sem};
pub use text::{
    clean, is_js_space, js_length, js_number_from, js_prefix, js_trim, literal, slug, squash,
};
pub use weft_core::{safe_absolute_url, safe_url};
