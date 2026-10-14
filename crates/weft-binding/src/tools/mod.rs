//! The Weft agent tools (`weft_primer` … `weft_context`) as one Rust API that every host calls:
//! the MCP server of `packages/mcp` and the cox plugin of `plugins/cox` (plan.md T106).

pub mod decls;
pub mod settings;

pub use settings::{ContextPolicy, Limits, Settings};
