//! One function per tool, ported from `packages/mcp/src/tools/*.ts`; the order of checks and the
//! texts of failures are the TypeScript server's, so either host answers a call alike.

use serde_json::{Map, Value as Json, json};
use weft_catalog::{DocumentSchemaOptions, document_schema};
use weft_core::aria::{expected_tree, format_aria_snapshot};
use weft_core::{
    ApplyOptions, Author, Child, DataCheckOptions, Document, Mode, Node, Value, apply_patches,
    check_data, did_you_mean, has_errors, js_number, serialize, to_compact,
};

use super::ToolHost;
use super::args::Args;
use super::decls::{CONTEXT_NOTICE, ENTRY_KINDS, LISTED, primer};
use super::host::{
    Context, Outcome, RenderRequest, Scope, diagnostics_text, js_length, read_markup, scope, warned,
};
use super::settings::ContextPolicy;

pub(crate) fn call(host: &ToolHost, name: &str, args: &str) -> Outcome {
    let limits = &host.settings.limits;
    let args = match Args::read(args, limits.input_elements) {
        Ok(a) => a,
        Err(e) => return Outcome::failure(e),
    };
    let run = match name {
        "weft_primer" => return Outcome::text(primer(&host.settings)),
        "weft_capabilities" => capabilities,
        "weft_catalog" => catalog,
        "weft_schema" => schema,
        "weft_validate" => validate,
        "weft_format" => format,
        "weft_patch" => patch,
        "weft_render" => render,
        "weft_context" => context,
        _ => return Outcome::failure(format!("Unknown tool {}.", to_compact(name))),
    };
    run(host, &args).unwrap_or_else(Outcome::failure)
}

type Run = Result<Outcome, String>;

/// The call's scope, or the failure that ends it: an over-long or broken project.
fn scoped<'a>(host: &'a ToolHost, args: &Args, mode: Mode) -> Result<Scope<'a>, String> {
    let limits = &host.settings.limits;
    match scope(&host.context, args.any("project"), mode, limits)? {
        Scope::Broken(d) => Err(diagnostics_text(&d, limits.diagnostics)),
        ok => Ok(ok),
    }
}

/// `scoped` that also hands back the context, which a non-broken scope always has.
macro_rules! context_of {
    ($scope:expr) => {
        match $scope.parts() {
            (Some(c), d) => (c, d),
            (None, _) => return Err("Internal error: no context".to_owned()),
        }
    };
}

fn listed(mut names: Vec<&str>) -> Json {
    let count = names.len();
    // `Array.prototype.toSorted` compares UTF-16 code units.
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names.truncate(LISTED);
    let mut out = json!({ "count": count, "names": names });
    if count > LISTED {
        out["truncated"] = Json::Bool(true);
    }
    out
}

/// What a host advertises (SPEC §8): the format version and catalogs a writer may use, and,
/// when the host checks them, the token paths and action names it knows.
fn capabilities(host: &ToolHost, args: &Args) -> Run {
    let scope = scoped(host, args, Mode::Lenient)?;
    let (c, _) = context_of!(scope);
    let (core_name, core_version) = &host.core;
    let mut catalogs = vec![json!({ "name": core_name, "version": core_version })];
    // Every catalog is merged over the core, so the core comes first. A host that hands over one
    // merged catalog, with no list, is named by that catalog.
    match &c.catalogs {
        Some(list) => catalogs.extend(list.iter().map(|s| json!(s))),
        None if c.catalog.name != *core_name => {
            let mut one = json!({ "name": c.catalog.name, "version": c.catalog.version });
            if let Some(prefix) = &c.catalog.prefix {
                one["prefix"] = json!(prefix);
            }
            catalogs.push(one);
        }
        None => {}
    }
    let mut out = json!({ "weft": c.catalog.weft, "catalogs": catalogs });
    if let Some(tokens) = &c.tokens {
        out["tokens"] = listed(tokens.keys().map(String::as_str).collect());
    }
    if let Some(actions) = &c.actions {
        out["actions"] = listed(actions.iter().map(String::as_str).collect());
    }
    if c.data.is_some() {
        out["data"] = Json::Bool(true);
    }
    Ok(Outcome::text(to_compact(&out)))
}

/// `String(value)` in JavaScript, for a prop that may be missing or not a string.
fn js_string(value: Option<&Value>) -> String {
    match value {
        None => "undefined".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => js_number(*n),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Bind { .. } | Value::Token(_)) => "[object Object]".to_owned(),
    }
}

/// A fragment's parameters as `name: type`, required ones marked (SPEC §10.7).
fn fragment_params(children: &[Child]) -> String {
    let mut out = vec![];
    for child in children {
        let Child::Node(node) = child else { break };
        if node.kind != "param" {
            break;
        }
        let required = matches!(node.props.get("required"), Some(Value::Bool(true)));
        out.push(format!(
            "{}: {}{}",
            js_string(node.props.get("name")),
            js_string(node.props.get("type")),
            if required { " (required)" } else { "" }
        ));
    }
    out.join(", ")
}

/// One line per kind. With several catalogs, a heading before each catalog's kinds, in load
/// order after the core, so a writer sees which library a kind belongs to.
fn index_lines(host: &ToolHost, c: &Context) -> Vec<String> {
    let line = |(name, def): (&String, &weft_core::ComponentDef)| {
        let content = serde_json::to_value(def.content).unwrap_or_default();
        let content = content.as_str().unwrap_or_default();
        format!("{name} | {} | {content} | {}", def.role, def.description)
    };
    let (Some(catalogs), Some(kinds)) = (&c.catalogs, &c.kinds) else {
        return c.catalog.components.iter().map(line).collect();
    };
    if catalogs.is_empty() {
        return c.catalog.components.iter().map(line).collect();
    }
    let core = (host.core.0.as_str(), host.core.1.as_str(), None);
    let sources = catalogs
        .iter()
        .map(|s| (s.name.as_str(), s.version.as_str(), s.prefix.as_deref()));
    std::iter::once(core)
        .chain(sources)
        .flat_map(|(name, version, prefix)| {
            let own: Vec<String> = c
                .catalog
                .components
                .iter()
                .filter(|(kind, _)| kinds.get(*kind).is_some_and(|k| k.catalog == name))
                .map(line)
                .collect();
            if own.is_empty() {
                return own;
            }
            let prefix = prefix.map(|p| format!(", prefix {p}")).unwrap_or_default();
            std::iter::once(format!("kinds of {name} {version}{prefix}"))
                .chain(own)
                .collect()
        })
        .collect()
}

fn catalog(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let kind = args.string("kind", 100)?;
    let markup = args.string("markup", limits.markup_chars)?;
    let scope = scoped(host, args, Mode::Lenient)?;
    let (c, _) = context_of!(scope);
    let inline = markup
        .and_then(|m| read_markup(m, c, Mode::Lenient, true).document)
        .map(|d| d.fragments)
        .unwrap_or_default();
    let components = &c.catalog.components;
    let Some(kind) = kind else {
        let mut lines = vec![format!(
            "catalog {} {} (weft {}); kind | role | content | description",
            c.catalog.name, c.catalog.version, c.catalog.weft
        )];
        lines.extend(index_lines(host, c));
        let fragments: Vec<String> = c
            .catalog
            .fragments
            .iter()
            .map(|(name, fragment)| {
                let line = format!(
                    "fragment {name} | {}",
                    fragment_params(&fragment.document.root.children)
                );
                // A library owns the fragments under its prefix (SPEC §10.4); the project pins
                // the library's version, and the fragment's own is shown beside it.
                let library = c.catalogs.iter().flatten().find(|s| {
                    s.prefix
                        .as_ref()
                        .is_some_and(|p| name.starts_with(&format!("{p}-")))
                });
                let Some(library) = library else { return line };
                let own = fragment
                    .document
                    .version
                    .as_ref()
                    .map(|v| format!(", fragment {v}"))
                    .unwrap_or_default();
                format!("{line} | library {} {}{own}", library.name, library.version)
            })
            .collect();
        if !fragments.is_empty() {
            lines.push("fragments; <use> one as fragment | parameters | library, if any".into());
            lines.extend(fragments);
        }
        if !inline.is_empty() {
            lines
                .push("inline fragments of this screen; <use> one as fragment | parameters".into());
            lines.extend(inline.iter().map(|(name, node)| {
                format!("inline {name} | {}", fragment_params(&node.children))
            }));
        }
        return Ok(Outcome::text(lines.join("\n")));
    };
    if !components.contains_key(kind) {
        if let Some(node) = inline.get(kind) {
            return Ok(Outcome::text(inline_markup(kind, node)));
        }
        if let Some(fragment) = c.catalog.fragments.get(kind) {
            return Ok(Outcome::text(serialize(&fragment.document)));
        }
        let hint = did_you_mean(kind, components.keys())
            .map(|h| format!(" {h}"))
            .unwrap_or_default();
        return Err(format!(
            "Unknown component {}.{hint} Call weft_catalog without arguments for the list.",
            to_compact(kind)
        ));
    }
    let mut out = Map::new();
    out.insert("kind".into(), json!(kind));
    let source = c.kinds.as_ref().and_then(|k| k.get(kind));
    for part in [json!(source), json!(components[kind])] {
        if let Json::Object(members) = part {
            out.extend(members);
        }
    }
    Ok(Outcome::text(to_compact(&out)))
}

/// One inline fragment as markup: `name` is the map key, written back as an attribute.
fn inline_markup(name: &str, node: &Node) -> String {
    let mut root = node.clone();
    root.props.insert("name".into(), Value::String(name.into()));
    serialize(&Document {
        weft: String::new(),
        version: None,
        context: vec![],
        fragments: Default::default(),
        root,
    })
}

fn schema(host: &ToolHost, args: &Args) -> Run {
    let scope = scoped(host, args, Mode::Lenient)?;
    let (c, _) = context_of!(scope);
    // serde_json's text, as `documentSchema` of `@weft/core/document-schema` returns it.
    let schema = document_schema(&c.catalog, &DocumentSchemaOptions::default());
    let text = serde_json::to_string(&schema).map_err(|e| format!("Internal error: {e}"))?;
    Ok(Outcome::text(text))
}

fn validate(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let markup = args.required_string("markup", limits.markup_chars)?;
    let mode = match args.bool("strict")? {
        None => host.settings.mode,
        Some(true) => Mode::Strict,
        Some(false) => Mode::Lenient,
    };
    let scope = scope(&host.context, args.any("project"), mode, limits)?;
    let (c, scoped) = scope.parts();
    let mut diagnostics = scoped.to_vec();
    if let Some(c) = c {
        diagnostics.extend(read_markup(markup, c, mode, true).diagnostics);
    }
    let valid = !has_errors(&diagnostics);
    let text = diagnostics_text(&diagnostics, limits.diagnostics);
    Ok(Outcome::text(format!("{{\"valid\":{valid},{}", &text[1..])))
}

fn format(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let markup = args.required_string("markup", limits.markup_chars)?;
    let scope = scoped(host, args, Mode::Lenient)?;
    let (c, scoped) = context_of!(scope);
    let read = read_markup(markup, c, Mode::Lenient, true);
    let ok = read.ok();
    let all = [scoped, &read.diagnostics].concat();
    match read.document {
        Some(document) if ok => Ok(warned(serialize(&document), &all, limits.diagnostics)),
        _ => Err(diagnostics_text(&all, limits.diagnostics)),
    }
}

fn patch(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let markup = args.required_string("markup", limits.markup_chars)?;
    let patches = args.required_array("patches", limits.patches)?;
    if js_length(patches) > limits.patches_chars {
        return Err(format!(
            "The patch list is longer than {} characters.",
            limits.patches_chars
        ));
    }
    let scope = scoped(host, args, Mode::Strict)?;
    let (c, scoped) = context_of!(scope);
    let fail = |all: &[weft_core::Diagnostic]| Err(diagnostics_text(all, limits.diagnostics));
    // The current markup only has to parse; the patched result is what gets validated.
    let input = read_markup(markup, c, Mode::Lenient, false);
    let Some(document) = input.document else {
        return fail(&input.diagnostics);
    };
    // The agent channel cannot speak for a human (W512), whatever the patch claims.
    let author = Author {
        by: "agent".into(),
        name: None,
    };
    let result = apply_patches(
        &document,
        patches,
        &ApplyOptions {
            catalog: &c.catalog,
            mode: Mode::Strict,
            tokens: c.tokens.as_ref(),
            actions: c.actions.as_deref(),
            author: Some(&author),
            read_only_context: host.settings.context == ContextPolicy::ReadOnly,
        },
    );
    let Some(document) = result.document else {
        return fail(&result.diagnostics);
    };
    let mut all = [scoped, &result.diagnostics].concat();
    if let Some(data) = &c.data {
        all.extend(check_data(
            &document,
            &DataCheckOptions {
                catalog: &c.catalog,
                data,
            },
        ));
    }
    if has_errors(&all) {
        return fail(&all);
    }
    Ok(warned(serialize(&document), &all, limits.diagnostics))
}

/// What `weft_render` says when the tree is empty, so the result is never a blank block.
const NOTHING_EXPOSED: &str = "(nothing is exposed to assistive technology)";

/// The checks of `weft_render`, then the accessibility tree; for `html` the host's renderer draws
/// the page of the document it hands back.
fn render(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let markup = args.required_string("markup", limits.markup_chars)?;
    let html = args.bool("html")?.unwrap_or(false);
    let data = args.any("data");
    if data.map_or(0, js_length) > limits.data_chars {
        return Err(format!(
            "The data is longer than {} characters of JSON.",
            limits.data_chars
        ));
    }
    let scope = scoped(host, args, Mode::Strict)?;
    let (c, scoped) = context_of!(scope);
    let read = read_markup(markup, c, Mode::Strict, true);
    let ok = read.ok();
    match read.document {
        Some(document) if ok => {
            let tree = expected_tree(&document, &c.catalog, data.unwrap_or(&Json::Null));
            let mut tree = format_aria_snapshot(&tree);
            if tree.is_empty() {
                tree = NOTHING_EXPOSED.to_owned();
            }
            if !html {
                return Ok(Outcome::text(tree));
            }
            Ok(Outcome::Render(Box::new(RenderRequest {
                tree,
                document,
                catalog: c.catalog.clone(),
                data: data.cloned(),
                tokens: c.token_values.clone(),
                modifiers: c.token_modifiers.clone(),
            })))
        }
        _ => Err(diagnostics_text(
            &[scoped, &read.diagnostics].concat(),
            limits.diagnostics,
        )),
    }
}

// The format's limits (SPEC §2.3). A lenient read only warns about a context over them (W228),
// so the tool cuts it here: an oversized block must not crowd the model's window.
const MAX_ENTRIES: usize = 100;
const MAX_TEXT: usize = 500;
const MAX_TOTAL: usize = 16_000;

/// The first `max` UTF-16 code units of `text`, as `String.prototype.slice` cuts it; a cut through
/// a surrogate pair keeps neither half, where JavaScript would keep a lone high surrogate.
fn utf16_prefix(text: &str, max: usize) -> &str {
    let mut units = 0;
    for (at, ch) in text.char_indices() {
        units += ch.len_utf16();
        if units > max {
            return &text[..at];
        }
    }
    text
}

fn context(host: &ToolHost, args: &Args) -> Run {
    let limits = &host.settings.limits;
    let markup = args.required_string("markup", limits.markup_chars)?;
    let target = args.string("for", usize::MAX)?;
    let kind = args.one_of("kind", &ENTRY_KINDS)?;
    let status = args.one_of("status", &["open", "resolved"])?;
    let scope = scoped(host, args, Mode::Lenient)?;
    let (c, _) = context_of!(scope);
    let read = read_markup(markup, c, Mode::Lenient, true);
    let Some(document) = read.document else {
        return Err(diagnostics_text(&read.diagnostics, limits.diagnostics));
    };
    let matching: Vec<_> = document
        .context
        .iter()
        .filter(|e| {
            target.is_none_or(|t| e.target.as_deref().unwrap_or("") == t)
                && kind.is_none_or(|k| e.kind == k)
                && status.is_none_or(|s| e.status.as_deref() == Some(s))
        })
        .collect();
    let (mut entries, mut total) = (vec![], 0);
    for entry in &matching {
        let text = utf16_prefix(&entry.text, MAX_TEXT);
        let units = text.encode_utf16().count();
        if entries.len() == MAX_ENTRIES || total + units > MAX_TOTAL {
            break;
        }
        total += units;
        let mut kept = json!(entry);
        kept["text"] = json!(text);
        entries.push(kept);
    }
    let omitted = matching.len() - entries.len();
    let mut out = json!({ "notice": CONTEXT_NOTICE, "entries": entries });
    if omitted > 0 {
        out["omitted"] = json!(omitted);
    }
    Ok(Outcome::text(to_compact(&out)))
}
