//! WebAssembly exports of the Weft core and catalog, one of the two engines behind `@weft/core`
//! and `@weft/catalog` (`packages/core/src/wasm.ts` loads it). The surface is thin: each export
//! reads JSON text, calls one function of `weft-binding`, and returns JSON text the TypeScript
//! wrapper parses. `weft-node` exports the same names; `packages/core/test/engines.test.ts` keeps
//! the two lists equal.

use wasm_bindgen::prelude::*;
#[cfg(feature = "web")]
use weft_binding::web;
use weft_binding::{api, boundary};

/// A catalog parsed once and reused across calls; the wrapper keeps one per catalog object.
#[wasm_bindgen]
pub struct Catalog(weft_core::Catalog);

#[wasm_bindgen]
impl Catalog {
    #[wasm_bindgen(constructor)]
    pub fn new(json: &str) -> Result<Catalog, JsError> {
        Ok(Catalog(boundary::read_catalog(json)?))
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

    /// `fromDom` of `@weft/from-aria`.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = fromDom)]
    pub fn from_dom(&self, html: &str) -> Result<String, JsError> {
        Ok(web::from_dom(html, &self.0)?)
    }

    /// The role tree builder behind `fromAriaSnapshot`.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = buildDocument)]
    pub fn build_document(&self, sems: &str, reserved: &str) -> Result<String, JsError> {
        Ok(web::build(sems, reserved, &self.0)?)
    }

    /// `toJsx` of `@weft/to-jsx`.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = toJsx)]
    pub fn to_jsx(&self, document: Option<String>, options: &str) -> Result<String, JsError> {
        Ok(web::to_jsx(document.as_deref(), options, &self.0)?)
    }

    /// A static HTML page from a screen: `{code}` or `{diagnostics}`.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = toHtml)]
    pub fn to_html(
        &self,
        document: Option<String>,
        tokens: Option<String>,
        options: &str,
    ) -> Result<String, JsError> {
        Ok(web::to_html(
            document.as_deref(),
            tokens.as_deref(),
            options,
            &self.0,
        )?)
    }

    /// A screen from an HTML page, with its losses.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = importHtml)]
    pub fn import_html(&self, html: &str, tokens: Option<String>) -> Result<String, JsError> {
        Ok(web::import_html(html, tokens.as_deref(), &self.0)?)
    }

    /// A screen from a React or SolidJS component (TSX when `typescript`), with its losses.
    #[cfg(feature = "web")]
    #[wasm_bindgen(js_name = importJsx)]
    pub fn import_jsx(
        &self,
        source: &str,
        typescript: bool,
        tokens: Option<String>,
    ) -> Result<String, JsError> {
        Ok(web::import_jsx(
            source,
            typescript,
            tokens.as_deref(),
            &self.0,
        )?)
    }

    #[wasm_bindgen(js_name = checkData)]
    pub fn check_data(
        &self,
        input: Option<String>,
        schema: Option<String>,
        sources: Option<String>,
    ) -> Result<String, JsError> {
        Ok(api::check_data_input(
            input.as_deref(),
            &self.0,
            schema.as_deref(),
            sources.as_deref(),
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

#[wasm_bindgen(js_name = compileDataSchema)]
pub fn compile_data_schema(schema: Option<String>) -> Result<String, JsError> {
    Ok(api::compile_data(schema.as_deref())?)
}

#[wasm_bindgen(js_name = projectFiles)]
pub fn project_files(text: &str) -> Result<String, JsError> {
    Ok(api::project_files(text)?)
}

#[wasm_bindgen(js_name = loadProject)]
pub fn load_project(text: &str, files: Option<String>, options: &str) -> Result<String, JsError> {
    Ok(api::load_project(text, files.as_deref(), options)?)
}

#[wasm_bindgen(js_name = readValue)]
pub fn read_value(raw: &str) -> Result<String, JsError> {
    Ok(api::read_attribute_value(raw)?)
}

#[wasm_bindgen(js_name = formatValue)]
pub fn format_value(value: Option<String>) -> Result<String, JsError> {
    Ok(api::format_attribute_value(value.as_deref())?)
}

#[cfg(feature = "web")]
#[wasm_bindgen(js_name = importFailure)]
pub fn import_failure(
    message: &str,
    expected: &str,
    got: Option<String>,
) -> Result<String, JsError> {
    Ok(web::import_failure(message, expected, got.as_deref())?)
}

#[cfg(feature = "web")]
#[wasm_bindgen(js_name = instanceId)]
pub fn instance_id(raw: &str) -> Option<String> {
    web::instance_id(raw)
}

#[cfg(feature = "web")]
#[wasm_bindgen(js_name = webTables)]
pub fn web_tables() -> Result<String, JsError> {
    Ok(web::tables()?)
}

#[cfg(feature = "web")]
#[wasm_bindgen(js_name = jsxTables)]
pub fn jsx_tables() -> Result<String, JsError> {
    Ok(web::jsx_tables()?)
}
