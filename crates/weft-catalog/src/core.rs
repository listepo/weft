//! The core catalog of SPEC §5.1. packages/catalog/catalog.json is generated from core.ts and a
//! TypeScript test keeps the two equal, so embedding the JSON keeps one source of truth.

use weft_core::Catalog;

pub const CORE_CATALOG_JSON: &str = include_str!("../../../packages/catalog/catalog.json");

/// The default design tokens of `packages/catalog`, for generators whose caller has no token file.
pub const DEFAULT_TOKENS_JSON: &str =
    include_str!("../../../packages/catalog/tokens/default.tokens.json");

#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("the embedded core catalog does not parse: {0}")]
    Embedded(#[from] serde_json::Error),
}

pub fn core_catalog() -> Result<Catalog, CatalogError> {
    Ok(serde_json::from_str(CORE_CATALOG_JSON)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_core_catalog_parses() {
        let catalog = core_catalog().unwrap();
        assert_eq!(catalog.name, "weft-core");
        assert!(catalog.components.contains_key("button"));
    }
}
