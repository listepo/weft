//! What every Weft importer shares (SPEC §9, "From a running UI" and "From source code"): the loss
//! table, the import limits, generated ids, literal text, stand-ins for required props, and the
//! role tree (`Sem`) that HTML, accessibility snapshots and other sources are reduced to before the
//! catalog is consulted. Kept apart from any parser so that importers of other platforms reuse it
//! without pulling in a web parser.

mod build;
mod ids;
mod kinds;
mod limits;
mod loss;
mod props;
mod sem;
mod text;

pub use build::{BuildOptions, Built, build_document};
pub use ids::IdState;
pub use kinds::{DISSOLVED_ROLES, KindIndex, ROLE_REFINEMENTS, Refinement, Resolved};
pub use limits::{MAX_DEPTH, MAX_NODES, limit_reached};
pub use loss::{ImportResult, Loss, LossKind, Losses, empty_result};
pub use props::{Scalar, coerce, fill_required};
pub use sem::{Note, Sem};
pub use text::{
    clean, is_js_space, js_length, js_number_from, js_prefix, js_trim, literal, slug, squash,
};
