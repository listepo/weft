//! The Weft core catalog, the design token loader and the catalog version diff: the Rust port of
//! packages/catalog. Pure functions over JSON values, like weft-core.

mod core;
mod diff;
mod project;
mod settings;
mod tokens;

pub use core::{CORE_CATALOG_JSON, CatalogError, DEFAULT_TOKENS_JSON, core_catalog};
pub use diff::{CatalogChange, CatalogDiff, ChangeLevel, diff_catalogs};
pub use project::{
    MAX_TOKEN_FILES, PROJECT_FILE, Project, ProjectLoad, ProjectOptions, ReadFile,
    is_project_file_name, load_project, load_project_text,
};
pub use settings::{MAX_COUNT, project_file_schema};
pub use tokens::{Token, TokenCode, TokenProblem, Tokens, load_tokens, token_types};
