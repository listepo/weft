//! json-render (`vercel-labs/json-render`, `@json-render/core` 0.21.0): `to_json_render` writes a
//! document as a spec whose catalog is the Weft catalog (SPEC §9).

mod export;

pub use export::{Exported, to_json_render};
