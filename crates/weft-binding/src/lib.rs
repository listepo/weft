//! The functions behind `@weft/core` and `@weft/catalog`, as plain Rust over JSON text. Two thin
//! crates adapt them to a JavaScript host: `weft-wasm` (wasm-bindgen) and `weft-node` (napi-rs).
//! Keeping them here means a fix lands once and both engines answer alike. A `None` JSON argument
//! means `JSON.stringify` could not write the value (see `boundary`).

pub mod api;
pub mod boundary;
mod sources;
#[cfg(feature = "web")]
pub mod web;

pub use boundary::{BindingError, Result};
