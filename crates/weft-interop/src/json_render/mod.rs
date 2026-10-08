//! json-render (`vercel-labs/json-render`, `@json-render/core` 0.21.0): `to_json_render` writes a
//! document as a spec whose catalog is the Weft catalog, and `from_json_render` reads one back
//! (SPEC §9).

mod export;
mod import;

pub use crate::MAX_SOURCE_LENGTH;
pub use export::{Exported, to_json_render};
pub use import::from_json_render;
