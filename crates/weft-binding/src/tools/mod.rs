//! The Weft agent tools (`weft_primer` … `weft_context`) as one Rust API that every host calls:
//! the MCP server of `packages/mcp` and the cox plugin of `plugins/cox` (plan.md T106).

#[allow(dead_code)] // Used by the tool calls of T106.1.3.
mod args;
pub mod decls;
pub mod host;
pub mod settings;

pub use host::{Context, Outcome, RenderRequest};
pub use settings::{ContextPolicy, Limits, Settings};
