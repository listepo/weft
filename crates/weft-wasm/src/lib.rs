//! WebAssembly exports of the Weft core and catalog, the engine behind `@weft/core` and
//! `@weft/catalog` (`packages/core/src/wasm.ts` loads it). The surface is thin: each export reads
//! JSON text, calls one function of `api`, and returns JSON text the TypeScript wrapper parses.
//! A `None` JSON argument means `JSON.stringify` could not write the value (see `boundary`).

mod api;
mod boundary;
mod sources;

use wasm_bindgen::prelude::*;

pub use boundary::BindingError;

/// A catalog parsed once and reused across calls; the wrapper keeps one per catalog object.
#[wasm_bindgen]
pub struct Catalog(weft_core::Catalog);

#[wasm_bindgen]
impl Catalog {
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<Catalog, JsError> {
        Ok(Catalog(
            serde_json::from_str(json).map_err(BindingError::Catalog)?,
        ))
    }

    pub fn parse(&self, markup: &str, options: &str) -> Result<String, JsError> {
        Ok(api::parse_markup(markup, Some(&self.0), options)?)
    }

    pub fn validate(
        &self,
        input: Option<String>,
        options: &str,
        sources: Option<String>,
    ) -> Result<String, JsError> {
        Ok(api::validate_json(
            input.as_deref(),
            Some(&self.0),
            options,
            sources.as_deref(),
        )?)
    }

    #[wasm_bindgen(js_name = applyPatches)]
    pub fn apply_patches(
        &self,
        document: Option<String>,
        patches: Option<String>,
        options: &str,
    ) -> Result<String, JsError> {
        Ok(api::apply(
            document.as_deref(),
            patches.as_deref(),
            &self.0,
            options,
        )?)
    }
}

/// `parse` without a catalog: the syntax layer only.
#[wasm_bindgen]
pub fn parse(markup: &str, options: &str) -> Result<String, JsError> {
    Ok(api::parse_markup(markup, None, options)?)
}

#[wasm_bindgen]
pub fn serialize(document: Option<String>) -> Result<String, JsError> {
    Ok(api::serialize_document(document.as_deref())?)
}

#[wasm_bindgen]
pub fn stringify(document: Option<String>) -> Result<String, JsError> {
    Ok(api::stringify_document(document.as_deref())?)
}

#[wasm_bindgen]
pub fn canonicalize(document: Option<String>) -> Result<String, JsError> {
    Ok(api::canonicalize_document(document.as_deref())?)
}

#[wasm_bindgen(js_name = didYouMean)]
pub fn did_you_mean(word: &str, candidates: &str) -> Result<Option<String>, JsError> {
    Ok(api::suggest(word, candidates)?)
}

#[wasm_bindgen(js_name = loadTokens)]
pub fn load_tokens(json: Option<String>) -> Result<String, JsError> {
    Ok(api::load_tokens(json.as_deref())?)
}

#[wasm_bindgen(js_name = diffCatalogs)]
pub fn diff_catalogs(previous: Option<String>, next: Option<String>) -> Result<String, JsError> {
    Ok(api::diff_catalogs(previous.as_deref(), next.as_deref())?)
}
