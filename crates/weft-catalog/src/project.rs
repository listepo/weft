//! The project file of SPEC §10: token layers, a catalog extension, host actions and a data
//! schema shared by every screen of a project. Ported from packages/catalog/src/project.ts with
//! the same rules, paths and messages. Pure: files reach it through an injected reader. The
//! project file and every file it names are untrusted, so problems are diagnostics, never errors;
//! the only error is a broken embedded core catalog.

use indexmap::IndexMap;
use serde_json::{Map as Object, Value as Json};
use weft_core::{
    Catalog, Code, ComponentDef, DataSchema, Diagnostic, Mode, compile_data_schema, did_you_mean,
    is_action, is_name, one_of, order_keys, parse_json,
};

use crate::core::{CORE_CATALOG_JSON, CatalogError, core_catalog};
use crate::diff::{ChangeLevel, diff_catalogs};
use crate::tokens::{Token, TokenCode, load_tokens};

pub const PROJECT_FILE: &str = "weft.json";
pub const MAX_TOKEN_FILES: usize = 64;

const MEMBERS: [&str; 5] = ["$schema", "tokens", "catalog", "actions", "data"];
/// Joined rather than replaced by an extension entry (SPEC §10.4).
const JOINED: [&str; 4] = ["states", "events", "allowedChildren", "allowedParents"];
/// Merged by name rather than replaced by an extension entry.
const MERGED: [&str; 2] = ["props", "slots"];
/// Element names with a fixed meaning in markup, which no component may take.
const STRUCTURAL: [&str; 2] = ["each", "slot"];
const CATALOG_MEMBERS: [&str; 4] = ["weft", "name", "version", "components"];

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    /// The core catalog, or the core catalog with the project's extension merged in.
    pub catalog: Catalog,
    /// Present when the project declares `tokens`.
    pub tokens: Option<IndexMap<String, Token>>,
    /// Present when the project declares `actions`.
    pub actions: Option<Vec<String>>,
    /// Present when the project declares `data`.
    pub data: Option<DataSchema>,
    /// The data schema as written, for callers that hand it on rather than check with it (the
    /// WebAssembly boundary passes JSON, not compiled schemas).
    pub data_source: Option<Json>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectLoad {
    pub project: Project,
    pub diagnostics: Vec<Diagnostic>,
}

/// Returns the text of a file named by the project, relative to the project directory, or `None`
/// when it cannot be read.
pub type ReadFile<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Clone, Copy)]
pub struct ProjectOptions<'a> {
    /// Without a reader, members hold the files' JSON content instead of their names (§10.1).
    pub read: Option<ReadFile<'a>>,
    /// Where diagnostic pointers start: `#` for a project file, `#/project` for a tool argument.
    pub prefix: &'a str,
    pub mode: Mode,
}

impl Default for ProjectOptions<'_> {
    fn default() -> Self {
        ProjectOptions {
            read: None,
            prefix: "#",
            mode: Mode::Lenient,
        }
    }
}

/// SPEC §10.2: relative, `/`-separated, and inside the project directory.
pub fn is_project_file_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':', '\0'])
        && name.split('/').all(|s| !s.is_empty() && s != "..")
}

fn escape_pointer(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// `JSON.stringify` of a string, as the TypeScript messages quote names.
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn is_token(v: &Object<String, Json>) -> bool {
    v.contains_key("$value")
}

/// DTCG trees merge group by group; a token replaces whatever was at its path (SPEC §10.3).
fn merge_token_trees(earlier: Json, later: Json) -> Json {
    match (earlier, later) {
        (Json::Object(mut merged), Json::Object(later))
            if !is_token(&merged) && !is_token(&later) =>
        {
            for (name, value) in later {
                let next = match merged.get_mut(&name).map(Json::take) {
                    Some(before) => merge_token_trees(before, value),
                    None => value,
                };
                merged.insert(name, next);
            }
            Json::Object(merged)
        }
        (_, later) => later,
    }
}

fn contains_null(v: &Json) -> bool {
    match v {
        Json::Null => true,
        Json::Array(items) => items.iter().any(contains_null),
        Json::Object(map) => map.values().any(contains_null),
        _ => false,
    }
}

/// An extension entry over a core definition, field by field (SPEC §10.4).
fn extend_definition(core: &Object<String, Json>, entry: &Object<String, Json>) -> Json {
    let mut merged = core.clone();
    for (field, value) in entry {
        let next = match (merged.get(field), value) {
            (Some(Json::Object(before)), Json::Object(added))
                if MERGED.contains(&field.as_str()) =>
            {
                let mut joined = before.clone();
                for (name, def) in added {
                    joined.insert(name.clone(), def.clone());
                }
                Json::Object(joined)
            }
            (Some(Json::Array(before)), Json::Array(added)) if JOINED.contains(&field.as_str()) => {
                let mut joined = before.clone();
                joined.extend(added.iter().filter(|v| !before.contains(v)).cloned());
                Json::Array(joined)
            }
            _ => value.clone(),
        };
        merged.insert(field.clone(), next);
    }
    order_keys(Json::Object(merged))
}

fn token_expected(code: TokenCode) -> &'static str {
    match code {
        TokenCode::T001 => "a JSON object of DTCG groups and tokens",
        TokenCode::T002 => "a name without \"{\", \"}\" or \".\"",
        TokenCode::T003 => "a $type on the token or on a group above it",
        TokenCode::T004 => "an alias to a token that exists",
        TokenCode::T005 => "aliases that end at a value",
        TokenCode::T006 => "a JSON object for every token and group, with a string $type",
    }
}

struct Loader<'a> {
    options: &'a ProjectOptions<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl Loader<'_> {
    fn report(&mut self, d: Diagnostic) {
        self.diagnostics.push(d.mode(self.options.mode));
    }

    fn wrong_type(&mut self, path: String, message: String, expected: &str) {
        self.report(Diagnostic::new(Code::W701, path, message, expected));
    }

    fn at(&self, rest: &str) -> String {
        format!("{}{rest}", self.options.prefix)
    }

    /// The JSON a member names: read from a file, or the member itself without a reader.
    fn content(&mut self, pointer: &str, value: &Json, what: &str) -> Option<Json> {
        let Some(read) = self.options.read else {
            return Some(value.clone());
        };
        let Some(name) = value.as_str() else {
            self.wrong_type(
                pointer.to_owned(),
                format!("{what} must be a file name."),
                "a file name relative to the project",
            );
            return None;
        };
        if !is_project_file_name(name) {
            self.report(
                Diagnostic::new(
                    Code::W703,
                    pointer,
                    format!(
                        "The file name {} is absolute, leaves the project directory or is malformed.",
                        quote(name)
                    ),
                    "a relative path inside the project directory, separated by \"/\"",
                )
                .got(name),
            );
            return None;
        }
        let text = read(name);
        match text.as_deref().map(parse_json) {
            Some(Ok(json)) => Some(json),
            _ => {
                let message = if text.is_none() {
                    format!("The file {} cannot be read.", quote(name))
                } else {
                    format!("The file {} is not JSON.", quote(name))
                };
                self.report(
                    Diagnostic::new(Code::W704, pointer, message, "a readable JSON file").got(name),
                );
                None
            }
        }
    }

    fn token_layers(&mut self, member: &Json) -> Option<IndexMap<String, Token>> {
        let pointer = self.at("/tokens");
        let what = if self.options.read.is_none() {
            "token trees"
        } else {
            "token file names"
        };
        let Some(entries) = member.as_array() else {
            self.wrong_type(
                pointer,
                format!("\"tokens\" must be an array of {what}."),
                &format!("an array of {what}"),
            );
            return None;
        };
        if entries.len() > MAX_TOKEN_FILES {
            self.wrong_type(
                pointer,
                format!(
                    "\"tokens\" lists {} files; a project has at most {MAX_TOKEN_FILES}.",
                    entries.len()
                ),
                &format!("at most {MAX_TOKEN_FILES} token files"),
            );
            return None;
        }
        let mut tree = Json::Object(Object::new());
        for (index, entry) in entries.iter().enumerate() {
            let at = format!("{pointer}/{index}");
            let Some(found) = self.content(&at, entry, &format!("Entry {index} of \"tokens\""))
            else {
                continue;
            };
            if !found.is_object() {
                self.report(Diagnostic::new(
                    Code::W705,
                    at,
                    "A token file must be a JSON object.",
                    token_expected(TokenCode::T001),
                ));
                continue;
            }
            tree = merge_token_trees(tree, found);
        }
        let loaded = load_tokens(&order_keys(tree));
        for p in loaded.problems {
            let message = if p.path.is_empty() {
                p.message
            } else {
                format!("{}: {}", p.path, p.message)
            };
            self.report(
                Diagnostic::new(Code::W705, pointer.clone(), message, token_expected(p.code))
                    .got(p.path),
            );
        }
        Some(loaded.tokens)
    }

    fn actions(&mut self, member: &Json) -> Option<Vec<String>> {
        let pointer = self.at("/actions");
        let Some(entries) = member.as_array() else {
            self.wrong_type(
                pointer,
                "\"actions\" must be an array of action names.".to_owned(),
                "an array of action names",
            );
            return None;
        };
        let mut actions = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            let at = format!("{pointer}/{index}");
            match entry.as_str() {
                None => self.wrong_type(
                    at,
                    format!("Entry {index} of \"actions\" must be a string."),
                    "an action name",
                ),
                Some(name) if !is_action(name) => self.report(
                    Diagnostic::new(
                        Code::W708,
                        at,
                        format!("{} in \"actions\" is not an action name.", quote(name)),
                        "a dotted name such as \"cart.add\"",
                    )
                    .got(name),
                ),
                Some(name) => actions.push(name.to_owned()),
            }
        }
        Some(actions)
    }

    /// The merged catalog, or `None` when the extension is not a catalog.
    fn extend_catalog(&mut self, extension: &Json, core: &Json) -> Option<Catalog> {
        let pointer = self.at("/catalog");
        let text = |key: &str| extension.get(key).and_then(Json::as_str).map(str::to_owned);
        let ext = extension.as_object();
        let (Some(ext), Some(weft), Some(name), Some(version), Some(Json::Object(entries))) = (
            ext,
            text("weft"),
            text("name"),
            text("version"),
            extension.get("components"),
        ) else {
            self.report_not_catalog(pointer);
            return None;
        };
        if ext.keys().any(|k| !CATALOG_MEMBERS.contains(&k.as_str())) {
            self.report_not_catalog(pointer);
            return None;
        }
        let empty = Object::new();
        let core_components = core
            .get("components")
            .and_then(Json::as_object)
            .unwrap_or(&empty);
        let mut components = core_components.clone();
        for (kind, entry) in entries {
            let at = format!("{pointer}/components/{}", escape_pointer(kind));
            let base = core_components.get(kind).and_then(Json::as_object);
            let invalid = |message: String, expected: &str| {
                Diagnostic::new(Code::W706, at.clone(), message, expected).got(kind)
            };
            if base.is_none() && (!is_name(kind) || STRUCTURAL.contains(&kind.as_str())) {
                let d = invalid(
                    format!("The new component name {} is not allowed.", quote(kind)),
                    "a lowercase name such as \"rating\" that is not \"each\" or \"slot\" and does not start with \"x-\"",
                );
                self.report(d);
                continue;
            }
            if base.is_none() && kind.starts_with("x-") {
                let d = invalid(
                    format!(
                        "The new component name {} starts with \"x-\", which every catalog treats as opaque.",
                        quote(kind)
                    ),
                    "a lowercase name such as \"rating\" that does not start with \"x-\"",
                );
                self.report(d);
                continue;
            }
            let merged = match (base, entry) {
                (Some(base), Json::Object(entry)) => extend_definition(base, entry),
                _ => entry.clone(),
            };
            if contains_null(entry)
                || serde_json::from_value::<ComponentDef>(merged.clone()).is_err()
            {
                let d = invalid(
                    format!(
                        "The catalog extension entry {} is not a valid component definition.",
                        quote(kind)
                    ),
                    "a component definition of SPEC §5 without null values",
                );
                self.report(d);
                continue;
            }
            if let Some(base) = base {
                let single = |def: Json| {
                    let mut components = Object::new();
                    components.insert(kind.clone(), def);
                    let mut catalog = Object::new();
                    catalog.insert("components".to_owned(), Json::Object(components));
                    Json::Object(catalog)
                };
                let diff =
                    diff_catalogs(&single(Json::Object(base.clone())), &single(merged.clone()));
                if let Some(breaking) = diff.changes.iter().find(|c| c.level == ChangeLevel::Major)
                {
                    self.report(
                        Diagnostic::new(
                            Code::W707,
                            at,
                            format!(
                                "The extension of {} would break existing screens, so it keeps its core definition: {}",
                                quote(kind),
                                breaking.message
                            ),
                            "a change that only widens the core definition (SPEC §8)",
                        )
                        .got(breaking.path.clone()),
                    );
                    continue;
                }
            }
            components.insert(kind.clone(), merged);
        }
        let mut catalog = Object::new();
        catalog.insert("weft".to_owned(), Json::String(weft));
        catalog.insert("name".to_owned(), Json::String(name));
        catalog.insert("version".to_owned(), Json::String(version));
        catalog.insert(
            "components".to_owned(),
            order_keys(Json::Object(components)),
        );
        // Every entry was checked on its own, so this only fails if that check is wrong.
        serde_json::from_value(Json::Object(catalog)).ok()
    }

    fn report_not_catalog(&mut self, pointer: String) {
        self.report(Diagnostic::new(
            Code::W706,
            pointer,
            "The catalog extension is not a catalog.",
            "an object with \"weft\", \"name\", \"version\" and \"components\" (SPEC §5)",
        ));
    }
}

/// Loads a project from the text of its project file.
pub fn load_project_text(
    text: &str,
    options: &ProjectOptions<'_>,
) -> Result<ProjectLoad, CatalogError> {
    match parse_json(text) {
        Ok(json) => load_project(&json, options),
        Err(_) => Ok(ProjectLoad {
            project: empty_project()?,
            diagnostics: vec![
                Diagnostic::new(
                    Code::W701,
                    options.prefix,
                    "The project file is not JSON.",
                    "a JSON object",
                )
                .mode(options.mode),
            ],
        }),
    }
}

fn empty_project() -> Result<Project, CatalogError> {
    Ok(Project {
        catalog: core_catalog()?,
        tokens: None,
        actions: None,
        data: None,
        data_source: None,
    })
}

/// Loads a project from its parsed project file, or from project content without a reader.
pub fn load_project(
    json: &Json,
    options: &ProjectOptions<'_>,
) -> Result<ProjectLoad, CatalogError> {
    let mut project = empty_project()?;
    let mut loader = Loader {
        options,
        diagnostics: Vec::new(),
    };
    let Some(members) = json.as_object() else {
        loader.wrong_type(
            options.prefix.to_owned(),
            "The project file is not a JSON object.".to_owned(),
            "a JSON object",
        );
        return Ok(ProjectLoad {
            project,
            diagnostics: loader.diagnostics,
        });
    };

    for name in members.keys() {
        if MEMBERS.contains(&name.as_str()) {
            continue;
        }
        loader.report(
            Diagnostic::new(
                Code::W702,
                loader.at(&format!("/{}", escape_pointer(name))),
                format!("Unknown member {} in the project file.", quote(name)),
                one_of(MEMBERS),
            )
            .got(name)
            .hint_opt(did_you_mean(name, MEMBERS)),
        );
    }

    if members.get("$schema").is_some_and(|s| !s.is_string()) {
        loader.wrong_type(
            loader.at("/$schema"),
            "\"$schema\" must be a string.".to_owned(),
            "a string",
        );
    }
    if let Some(member) = members.get("tokens") {
        project.tokens = loader.token_layers(member);
    }
    if let Some(member) = members.get("catalog")
        && let Some(found) = loader.content(&loader.at("/catalog"), member, "\"catalog\"")
    {
        let core: Json = serde_json::from_str(CORE_CATALOG_JSON)?;
        if let Some(catalog) = loader.extend_catalog(&found, &core) {
            project.catalog = catalog;
        }
    }
    if let Some(member) = members.get("actions") {
        project.actions = loader.actions(member);
    }
    if let Some(member) = members.get("data")
        && let Some(found) = loader.content(&loader.at("/data"), member, "\"data\"")
    {
        let (schema, problems) = compile_data_schema(&found);
        project.data = Some(schema);
        project.data_source = Some(found);
        for p in problems {
            let expected = if p.code == Code::W709 {
                "a JSON Schema 2020-12 object or boolean (SPEC §10.5)"
            } else {
                "\"type\", \"properties\", \"additionalProperties\" or \"items\""
            };
            let path = loader.at(&format!("/data{}", p.pointer));
            loader.report(Diagnostic::new(p.code, path, p.message, expected));
        }
    }
    Ok(ProjectLoad {
        project,
        diagnostics: loader.diagnostics,
    })
}
