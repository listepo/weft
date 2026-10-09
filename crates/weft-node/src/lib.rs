//! Native Node and Bun exports of the Weft core and catalog, the second engine behind
//! `@weft/core` and `@weft/catalog` (`packages/core/src/native.ts` loads it, and falls back to the
//! WebAssembly module when it cannot). Each export reads JSON text, calls one function of
//! `weft-binding`, and returns JSON text, exactly as `weft-wasm` does; napi-rs turns the snake_case
//! names into the camelCase ones the wrapper calls, and
//! `packages/core/test/engines.test.ts` keeps both export lists equal.

use napi::{Error, Result};
use napi_derive::napi;
use weft_binding::{BindingError, api, boundary, web};

/// A thrown error carries the message, as `JsError` does in the WebAssembly build.
fn js<T>(result: std::result::Result<T, BindingError>) -> Result<T> {
    result.map_err(|e| Error::from_reason(e.to_string()))
}

/// A catalog parsed once and reused across calls; the wrapper keeps one per catalog object.
#[napi]
pub struct Catalog {
    inner: weft_core::Catalog,
}

#[napi]
impl Catalog {
    #[napi(constructor)]
    pub fn new(json: String) -> Result<Self> {
        Ok(Self {
            inner: js(boundary::read_catalog(&json))?,
        })
    }

    #[napi]
    pub fn parse(&self, markup: String, options: String) -> Result<String> {
        js(api::parse_markup(&markup, Some(&self.inner), &options))
    }

    #[napi]
    pub fn validate(
        &self,
        input: Option<String>,
        options: String,
        sources: Option<String>,
    ) -> Result<String> {
        js(api::validate_json(
            input.as_deref(),
            Some(&self.inner),
            &options,
            sources.as_deref(),
        ))
    }

    /// The document with its fragment uses expanded (SPEC §10.7), as `{document, diagnostics}`.
    #[napi]
    pub fn expand(&self, document: Option<String>) -> Result<String> {
        js(api::expand_document(document.as_deref(), &self.inner))
    }

    #[napi]
    pub fn apply_patches(
        &self,
        document: Option<String>,
        patches: Option<String>,
        options: String,
    ) -> Result<String> {
        js(api::apply(
            document.as_deref(),
            patches.as_deref(),
            &self.inner,
            &options,
        ))
    }

    /// `fromDom` of `@weft/from-aria`.
    #[napi]
    pub fn from_dom(&self, html: String) -> Result<String> {
        js(web::from_dom(&html, &self.inner))
    }

    /// The role tree builder behind `fromAriaSnapshot`.
    #[napi]
    pub fn build_document(&self, sems: String, reserved: String) -> Result<String> {
        js(web::build(&sems, &reserved, &self.inner))
    }

    /// `toJsx` of `@weft/to-jsx`.
    #[napi]
    pub fn to_jsx(&self, document: Option<String>, options: String) -> Result<String> {
        js(web::to_jsx(document.as_deref(), &options, &self.inner))
    }

    /// A static HTML page from a screen: `{code}` or `{diagnostics}`.
    #[napi]
    pub fn to_html(
        &self,
        document: Option<String>,
        tokens: Option<String>,
        options: String,
    ) -> Result<String> {
        js(web::to_html(
            document.as_deref(),
            tokens.as_deref(),
            &options,
            &self.inner,
        ))
    }

    /// A screen from an HTML page, with its losses.
    #[napi]
    pub fn import_html(&self, html: String, tokens: Option<String>) -> Result<String> {
        js(web::import_html(&html, tokens.as_deref(), &self.inner))
    }

    /// A screen from a React or SolidJS component (TSX when `typescript`), with its losses.
    #[napi]
    pub fn import_jsx(
        &self,
        source: String,
        typescript: bool,
        tokens: Option<String>,
    ) -> Result<String> {
        js(web::import_jsx(
            &source,
            typescript,
            tokens.as_deref(),
            &self.inner,
        ))
    }

    /// A catalog from a Custom Elements Manifest that extends this one, with its losses.
    #[napi]
    pub fn import_cem(&self, manifest: String, options: String) -> Result<String> {
        js(web::import_cem(&manifest, &options, &self.inner))
    }

    /// The JSON Schema of the canonical documents this catalog admits, as compact JSON text.
    #[napi]
    pub fn document_schema(&self) -> Result<String> {
        js(web::document_schema(&self.inner))
    }

    #[napi]
    pub fn check_data(
        &self,
        input: Option<String>,
        schema: Option<String>,
        sources: Option<String>,
    ) -> Result<String> {
        js(api::check_data_input(
            input.as_deref(),
            &self.inner,
            schema.as_deref(),
            sources.as_deref(),
        ))
    }
}

/// `parse` without a catalog: the syntax layer only.
#[napi]
pub fn parse(markup: String, options: String) -> Result<String> {
    js(api::parse_markup(&markup, None, &options))
}

#[napi]
pub fn serialize(document: Option<String>) -> Result<String> {
    js(api::serialize_document(document.as_deref()))
}

#[napi]
pub fn stringify(document: Option<String>) -> Result<String> {
    js(api::stringify_document(document.as_deref()))
}

#[napi]
pub fn canonicalize(document: Option<String>) -> Result<String> {
    js(api::canonicalize_document(document.as_deref()))
}

#[napi]
pub fn did_you_mean(word: String, candidates: String) -> Result<Option<String>> {
    js(api::suggest(&word, &candidates))
}

#[napi]
pub fn load_tokens(json: Option<String>) -> Result<String> {
    js(api::load_tokens(json.as_deref()))
}

#[napi]
pub fn diff_catalogs(previous: Option<String>, next: Option<String>) -> Result<String> {
    js(api::diff_catalogs(previous.as_deref(), next.as_deref()))
}

#[napi]
pub fn compile_data_schema(schema: Option<String>) -> Result<String> {
    js(api::compile_data(schema.as_deref()))
}

#[napi]
pub fn project_files(text: String, files: Option<String>) -> Result<String> {
    js(api::project_files(&text, files.as_deref()))
}

#[napi]
pub fn load_project(text: String, files: Option<String>, options: String) -> Result<String> {
    js(api::load_project(&text, files.as_deref(), &options))
}

#[napi]
pub fn read_value(raw: String) -> Result<String> {
    js(api::read_attribute_value(&raw))
}

#[napi]
pub fn format_value(value: Option<String>) -> Result<String> {
    js(api::format_attribute_value(value.as_deref()))
}

#[napi]
pub fn import_failure(message: String, expected: String, got: Option<String>) -> Result<String> {
    js(web::import_failure(&message, &expected, got.as_deref()))
}

#[napi]
pub fn instance_id(raw: String) -> Option<String> {
    web::instance_id(&raw)
}

#[napi]
pub fn web_tables() -> Result<String> {
    js(web::tables())
}

#[napi]
pub fn jsx_tables() -> Result<String> {
    js(web::jsx_tables())
}
