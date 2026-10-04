//! The functions behind the exports, as plain Rust over JSON text so that native tests reach
//! them; `lib.rs` only adapts them to wasm-bindgen. Each mirrors one TypeScript function of
//! `@weft/core` or `@weft/catalog` and returns what that function returned, as JSON text.

use serde::Serialize;
use weft_catalog::{Token, TokenProblem};
use weft_core::{
    ApplyOptions, Catalog, Diagnostic, Document, ParseOptions, ValidateOptions, apply_patches,
    canonicalize, did_you_mean, parse, serialize, stringify, to_document, validate,
    validate_document,
};

use crate::boundary::{Options, Result, read_document, read_input, read_list, write};
use crate::sources::{Src, attach, collect};

#[derive(Serialize)]
struct Parsed<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<&'a Document>,
    diagnostics: &'a [Diagnostic],
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<Vec<Option<Src>>>,
}

/// `parse`: `{document?, diagnostics, sources?}`, with `sources` beside every document.
pub fn parse_markup(markup: &str, catalog: Option<&Catalog>, options: &str) -> Result<String> {
    let o = Options::read(options)?;
    let result = parse(
        markup,
        &ParseOptions {
            catalog,
            mode: o.mode,
            tokens: o.tokens.as_ref(),
            actions: o.actions.as_deref(),
        },
    );
    write(&Parsed {
        document: result.document.as_ref(),
        diagnostics: &result.diagnostics,
        sources: result.document.as_ref().map(|d| collect(&d.root)),
    })
}

/// `validate`: the diagnostics. With `sources` (positions from `parse_markup`), diagnostics
/// carry line and column, as the TypeScript core's `source` option did.
pub fn validate_json(
    input: Option<&str>,
    catalog: Option<&Catalog>,
    options: &str,
    sources: Option<&str>,
) -> Result<String> {
    let input = read_input(input)?;
    let o = Options::read(options)?;
    let options = ValidateOptions {
        catalog,
        mode: o.mode,
        tokens: o.tokens.as_ref(),
        actions: o.actions.as_deref(),
    };
    let diagnostics = match sources {
        None => validate(&input, &options),
        Some(text) => {
            // Without a catalog `validate` stops after the depth and shape checks, and with a
            // clean shape it validates `to_document(input)`; that document takes the positions.
            let shape = validate(
                &input,
                &ValidateOptions {
                    catalog: None,
                    ..options
                },
            );
            if shape.is_empty() {
                let mut document = to_document(&input);
                let sources: Vec<Option<Src>> = serde_json::from_str(text).map_err(|source| {
                    crate::boundary::BindingError::Wire {
                        what: "sources",
                        source,
                    }
                })?;
                attach(&mut document.root, sources);
                validate_document(&document, &options)
            } else {
                shape
            }
        }
    };
    write(&diagnostics)
}

pub fn serialize_document(document: Option<&str>) -> Result<String> {
    Ok(serialize(&read_document(document)?))
}

pub fn stringify_document(document: Option<&str>) -> Result<String> {
    Ok(stringify(&read_document(document)?))
}

pub fn canonicalize_document(document: Option<&str>) -> Result<String> {
    write(&canonicalize(&read_document(document)?))
}

#[derive(Serialize)]
struct Patched<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<&'a Document>,
    diagnostics: &'a [Diagnostic],
}

/// `applyPatches`: `{document?, diagnostics}`.
pub fn apply(
    document: Option<&str>,
    patches: Option<&str>,
    catalog: &Catalog,
    options: &str,
) -> Result<String> {
    let document = read_document(document)?;
    let patches = read_input(patches)?;
    let o = Options::read(options)?;
    let result = apply_patches(
        &document,
        &patches,
        &ApplyOptions {
            catalog,
            mode: o.mode,
            tokens: o.tokens.as_ref(),
            actions: o.actions.as_deref(),
        },
    );
    write(&Patched {
        document: result.document.as_ref(),
        diagnostics: &result.diagnostics,
    })
}

pub fn suggest(word: &str, candidates: &str) -> Result<Option<String>> {
    Ok(did_you_mean(word, read_list(candidates, "candidates")?))
}

#[derive(Serialize)]
struct LoadedTokens<'a> {
    /// Entries rather than an object: a JavaScript object would move integer-like paths first,
    /// and the TypeScript `Map` keeps the order the tokens were found in.
    tokens: Vec<(&'a String, &'a Token)>,
    problems: &'a [TokenProblem],
}

/// `loadTokens`: `{tokens: [path, token][], problems}`.
pub fn load_tokens(json: Option<&str>) -> Result<String> {
    let loaded = weft_catalog::load_tokens(&read_input(json)?);
    write(&LoadedTokens {
        tokens: loaded.tokens.iter().collect(),
        problems: &loaded.problems,
    })
}

/// `diffCatalogs`: `{level, changes}`.
pub fn diff_catalogs(previous: Option<&str>, next: Option<&str>) -> Result<String> {
    write(&weft_catalog::diff_catalogs(
        &read_input(previous)?,
        &read_input(next)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value as Json, json};

    fn catalog() -> Catalog {
        weft_catalog::core_catalog().unwrap()
    }

    fn json(text: &str) -> Json {
        serde_json::from_str(text).unwrap()
    }

    const LOGIN: &str =
        r#"<screen id="login" label="Sign in" weft="0.1"><button id="go">Go</button></screen>"#;

    #[test]
    fn parse_returns_the_document_diagnostics_and_one_position_per_node() {
        let out = json(&parse_markup(LOGIN, Some(&catalog()), "{}").unwrap());
        assert_eq!(out["document"]["root"]["children"][0]["id"], "go");
        assert_eq!(out["diagnostics"], json!([]));
        assert_eq!(out["sources"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_syntax_error_leaves_out_document_and_positions() {
        let out = json(&parse_markup("<screen", None, "{}").unwrap());
        assert!(out.get("document").is_none() && out.get("sources").is_none());
        assert_eq!(out["diagnostics"][0]["code"], "W110");
    }

    #[test]
    fn validate_with_positions_reports_line_and_column() {
        let markup = "<screen id=\"s\" weft=\"0.1\">\n  <button id=\"go\" variant=\"loud\">Go</button>\n</screen>";
        let parsed = json(&parse_markup(markup, None, "{}").unwrap());
        let document = parsed["document"].to_string();
        let sources = parsed["sources"].to_string();
        let catalog = catalog();
        let with =
            json(&validate_json(Some(&document), Some(&catalog), "{}", Some(&sources)).unwrap());
        let without = json(&validate_json(Some(&document), Some(&catalog), "{}", None).unwrap());
        assert_eq!(with[0]["code"], "W203");
        assert_eq!(
            (&with[0]["line"], &without[0]["line"]),
            (&json!(2), &Json::Null)
        );
    }

    #[test]
    fn validate_with_positions_still_reports_shape_errors() {
        let out = json(&validate_json(Some("{}"), Some(&catalog()), "{}", Some("[]")).unwrap());
        assert_eq!(out[0]["code"], "W200");
    }

    #[test]
    fn input_json_cannot_write_is_too_deep() {
        let out = json(&validate_json(None, Some(&catalog()), "{}", None).unwrap());
        assert_eq!(out[0]["path"], "#");
        assert_eq!(out[0]["code"], "W200");
    }

    #[test]
    fn strict_mode_turns_unknown_elements_into_errors() {
        let markup = r#"<screen id="s" weft="0.1"><fancy id="f"/></screen>"#;
        let lenient = json(&parse_markup(markup, Some(&catalog()), "{}").unwrap());
        let strict = json(&parse_markup(markup, Some(&catalog()), r#"{"strict":true}"#).unwrap());
        assert_eq!(lenient["diagnostics"][0]["severity"], "warning");
        assert_eq!(strict["diagnostics"][0]["severity"], "error");
    }

    #[test]
    fn writers_take_documents_as_json() {
        let doc = r#"{"weft":"0.1","root":{"props":{"z":1,"a":true},"kind":"screen","id":"s"}}"#;
        assert_eq!(
            serialize_document(Some(doc)).unwrap(),
            "<screen id=\"s\" a=\"true\" weft=\"0.1\" z=\"1\"/>\n"
        );
        assert!(stringify_document(Some(doc)).unwrap().ends_with("}\n"));
        let canonical = json(&canonicalize_document(Some(doc)).unwrap());
        assert_eq!(canonical["root"]["props"], json!({"a": true, "z": 1.0}));
    }

    #[test]
    fn patches_apply_and_rejections_carry_diagnostics() {
        let parsed = json(&parse_markup(LOGIN, Some(&catalog()), "{}").unwrap());
        let doc = parsed["document"].to_string();
        let ok = json(
            &apply(
                Some(&doc),
                Some(r#"[{"op":"set","id":"go","prop":"variant","value":"primary"}]"#),
                &catalog(),
                "{}",
            )
            .unwrap(),
        );
        assert_eq!(
            ok["document"]["root"]["children"][0]["props"]["variant"],
            "primary"
        );
        let rejected = json(&apply(Some(&doc), Some("{}"), &catalog(), "{}").unwrap());
        assert!(rejected.get("document").is_none());
        assert_eq!(rejected["diagnostics"][0]["code"], "W501");
    }

    #[test]
    fn suggestions_come_from_the_candidates() {
        assert_eq!(
            suggest("buton", r#"["button","link"]"#).unwrap().as_deref(),
            Some("did you mean \"button\"?")
        );
        assert_eq!(suggest("zzz", "[]").unwrap(), None);
    }

    #[test]
    fn tokens_come_back_as_entries_in_the_order_found() {
        let out = json(
            &load_tokens(Some(r#"{"b":{"$type":"color","$value":"black"},"1":{"$type":"dimension","$value":"1px"}}"#))
                .unwrap(),
        );
        assert_eq!(out["tokens"][0][0], "1");
        assert_eq!(
            out["tokens"][1][1],
            json!({"type": "color", "value": "black"})
        );
        assert_eq!(out["problems"], json!([]));
    }

    #[test]
    fn a_catalog_against_itself_is_no_change() {
        let text = weft_catalog::CORE_CATALOG_JSON;
        let out = json(&diff_catalogs(Some(text), Some(text)).unwrap());
        assert_eq!(out, json!({"level": "none", "changes": []}));
    }
}
