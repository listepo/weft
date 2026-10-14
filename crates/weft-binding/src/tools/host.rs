//! What every tool shares: the context it judges markup in, the per-call `project` scope, reading
//! markup, and the shape of a result. Ported from `packages/mcp/src/{context,project,read,result}.ts`
//! so that each host answers with the same bytes.

use indexmap::IndexMap;
use serde_json::{Value as Json, json};
use weft_catalog::{CatalogSource, KindSource, Project, ProjectOptions, load_project, token_types};
use weft_core::{
    Catalog, DataCheckOptions, DataSchema, Diagnostic, Document, Mode, ParseOptions, check_data,
    has_errors, parse, to_compact,
};

use super::settings::Limits;

/// What every tool needs to judge markup; the host decides, never the model.
#[derive(Clone, Debug)]
pub struct Context {
    pub catalog: Catalog,
    /// The catalogs merged over the core, in load order (SPEC §10.4); `None` when the host gives
    /// one catalog.
    pub catalogs: Option<Vec<CatalogSource>>,
    /// For every kind of `catalog`, the catalog that defined it and those that widened it.
    pub kinds: Option<IndexMap<String, KindSource>>,
    /// Token path → DTCG `$type`. Token references are checked only when given (SPEC §6).
    pub tokens: Option<IndexMap<String, String>>,
    /// Known host actions. Action names are checked only when given.
    pub actions: Option<Vec<String>>,
    /// The data model's schema. Bindings are checked against it only when given (SPEC §10.5).
    pub data: Option<DataSchema>,
}

impl Context {
    pub fn of_catalog(catalog: Catalog) -> Context {
        Context {
            catalog,
            catalogs: None,
            kinds: None,
            tokens: None,
            actions: None,
            data: None,
        }
    }

    /// What a loaded project gives a tool to judge markup with.
    pub fn of_project(project: Project) -> Context {
        Context {
            tokens: project.tokens.as_ref().map(token_types),
            catalog: project.catalog,
            catalogs: Some(project.catalogs),
            kinds: Some(project.kinds),
            actions: project.actions,
            data: project.data,
        }
    }

    pub(crate) fn parse_options(&self, mode: Mode) -> ParseOptions<'_> {
        ParseOptions {
            catalog: Some(&self.catalog),
            mode,
            tokens: self.tokens.as_ref(),
            actions: self.actions.as_deref(),
        }
    }
}

/// `JSON.stringify(value).length`: the limits count UTF-16 code units of JavaScript's text.
pub(crate) fn js_length(value: &Json) -> usize {
    to_compact(value).encode_utf16().count()
}

/// The context one call works in.
pub(crate) enum Scope<'a> {
    Host(&'a Context),
    /// From the call's `project` argument, with the project's warnings to return.
    Project(Box<Context>, Vec<Diagnostic>),
    /// The project has errors; nothing can be judged.
    Broken(Vec<Diagnostic>),
}

impl Scope<'_> {
    pub(crate) fn parts(&self) -> (Option<&Context>, &[Diagnostic]) {
        match self {
            Scope::Host(c) => (Some(*c), &[]),
            Scope::Project(c, d) => (Some(c.as_ref()), d),
            Scope::Broken(d) => (None, d),
        }
    }
}

/// The host's context, or the one the call's `project` argument describes; `Err` is the text of a
/// failure (the project is over its limit).
pub(crate) fn scope<'a>(
    host: &'a Context,
    project: Option<&Json>,
    mode: Mode,
    limits: &Limits,
) -> Result<Scope<'a>, String> {
    let Some(project) = project else {
        return Ok(Scope::Host(host));
    };
    if js_length(project) > limits.project_chars {
        return Err(format!(
            "The project is longer than {} characters of JSON.",
            limits.project_chars
        ));
    }
    let loaded = load_project(
        project,
        &ProjectOptions {
            read: None,
            prefix: "#/project",
            mode,
        },
    )
    .map_err(|e| format!("Internal error: {e}"))?;
    Ok(if has_errors(&loaded.diagnostics) {
        Scope::Broken(loaded.diagnostics)
    } else {
        Scope::Project(
            Box::new(Context::of_project(loaded.project)),
            loaded.diagnostics,
        )
    })
}

pub(crate) struct Read {
    pub document: Option<Document>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Read {
    pub fn ok(&self) -> bool {
        self.document.is_some() && !has_errors(&self.diagnostics)
    }
}

/// Markup → document with the context's catalog, its bindings checked against the data schema
/// when there is one and `with_data` is set.
pub(crate) fn read_markup(markup: &str, context: &Context, mode: Mode, with_data: bool) -> Read {
    let parsed = parse(markup, &context.parse_options(mode));
    let mut diagnostics = parsed.diagnostics;
    if let (Some(document), Some(data), true) = (&parsed.document, &context.data, with_data) {
        diagnostics.extend(check_data(
            document,
            &DataCheckOptions {
                catalog: &context.catalog,
                data,
            },
        ));
    }
    Read {
        document: parsed.document,
        diagnostics,
    }
}

/// What a tool call answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Text blocks, as MCP `content`; `is_error` marks a failure the model should fix.
    Content { blocks: Vec<String>, is_error: bool },
    /// `weft_render` with `html`: the accessibility tree, and the strictly valid document for the
    /// host's page renderer, whose page is the second block.
    Render(Box<RenderRequest>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderRequest {
    /// The first block: the accessibility tree as a Playwright-style snapshot.
    pub tree: String,
    pub document: Document,
    pub catalog: Catalog,
    /// The sample data model the bindings read, when the call gave one.
    pub data: Option<Json>,
}

impl Outcome {
    pub fn text(value: String) -> Outcome {
        Outcome::Content {
            blocks: vec![value],
            is_error: false,
        }
    }

    pub fn failure(value: String) -> Outcome {
        Outcome::Content {
            blocks: vec![value],
            is_error: true,
        }
    }

    /// The MCP `CallToolResult` shape, or `{"render":{…}}` for the host to finish.
    pub fn to_json(&self) -> String {
        match self {
            Outcome::Content { blocks, is_error } => {
                let content: Vec<Json> = blocks
                    .iter()
                    .map(|text| json!({"type": "text", "text": text}))
                    .collect();
                let mut out = json!({ "content": content });
                if *is_error {
                    out["isError"] = Json::Bool(true);
                }
                to_compact(&out)
            }
            Outcome::Render(r) => {
                let mut render =
                    json!({ "tree": r.tree, "document": r.document, "catalog": r.catalog });
                // Absent, not null: the renderer reads a missing data model as no data.
                if let Some(data) = &r.data {
                    render["data"] = data.clone();
                }
                to_compact(&json!({ "render": render }))
            }
        }
    }
}

/// Diagnostics as compact JSON; a flood is cut so one bad call cannot fill the model's context.
pub(crate) fn diagnostics_text(diagnostics: &[Diagnostic], limit: usize) -> String {
    let shown = &diagnostics[..diagnostics.len().min(limit)];
    let omitted = diagnostics.len() - shown.len();
    if omitted > 0 {
        to_compact(&json!({ "diagnostics": shown, "omitted": omitted }))
    } else {
        to_compact(&json!({ "diagnostics": shown }))
    }
}

/// Markup first; warnings, if any, in a second block so the markup can be used as is.
pub(crate) fn warned(markup: String, diagnostics: &[Diagnostic], limit: usize) -> Outcome {
    let mut blocks = vec![markup];
    if !diagnostics.is_empty() {
        blocks.push(diagnostics_text(diagnostics, limit));
    }
    Outcome::Content {
        blocks,
        is_error: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagnostic() -> Diagnostic {
        Diagnostic::new(weft_core::Code::W201, "#", "m", "e")
    }

    #[test]
    fn a_flood_of_diagnostics_is_cut_and_counted() {
        let all: Vec<Diagnostic> = (0..5).map(|_| diagnostic()).collect();
        let text = diagnostics_text(&all, 2);
        let v: Json = serde_json::from_str(&text).unwrap();
        assert_eq!(v["diagnostics"].as_array().unwrap().len(), 2);
        assert_eq!(v["omitted"], 3);
        assert!(!diagnostics_text(&all[..1], 2).contains("omitted"));
    }

    #[test]
    fn results_take_the_mcp_shape() {
        assert_eq!(
            Outcome::failure("x".into()).to_json(),
            r#"{"content":[{"type":"text","text":"x"}],"isError":true}"#
        );
        assert_eq!(
            Outcome::text("y".into()).to_json(),
            r#"{"content":[{"type":"text","text":"y"}]}"#
        );
    }

    #[test]
    fn lengths_count_utf16_units_of_javascript_text() {
        assert_eq!(js_length(&json!("😀")), 4);
        assert_eq!(js_length(&json!(1.0)), 1);
    }
}
