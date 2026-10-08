//! A2UI v0.9 (basic catalog): `to_a2ui` writes the messages of a document and `from_a2ui` reads
//! them back (SPEC §9).

mod export;
mod import;

pub use crate::MAX_SOURCE_LENGTH;
pub use export::{CATALOG_ID, Exported, to_a2ui};
pub use import::from_a2ui;
