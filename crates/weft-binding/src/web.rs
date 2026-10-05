//! The web importers and generators of `weft-web`, in the `web` feature only: the WebAssembly
//! module behind `@weft/core/web` has it, so core-only consumers do not carry the HTML and JSX
//! parsers; the native addon always does.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use weft_catalog::{DEFAULT_TOKENS_JSON, Token, load_tokens};
use weft_core::to_document;
use weft_core::{Catalog, Code, Diagnostic};
use weft_import::{
    BuildOptions, DISSOLVED_ROLES, ROLE_REFINEMENTS, Sem, build_document, empty_result,
};
use weft_web::{Framework, HtmlOptions, ImportOptions, JsxOptions};

use crate::boundary::{BindingError, Result, read_input, read_list, write};

/// `instanceId` of `@weft/from-aria`: the id a component instance name stands for.
pub fn instance_id(raw: &str) -> Option<String> {
    weft_web::instance_id(raw)
}

/// `fromDom` of `@weft/from-aria`.
pub fn from_dom(html: &str, catalog: &Catalog) -> Result<String> {
    write(&weft_web::from_dom(html, catalog))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Built<'a> {
    #[serde(flatten)]
    result: &'a weft_import::ImportResult,
    root_path: &'a str,
}

/// The role tree builder behind `fromAriaSnapshot`: `{document, losses, diagnostics, rootPath}`.
pub fn build(sems: &str, reserved: &str, catalog: &Catalog) -> Result<String> {
    let sems: Vec<Sem> = serde_json::from_str(sems).map_err(|source| BindingError::Wire {
        what: "role tree",
        source,
    })?;
    let built = build_document(
        &sems,
        BuildOptions {
            catalog,
            reserved: read_list(reserved, "reserved ids")?,
            diagnostics: Vec::new(),
        },
    );
    write(&Built {
        result: &built.result,
        root_path: &built.root_path,
    })
}

/// The result of an import that could not start (W601): input of the wrong type, or a catalog
/// the importer cannot use.
pub fn import_failure(message: &str, expected: &str, got: Option<&str>) -> Result<String> {
    let diagnostic =
        Diagnostic::new(Code::W601, "#", message, expected).got_opt(got.map(str::to_owned));
    write(&empty_result(vec![diagnostic]))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Tables {
    implicit_roles: IndexMap<&'static str, &'static str>,
    input_roles: IndexMap<&'static str, &'static str>,
    max_html_length: usize,
    dissolved_roles: &'static [&'static str],
    role_refinements: IndexMap<&'static str, Refined>,
}

#[derive(Serialize)]
struct Refined {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    props: Option<IndexMap<&'static str, &'static str>>,
}

/// The constants `@weft/from-aria` exports, read from the importer instead of copied.
pub fn tables() -> Result<String> {
    write(&Tables {
        implicit_roles: weft_web::IMPLICIT_ROLES.iter().copied().collect(),
        input_roles: weft_web::INPUT_ROLES.iter().copied().collect(),
        max_html_length: weft_web::MAX_HTML_LENGTH,
        dissolved_roles: DISSOLVED_ROLES,
        role_refinements: ROLE_REFINEMENTS
            .iter()
            .map(|r| {
                let props = (!r.props.is_empty()).then(|| r.props.iter().copied().collect());
                (
                    r.role,
                    Refined {
                        kind: r.kind,
                        props,
                    },
                )
            })
            .collect(),
    })
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FrameworkWire {
    #[default]
    React,
    Solid,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JsxWire {
    component_name: Option<String>,
    #[serde(default)]
    framework: FrameworkWire,
    #[serde(default)]
    typescript: bool,
    #[serde(default)]
    source: bool,
}

/// What a generator returns: the code, or why it refused the options.
#[derive(Serialize)]
#[serde(untagged)]
enum Generated {
    Code { code: String },
    Error { error: String },
}

/// `toJsx` of `@weft/to-jsx`: `{code}`, or `{error}` for a component name it cannot use.
pub fn to_jsx(document: Option<&str>, options: &str, catalog: &Catalog) -> Result<String> {
    let document = read_input(document)?;
    let wire: JsxWire = serde_json::from_str(options).map_err(|source| BindingError::Wire {
        what: "JSX options",
        source,
    })?;
    let options = JsxOptions {
        catalog,
        component_name: wire.component_name.as_deref(),
        framework: match wire.framework {
            FrameworkWire::React => Framework::React,
            FrameworkWire::Solid => Framework::Solid,
        },
        typescript: wire.typescript,
        source: wire.source,
    };
    write(&match weft_web::to_jsx(&document, &options) {
        Ok(code) => Generated::Code { code },
        Err(e) => Generated::Error {
            error: e.to_string(),
        },
    })
}

/// A DTCG token tree as JSON text, resolved; the default tokens when absent. Token problems are
/// the caller's to report (`loadTokens`): the generators and importers use what resolves.
fn tokens(json: Option<&str>) -> Result<IndexMap<String, Token>> {
    let json = match json {
        Some(text) => read_input(Some(text))?,
        None => read_input(Some(DEFAULT_TOKENS_JSON))?,
    };
    Ok(load_tokens(&json).tokens)
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct HtmlWire {
    #[serde(default)]
    source: bool,
}

/// What the HTML generator returns: the page, or the diagnostics of a screen it refused.
#[derive(Serialize)]
#[serde(untagged)]
enum Page {
    Code { code: String },
    Invalid { diagnostics: Vec<Diagnostic> },
}

/// `toHtml`: `{code}`, or `{diagnostics}` when the screen is not strictly valid.
pub fn to_html(
    document: Option<&str>,
    tokens_json: Option<&str>,
    options: &str,
    catalog: &Catalog,
) -> Result<String> {
    let json = read_input(document)?;
    let wire: HtmlWire = serde_json::from_str(options).map_err(|source| BindingError::Wire {
        what: "HTML options",
        source,
    })?;
    let tokens = tokens(tokens_json)?;
    // The model reads any JSON leniently; a malformed shape must stop here, as the parser would.
    let shape = weft_core::validate(
        &json,
        &weft_core::ValidateOptions {
            catalog: Some(catalog),
            mode: weft_core::Mode::Strict,
            ..Default::default()
        },
    );
    if weft_core::has_errors(&shape) {
        return write(&Page::Invalid { diagnostics: shape });
    }
    let document = to_document(&json);
    let options = HtmlOptions {
        catalog,
        tokens: &tokens,
        source: wire.source,
        appearance: None,
    };
    write(&match weft_web::to_html(&document, &options) {
        Ok(code) => Page::Code { code },
        Err(invalid) => Page::Invalid {
            diagnostics: invalid.0,
        },
    })
}

/// `importHtml`: `{document, losses, diagnostics}` of a page.
pub fn import_html(html: &str, tokens_json: Option<&str>, catalog: &Catalog) -> Result<String> {
    let tokens = tokens(tokens_json)?;
    write(&weft_web::import_html(
        html,
        &ImportOptions {
            catalog,
            tokens: &tokens,
        },
    ))
}

/// `importJsx`: `{document, losses, diagnostics}` of a React or SolidJS component.
pub fn import_jsx(
    source: &str,
    typescript: bool,
    tokens_json: Option<&str>,
    catalog: &Catalog,
) -> Result<String> {
    let tokens = tokens(tokens_json)?;
    write(&weft_web::import_jsx(
        source,
        typescript,
        &ImportOptions {
            catalog,
            tokens: &tokens,
        },
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsxTables {
    max_depth: usize,
    runtime: IndexMap<&'static str, &'static str>,
}

/// The constants `@weft/to-jsx` exports, read from the generator instead of copied.
pub fn jsx_tables() -> Result<String> {
    write(&JsxTables {
        max_depth: weft_web::jsx::MAX_DEPTH,
        runtime: weft_web::jsx::react_runtime().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_results_carry_the_root_path() {
        let catalog = weft_catalog::core_catalog().unwrap();
        let out = build(
            r#"[{"role":"button","name":"Go","states":{},"props":{},"children":[]}]"#,
            "[]",
            &catalog,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(value["rootPath"], "/screen#screen");
        assert!(value["document"]["root"]["children"].is_array());
    }

    #[test]
    fn jsx_reports_a_bad_component_name_as_data() {
        let catalog = weft_catalog::core_catalog().unwrap();
        let out = to_jsx(Some("null"), r#"{"componentName":"x"}"#, &catalog).unwrap();
        assert!(out.starts_with(r#"{"error":"#), "{out}");
        let out = to_jsx(Some("null"), r#"{"framework":"solid"}"#, &catalog).unwrap();
        assert!(
            out.contains("export default function WeftScreen(props)"),
            "{out}"
        );
    }

    #[test]
    fn pages_and_components_come_back_through_the_wire() {
        let catalog = weft_catalog::core_catalog().unwrap();
        let screen = r#"{"weft":"0.1","root":{"kind":"screen","id":"s","props":{"label":"S"},"children":[{"kind":"button","id":"go","props":{},"on":{"press":"run"},"children":["Go"]}]}}"#;
        let page = to_html(Some(screen), None, r#"{"source":true}"#, &catalog).unwrap();
        let code: serde_json::Value = serde_json::from_str(&page).unwrap();
        let html = code["code"].as_str().unwrap();
        let back: serde_json::Value =
            serde_json::from_str(&import_html(html, None, &catalog).unwrap()).unwrap();
        assert_eq!(back["losses"], serde_json::json!([]), "{back}");
        assert_eq!(back["document"]["root"]["id"], "s");
        let jsx = r#"export default function S({ actions }) { return <main aria-label="S"><button onClick={() => actions.run()}>Go</button></main>; }"#;
        let back: serde_json::Value =
            serde_json::from_str(&import_jsx(jsx, false, None, &catalog).unwrap()).unwrap();
        assert_eq!(back["document"]["root"]["kind"], "screen", "{back}");
        let refused = to_html(Some(r#"{"weft":"0.1"}"#), None, "{}", &catalog).unwrap();
        assert!(refused.starts_with(r#"{"diagnostics":"#), "{refused}");
    }

    #[test]
    fn a_failed_import_is_an_empty_screen_with_w601() {
        let out = import_failure("m", "e", Some("g")).unwrap();
        assert!(out.contains("\"W601\"") && out.contains("nothing could be imported"));
    }
}
