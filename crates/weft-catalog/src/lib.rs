//! The Weft core catalog, the design token loader and the catalog version diff: the Rust port of
//! packages/catalog. Pure functions over JSON values, like weft-core.

mod core;
mod diff;
mod document_schema;
mod project;
mod resolver;
mod settings;
mod tokens;
mod version;

pub use core::{CORE_CATALOG_JSON, CatalogError, DEFAULT_TOKENS_JSON, core_catalog};
pub use diff::{CatalogChange, CatalogDiff, ChangeLevel, diff_catalogs};
pub use document_schema::{DocumentSchemaOptions, document_schema};
pub use project::{
    CatalogSource, KindSource, MAX_CATALOGS, MAX_TOKEN_FILES, PROJECT_FILE, Project, ProjectLoad,
    ProjectOptions, ReadFile, is_project_file_name, load_project, load_project_text,
    merge_catalogs,
};
pub use resolver::{
    Appearance, MAX_CONTEXTS, RESOLVER_VERSION, TokenModifier, appearance, is_resolver,
};
pub use settings::{MAX_COUNT, project_file_schema};
pub use tokens::{
    MATERIAL, MATERIAL_EXTENSION, MAX_BLUR_PX, Token, TokenCode, TokenProblem, Tokens,
    composite_part, font_weight, load_tokens, material_parts, token_types,
};
pub use version::{VersionChange, VersionCheck, VersionLevel, check_catalogs, check_documents};
