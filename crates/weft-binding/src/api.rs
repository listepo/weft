//! The functions behind the exports, as plain Rust over JSON text so that native tests reach
//! them; the host crates adapt them to wasm-bindgen and napi-rs. Each mirrors one TypeScript function of
//! `@weft/core` or `@weft/catalog` and returns what that function returned, as JSON text.

use std::cell::RefCell;

use serde::Serialize;
use serde_json::Value as Json;
use weft_catalog::{ProjectOptions, Token, TokenProblem, load_project_text};
use weft_core::{
    ApplyOptions, Catalog, DataCheckOptions, Diagnostic, Document, ParseOptions, ValidateOptions,
    apply_patches, canonicalize, check_data, check_data_json, compile_data_schema, did_you_mean,
    format_value, parse, read_value, serialize, stringify, to_document, to_value, validate,
    validate_document,
};

use crate::boundary::{
    BindingError, Options, ProjectWire, Result, read_document, read_input, read_list, write,
};
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

#[derive(Serialize)]
struct SchemaProblem<'a> {
    code: &'a str,
    pointer: &'a str,
    message: &'a str,
}

/// `compileDataSchema`: the problems; the TypeScript side keeps the schema as JSON text.
pub fn compile_data(schema: Option<&str>) -> Result<String> {
    let (_, problems) = compile_data_schema(&read_input(schema)?);
    let problems: Vec<SchemaProblem<'_>> = problems
        .iter()
        .map(|p| SchemaProblem {
            code: p.code.as_str(),
            pointer: &p.pointer,
            message: &p.message,
        })
        .collect();
    write(&problems)
}

/// `checkData`: the diagnostics, with line and column when `sources` comes from `parse_markup`.
/// A document of the wrong shape gives none here, as `validate` already reports it.
pub fn check_data_input(
    input: Option<&str>,
    catalog: &Catalog,
    schema: Option<&str>,
    sources: Option<&str>,
) -> Result<String> {
    let input = read_input(input)?;
    let (data, _) = compile_data_schema(&read_input(schema)?);
    let options = DataCheckOptions {
        catalog,
        data: &data,
    };
    let shape_ok = validate(&input, &ValidateOptions::default()).is_empty();
    let diagnostics = match sources {
        Some(text) if shape_ok => {
            let mut document = to_document(&input);
            let sources: Vec<Option<Src>> =
                serde_json::from_str(text).map_err(|source| BindingError::Wire {
                    what: "sources",
                    source,
                })?;
            attach(&mut document.root, sources);
            check_data(&document, &options)
        }
        _ => check_data_json(&input, &options),
    };
    write(&diagnostics)
}

/// The files a project file names, in the order the loader reads them; names that would leave
/// the project directory are not among them. The TypeScript side reads these and calls
/// `load_project` with their text, so no callback crosses the boundary. A resolver names further
/// files only once it is read, so `files` (name → text) gives the files read so far and the
/// result lists the ones still missing; the caller repeats until none are.
pub fn project_files(text: &str, files: Option<&str>) -> Result<String> {
    let files = read_files(files)?;
    let names = RefCell::new(Vec::new());
    let record = |name: &str| {
        let known = files
            .as_ref()
            .and_then(|f| f.get(name))
            .and_then(Json::as_str);
        if known.is_none() {
            names.borrow_mut().push(name.to_owned());
        }
        known.map(str::to_owned)
    };
    load_project_text(
        text,
        &ProjectOptions {
            read: Some(&record),
            ..Default::default()
        },
    )
    .map_err(|weft_catalog::CatalogError::Embedded(e)| BindingError::Catalog(e))?;
    write(&names.into_inner())
}

fn read_files(files: Option<&str>) -> Result<Option<serde_json::Map<String, Json>>> {
    files
        .map(|f| {
            serde_json::from_str(f).map_err(|source| BindingError::Wire {
                what: "files",
                source,
            })
        })
        .transpose()
}

#[derive(Serialize)]
struct LoadedModifier<'a> {
    name: &'a str,
    default: &'a str,
    /// Entries, as for the tokens: `[context, [path, token][]][]`.
    contexts: Vec<(&'a String, Vec<(&'a String, &'a Token)>)>,
}

#[derive(Serialize)]
struct LoadedAppearance<'a> {
    modifier: &'a str,
    light: &'a str,
    dark: &'a str,
}

#[derive(Serialize)]
struct LoadedProject<'a> {
    catalog: &'a Catalog,
    tokens: Option<Vec<(&'a String, &'a Token)>>,
    modifiers: Vec<LoadedModifier<'a>>,
    /// The appearance modifier's light and dark contexts, by name (`weft_catalog::appearance`).
    appearance: Option<LoadedAppearance<'a>>,
    actions: Option<&'a [String]>,
    /// The data schema as JSON text, which `checkData` takes.
    data: Option<String>,
    settings: &'a serde_json::Map<String, Json>,
    diagnostics: &'a [Diagnostic],
}

/// `loadProjectText`: with `files` (name → text) members name files; without, they hold content.
pub fn load_project(text: &str, files: Option<&str>, options: &str) -> Result<String> {
    let wire = ProjectWire::read(options)?;
    let files = read_files(files)?;
    let read = |name: &str| {
        files
            .as_ref()
            .and_then(|f| f.get(name))
            .and_then(Json::as_str)
            .map(str::to_owned)
    };
    let loaded = load_project_text(
        text,
        &ProjectOptions {
            read: files
                .as_ref()
                .map(|_| &read as &dyn Fn(&str) -> Option<String>),
            prefix: &wire.prefix,
            mode: wire.mode,
        },
    )
    .map_err(|weft_catalog::CatalogError::Embedded(e)| BindingError::Catalog(e))?;
    let project = &loaded.project;
    write(&LoadedProject {
        catalog: &project.catalog,
        tokens: project.tokens.as_ref().map(|t| t.iter().collect()),
        modifiers: project
            .modifiers
            .iter()
            .map(|m| LoadedModifier {
                name: &m.name,
                default: &m.default,
                contexts: m
                    .contexts
                    .iter()
                    .map(|(c, t)| (c, t.iter().collect()))
                    .collect(),
            })
            .collect(),
        appearance: weft_catalog::appearance(&project.modifiers).map(|a| LoadedAppearance {
            modifier: a.modifier,
            light: a.light_context,
            dark: a.dark_context,
        }),
        actions: project.actions.as_deref(),
        data: project.data_source.as_ref().map(Json::to_string),
        settings: &project.settings,
        diagnostics: &loaded.diagnostics,
    })
}

/// `readValue`: one attribute value as written, read as a string prop reads it (SPEC §2.1):
/// `{ok: true, value}` or `{ok: false, message, hint}`.
pub fn read_attribute_value(raw: &str) -> Result<String> {
    write(&match read_value(raw, None) {
        Ok(value) => serde_json::json!({ "ok": true, "value": value }),
        Err(bad) => serde_json::json!({ "ok": false, "message": bad.message, "hint": bad.hint }),
    })
}

/// `formatValue`: a value as attribute text, before XML escaping.
pub fn format_attribute_value(value: Option<&str>) -> Result<String> {
    Ok(format_value(&to_value(&read_input(value)?)))
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
    fn single_values_are_read_and_written_like_attributes() {
        let read = |raw: &str| json(&read_attribute_value(raw).unwrap());
        assert_eq!(read("Sign in"), json!({ "ok": true, "value": "Sign in" }));
        assert_eq!(read("{{brace"), json!({ "ok": true, "value": "{brace" }));
        assert_eq!(
            read("{$.user.name}"),
            json!({ "ok": true, "value": { "bind": "$.user.name" } })
        );
        assert_eq!(
            read("{!$.email}"),
            json!({ "ok": true, "value": { "bind": "$.email", "not": true } })
        );
        assert_eq!(
            read("{token.space.md}"),
            json!({ "ok": true, "value": { "token": "space.md" } })
        );
        assert_eq!(read("true"), json!({ "ok": true, "value": "true" }));
        assert_eq!(read("{oops}")["ok"], json!(false));
        let format = |value: &str| format_attribute_value(Some(value)).unwrap();
        assert_eq!(format(r#""{brace""#), "{{brace");
        assert_eq!(format(r#"{"bind":"$.a","not":true}"#), "{!$.a}");
        assert_eq!(format(r#"{"token":"space.md"}"#), "{token.space.md}");
        assert_eq!(format("2"), "2");
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

    #[test]
    fn data_check_reports_line_and_column_with_positions() {
        let markup = "<screen id=\"s\" weft=\"0.1\">\n  <field id=\"f\" label=\"E\" value=\"{$.nme}\"/>\n</screen>";
        let parsed = json(&parse_markup(markup, None, "{}").unwrap());
        let document = parsed["document"].to_string();
        let sources = parsed["sources"].to_string();
        let schema = r#"{"type":"object","properties":{"name":{"type":"string"}}}"#;
        let catalog = catalog();
        let with = json(
            &check_data_input(Some(&document), &catalog, Some(schema), Some(&sources)).unwrap(),
        );
        let without =
            json(&check_data_input(Some(&document), &catalog, Some(schema), None).unwrap());
        assert_eq!(with[0]["code"], "W315");
        assert_eq!(
            (&with[0]["line"], &without[0]["line"]),
            (&json!(2), &Json::Null)
        );
        // A document of the wrong shape is `validate`'s to report.
        let shape = check_data_input(Some("{}"), &catalog, Some(schema), Some("[]")).unwrap();
        assert_eq!(shape, "[]");
    }

    #[test]
    fn a_data_schema_reports_its_problems() {
        let out = json(&compile_data(Some(r#"{"type":"thing"}"#)).unwrap());
        assert_eq!(out[0]["code"], "W709");
        assert_eq!(out[0]["pointer"], "/type");
        let deep = json(&compile_data(None).unwrap());
        assert!(!deep.as_array().unwrap().is_empty());
    }

    #[test]
    fn a_resolver_names_its_files_once_it_is_read() {
        let text = r#"{"tokens":"t/theme.resolver.json"}"#;
        assert_eq!(
            json(&project_files(text, None).unwrap()),
            json!(["t/theme.resolver.json"])
        );
        let resolver = json!({
            "version": "2025.10",
            "modifiers": { "theme": { "contexts": {
                "light": [{ "$ref": "light.json" }], "dark": [{ "$ref": "dark.json" }]
            } } },
            "resolutionOrder": [{ "$ref": "#/modifiers/theme" }]
        });
        let mut files = json!({ "t/theme.resolver.json": resolver.to_string() });
        assert_eq!(
            json(&project_files(text, Some(&files.to_string())).unwrap()),
            json!(["t/light.json", "t/dark.json"])
        );
        files["t/light.json"] = json!(r##"{"ink":{"$type":"color","$value":"#000000"}}"##);
        files["t/dark.json"] = json!(r##"{"ink":{"$type":"color","$value":"#ffffff"}}"##);
        assert_eq!(
            json(&project_files(text, Some(&files.to_string())).unwrap()),
            json!([])
        );
        let out = json(&load_project(text, Some(&files.to_string()), "{}").unwrap());
        assert_eq!(out["diagnostics"], json!([]));
        assert_eq!(out["modifiers"][0]["name"], "theme");
        assert_eq!(out["appearance"]["dark"], "dark");
    }

    #[test]
    fn a_project_names_its_files_and_loads_with_their_text() {
        let text = r#"{"tokens":["a.json","../out.json"],"actions":["go"],"data":"d.json"}"#;
        let names = json(&project_files(text, None).unwrap());
        assert_eq!(names, json!(["a.json", "d.json"]));
        let files = json!({
            "a.json": r##"{"color":{"$type":"color","brand":{"$value":"#000"}}}"##,
            "d.json": r#"{"type":"object"}"#,
        })
        .to_string();
        let out = json(&load_project(text, Some(&files), "{}").unwrap());
        assert_eq!(out["tokens"][0][0], "color.brand");
        assert_eq!(out["actions"], json!(["go"]));
        assert_eq!(out["data"], r#"{"type":"object"}"#);
        assert_eq!(out["catalog"]["name"], "weft-core");
        assert_eq!(out["diagnostics"][0]["path"], "#/tokens/1");
    }

    #[test]
    fn project_content_takes_the_pointer_prefix() {
        let out = json(
            &load_project(
                r#"{"actions":[1]}"#,
                None,
                r##"{"strict":true,"prefix":"#/project"}"##,
            )
            .unwrap(),
        );
        assert_eq!(out["diagnostics"][0]["path"], "#/project/actions/0");
        assert_eq!(out["data"], Json::Null);
        assert!(load_project("{}", None, r#"{"bogus":1}"#).is_err());
    }
}
