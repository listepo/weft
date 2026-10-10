//! The project file of SPEC §10: token layers, catalogs over the core, host actions and a data
//! schema shared by every screen of a project. Ported from packages/catalog/src/project.ts with
//! the same rules, paths and messages. Pure: files reach it through an injected reader. The
//! project file and every file it names are untrusted, so problems are diagnostics, never errors;
//! the only error is a broken embedded core catalog.

use indexmap::IndexMap;
use semver::{Comparator, Op, Version, VersionReq};
use serde::Serialize;
use serde_json::{Map as Object, Value as Json};
use weft_core::fragment::{FRAGMENT, host_reaches};
use weft_core::{
    Catalog, Code, ComponentDef, DataSchema, Diagnostic, Fragment, Map, Mode, ParseOptions,
    compile_data_schema, data_schema_diagnostics, did_you_mean, is_action, is_name, one_of,
    order_keys, parse, parse_json,
};

use crate::core::{CORE_CATALOG_JSON, CatalogError, core_catalog};
use crate::diff::{ChangeLevel, diff_catalogs};
use crate::resolver::{Env, TokenModifier, read_resolver};
use crate::settings::{SECTIONS, sanitize};
use crate::tokens::{Token, TokenCode, TokenProblem, load_tokens, token_types};

pub const PROJECT_FILE: &str = "weft.json";
pub const MAX_TOKEN_FILES: usize = 64;
pub const MAX_CATALOGS: usize = 32;
pub const MAX_LIBRARY_FRAGMENTS: usize = 256;

/// The shared resources; the tool sections follow them (`settings::SECTIONS`).
const RESOURCES: [&str; 6] = [
    "$schema",
    "tokens",
    "catalog",
    "actions",
    "data",
    "fragments",
];
/// Joined rather than replaced by an extension entry (SPEC §10.4).
const JOINED: [&str; 4] = ["states", "events", "allowedChildren", "allowedParents"];
/// Merged by name rather than replaced by an extension entry.
const MERGED: [&str; 2] = ["props", "slots"];
/// Element names with a fixed meaning in markup (SPEC §10.4), which no component may take.
const STRUCTURAL: [&str; 8] = [
    "context", "each", "entry", "slot", "use", "fragment", "param", "outlet",
];
const CATALOG_MEMBERS: [&str; 7] = [
    "weft",
    "name",
    "version",
    "prefix",
    "requires",
    "components",
    "fragments",
];

#[derive(Clone, Debug, PartialEq)]
pub struct Project {
    /// The core catalog, or the core catalog with the project's catalogs merged in (SPEC §10.4).
    pub catalog: Catalog,
    /// The catalogs merged over the core, in the order the project lists them.
    pub catalogs: Vec<CatalogSource>,
    /// For every kind of `catalog`, the catalog that defined it and those that extended it.
    pub kinds: IndexMap<String, KindSource>,
    /// Present when the project declares `tokens`; with a resolver, the default context.
    pub tokens: Option<IndexMap<String, Token>>,
    /// The modifiers of the project's resolver, each resolved for every context (SPEC §10.3);
    /// empty for token files.
    pub modifiers: Vec<TokenModifier>,
    /// Present when the project declares `actions`.
    pub actions: Option<Vec<String>>,
    /// Present when the project declares `data`.
    pub data: Option<DataSchema>,
    /// The data schema as written, for callers that hand it on rather than check with it (the
    /// WebAssembly boundary passes JSON, not compiled schemas).
    pub data_source: Option<Json>,
    /// The tool sections that passed their checks (SPEC §10.6), shaped as in the file; a key that
    /// is absent takes the tool's default.
    pub settings: Object<String, Json>,
}

/// A catalog the project loaded over the core.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CatalogSource {
    pub name: String,
    pub version: String,
    /// Absent for the project catalog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// The file the catalog was read from; absent for project content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KindSource {
    pub catalog: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extended_by: Vec<String>,
}

fn defined_by(catalog: &str) -> KindSource {
    KindSource {
        catalog: catalog.to_owned(),
        extended_by: vec![],
    }
}

impl Project {
    /// The setting at `path`, e.g. `["render", "outDir"]`, when the project file gives a valid one.
    pub fn setting(&self, path: &[&str]) -> Option<&Json> {
        let (first, rest) = path.split_first()?;
        rest.iter()
            .try_fold(self.settings.get(*first)?, |value, key| value.get(key))
    }
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

pub(crate) fn escape_pointer(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

/// `JSON.stringify` of a string, as the TypeScript messages quote names.
pub(crate) fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn is_token(v: &Object<String, Json>) -> bool {
    v.contains_key("$value")
}

/// DTCG trees merge group by group; a token replaces whatever was at its path (SPEC §10.3).
pub(crate) fn merge_token_trees(earlier: Json, later: Json) -> Json {
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

/// A catalog whose own members passed their checks, and where its diagnostics point.
struct Entry {
    at: String,
    weft: String,
    source: CatalogSource,
    requires: Vec<(String, Version)>,
    components: Object<String, Json>,
    /// A library's `fragments` member, entries not yet read.
    fragments: Object<String, Json>,
}

/// A library with fragments, and the catalog they are checked against: the core and the kinds
/// of the library and of the catalogs it requires, as those define them (SPEC §10.4).
struct Library {
    at: String,
    source: CatalogSource,
    requires: Vec<String>,
    fragments: Object<String, Json>,
    scope: Catalog,
}

/// Cargo's rule: `1.2.0` admits `>=1.2.0, <2.0.0`, and `0.1.0` admits `>=0.1.0, <0.2.0`.
fn compatible(required: &Version, loaded: &str) -> bool {
    let caret = VersionReq {
        comparators: vec![Comparator {
            op: Op::Caret,
            major: required.major,
            minor: Some(required.minor),
            patch: Some(required.patch),
            pre: required.pre.clone(),
        }],
    };
    Version::parse(loaded).is_ok_and(|v| caret.matches(&v))
}

fn owns(prefix: &str, kind: &str) -> bool {
    kind.strip_prefix(prefix)
        .is_some_and(|rest| rest.starts_with('-'))
}

/// Why `catalog` may not define or extend `kind` (SPEC §10.4): a library defines only new kinds
/// under its prefix, and the project catalog no new kind under a library's prefix.
fn trespass(
    catalog: &Entry,
    kind: &str,
    defined: Option<&KindSource>,
    libraries: &[&Entry],
) -> Option<(Code, String, String)> {
    let name = quote(&catalog.source.name);
    if let Some(prefix) = &catalog.source.prefix {
        if !owns(prefix, kind) {
            return Some((
                Code::W713,
                format!(
                    "{name} owns the kinds named \"{prefix}-…\" and extends no other kind, so its entry {} is ignored.",
                    quote(kind)
                ),
                format!("a new kind named \"{prefix}-…\""),
            ));
        }
        // Unique prefixes already rule this out; checked on its own so that the guarantee does
        // not rest on the other checks.
        let owner = defined?;
        return Some((
            Code::W711,
            format!(
                "The kind {} is already defined by {}, so the definition in {name} is ignored.",
                quote(kind),
                quote(&owner.catalog)
            ),
            "a kind that no other loaded catalog defines".to_owned(),
        ));
    }
    if defined.is_some() {
        return None;
    }
    let library = libraries
        .iter()
        .find(|l| l.source.prefix.as_deref().is_some_and(|p| owns(p, kind)))?;
    let prefix = library.source.prefix.as_deref().unwrap_or_default();
    let owner = quote(&library.source.name);
    Some((
        Code::W713,
        format!(
            "Kinds named \"{prefix}-…\" belong to {owner}, so {name} may not define {}; the entry is ignored.",
            quote(kind)
        ),
        format!("an extension of a kind {owner} defines, or a new kind outside its prefix"),
    ))
}

/// Why a fragment name may not be claimed where it is (SPEC §10.4): a library's fragments are
/// named under its prefix, and the project's under no loaded library's.
fn fragment_trespass(
    name: &str,
    own: Option<&CatalogSource>,
    catalogs: &[CatalogSource],
) -> Option<(String, String)> {
    if let Some(library) = own {
        let prefix = library.prefix.as_deref().unwrap_or_default();
        return (!owns(prefix, name)).then(|| {
            (
                format!(
                    "{} owns the fragments named \"{prefix}-…\", so its fragment {} is ignored.",
                    quote(&library.name),
                    quote(name)
                ),
                format!("a fragment named \"{prefix}-…\""),
            )
        });
    }
    let library = catalogs
        .iter()
        .find(|c| c.prefix.as_deref().is_some_and(|p| owns(p, name)))?;
    let prefix = library.prefix.as_deref().unwrap_or_default();
    Some((
        format!(
            "Fragments named \"{prefix}-…\" belong to {}, so the project may not define {}; the entry is ignored.",
            quote(&library.name),
            quote(name)
        ),
        format!("a fragment name outside the prefix \"{prefix}\""),
    ))
}

fn token_expected(code: TokenCode) -> &'static str {
    match code {
        TokenCode::T001 => "a JSON object of DTCG groups and tokens",
        TokenCode::T002 => "a name without \"{\", \"}\" or \".\"",
        TokenCode::T003 => "a $type on the token or on a group above it",
        TokenCode::T004 => "an alias to a token that exists",
        TokenCode::T005 => "aliases that end at a value",
        TokenCode::T006 => "a JSON object for every token and group, with a string $type",
        TokenCode::T007 => "a material colour with an alpha of 0 to 1 and a blur of 0 to 100 px",
    }
}

/// A library's fragments with their scope, built from the kinds merged so far.
fn library(
    entry: &Entry,
    core: &Entry,
    components: &Object<String, Json>,
    kinds: &IndexMap<String, KindSource>,
) -> Option<Library> {
    let requires: Vec<String> = entry
        .requires
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    let visible = |kind: &str| {
        kinds.get(kind).is_some_and(|k| {
            k.catalog == core.source.name
                || k.catalog == entry.source.name
                || requires.contains(&k.catalog)
        })
    };
    let scoped: Object<String, Json> = (components.iter())
        .filter(|(kind, _)| visible(kind))
        .map(|(kind, def)| (kind.clone(), def.clone()))
        .collect();
    let scope = Catalog {
        weft: entry.weft.clone(),
        name: entry.source.name.clone(),
        version: entry.source.version.clone(),
        // Every entry was checked on its own, so this only fails if that check is wrong.
        components: serde_json::from_value(Json::Object(scoped)).ok()?,
        fragments: Map::new(),
    };
    Some(Library {
        at: entry.at.clone(),
        source: entry.source.clone(),
        requires,
        fragments: entry.fragments.clone(),
        scope,
    })
}

type Merged = (
    Catalog,
    Vec<CatalogSource>,
    IndexMap<String, KindSource>,
    Vec<Library>,
);

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
        if self.options.read.is_none() {
            return Some(value.clone());
        }
        let Some(name) = value.as_str() else {
            self.wrong_type(
                pointer.to_owned(),
                format!("{what} must be a file name."),
                "a file name relative to the project",
            );
            return None;
        };
        let text = self.file(pointer, name)?;
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

    /// The text of a file the project names: `None` for a name it may not use (`W703`), and
    /// `Some(None)` for a file that cannot be read.
    fn file(&mut self, pointer: &str, name: &str) -> Option<Option<String>> {
        let read = self.options.read?;
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
        Some(read(name))
    }

    /// The project's `fragments` (SPEC §10.7): a name → file map, or name → markup in project
    /// content. Each is parsed against the merged catalog, which already holds the libraries'
    /// fragments, with the project's tokens and actions, and its problems point at
    /// `#/fragments/<name>`.
    fn fragments(&mut self, member: &Json, project: &Project) -> Map<Fragment> {
        let pointer = self.at("/fragments");
        let Some(entries) = member.as_object() else {
            let expected = "a JSON object of fragment names and file names";
            let message = "\"fragments\" must map fragment names to files.".to_owned();
            self.wrong_type(pointer, message, expected);
            return Map::new();
        };
        let texts = self.fragment_texts(entries, &pointer, "", None, &project.catalogs);
        let types = project.tokens.as_ref().map(token_types);
        let actions = project.actions.as_deref();
        let mut catalog = project.catalog.clone();
        let signatures = self.parse_fragments(&texts, &catalog, types.as_ref(), actions, false);
        catalog.fragments.extend(signatures);
        let loaded = self.parse_fragments(&texts, &catalog, types.as_ref(), actions, true);
        let mut out = project.catalog.fragments.clone();
        out.extend(loaded);
        out
    }

    /// The fragments every library lists (SPEC §10.4), parsed against what the library declares:
    /// its scope of kinds, its own fragments and those of the libraries it requires, with the
    /// consuming project's tokens and no host actions. Their problems point at
    /// `#/catalog/<i>/fragments/<name>`; a direct reach into the host is `W716`.
    fn library_fragments(&mut self, libraries: &[Library], project: &Project) -> Map<Fragment> {
        let texts: Vec<_> = (libraries.iter())
            .map(|library| {
                let dir = (library.source.source.as_deref())
                    .and_then(|file| file.rsplit_once('/'))
                    .map_or(String::new(), |(dir, _)| format!("{dir}/"));
                let pointer = format!("{}/fragments", library.at);
                let own = Some(&library.source);
                self.fragment_texts(&library.fragments, &pointer, &dir, own, &project.catalogs)
            })
            .collect();
        let types = project.tokens.as_ref().map(token_types);
        // Every library's parameters first, so that uses across libraries are typed below.
        let signatures: Vec<_> = (libraries.iter().zip(&texts))
            .map(|(library, texts)| {
                self.parse_fragments(texts, &library.scope, types.as_ref(), None, false)
            })
            .collect();
        let mut out = Map::new();
        for (library, texts) in libraries.iter().zip(&texts) {
            let mut scope = library.scope.clone();
            for (other, found) in libraries.iter().zip(&signatures) {
                let name = &other.source.name;
                if *name == library.source.name || library.requires.contains(name) {
                    scope.fragments.extend(found.clone());
                }
            }
            let loaded = self.parse_fragments(texts, &scope, types.as_ref(), None, true);
            for (name, fragment) in loaded {
                let at = format!("{}/fragments/{}", library.at, escape_pointer(&name));
                for mut d in host_reaches(&fragment.document.root) {
                    d.path = format!("{at}{}", d.path.trim_start_matches('#'));
                    self.report(d);
                }
                out.insert(name, fragment);
            }
        }
        out
    }

    /// The markup of each entry of a `fragments` member, with the pointer its problems take: a
    /// file name relative to `dir`, or the markup itself in project content. `own` is the library
    /// that lists the entries; without one they are the project's. A name outside its owner is
    /// `W715` and is left out.
    fn fragment_texts(
        &mut self,
        entries: &Object<String, Json>,
        pointer: &str,
        dir: &str,
        own: Option<&CatalogSource>,
        catalogs: &[CatalogSource],
    ) -> Vec<(String, String, String)> {
        // An entry of a catalog file is a catalog problem, one of the project file a project one.
        let (shape, relative) = match own {
            Some(_) => (Code::W706, "a file name relative to the catalog file"),
            None => (Code::W701, "a file name relative to the project"),
        };
        let mut texts = Vec::new();
        for (name, value) in entries {
            let at = format!("{pointer}/{}", escape_pointer(name));
            if is_name(name)
                && let Some((message, expected)) = fragment_trespass(name, own, catalogs)
            {
                self.report(Diagnostic::new(Code::W715, at, message, expected).got(name));
                continue;
            }
            let text = match value.as_str() {
                _ if !is_name(name) => {
                    let message = format!("{} is not a fragment name.", quote(name));
                    let expected = "a name such as \"page-header\"";
                    self.report(Diagnostic::new(shape, at, message, expected));
                    continue;
                }
                None => {
                    let message = format!("Fragment {} must be a file name.", quote(name));
                    self.report(Diagnostic::new(shape, at, message, relative));
                    continue;
                }
                Some(markup) if self.options.read.is_none() => markup.to_owned(),
                Some(file) => match self.file(&at, &format!("{dir}{file}")) {
                    Some(Some(text)) => text,
                    Some(None) => {
                        let message = format!("The file {} cannot be read.", quote(file));
                        let d = Diagnostic::new(Code::W704, &at, message, "a readable Weft file");
                        self.report(d.got(file));
                        continue;
                    }
                    None => continue,
                },
            };
            texts.push((name.clone(), at, text));
        }
        texts
    }

    /// The fragments among `texts` parsed against `catalog`. Only the pass with `report` says
    /// what is wrong; the pass without it reads the parameters that the other fragments' uses
    /// are typed by.
    fn parse_fragments(
        &mut self,
        texts: &[(String, String, String)],
        catalog: &Catalog,
        tokens: Option<&IndexMap<String, String>>,
        actions: Option<&[String]>,
        report: bool,
    ) -> Map<Fragment> {
        let options = ParseOptions {
            catalog: Some(catalog),
            mode: self.options.mode,
            tokens,
            actions,
        };
        let mut out = Map::new();
        for (name, at, text) in texts {
            let parsed = parse(text, &options);
            for mut d in parsed.diagnostics.into_iter().filter(|_| report) {
                d.path = format!("{at}{}", d.path.trim_start_matches('#'));
                self.report(d);
            }
            match parsed.document {
                Some(document) if document.root.kind == FRAGMENT => {
                    out.insert(name.clone(), Fragment::new(document));
                }
                Some(_) if report => {
                    let message = format!("Fragment {} is not a <fragment> file.", quote(name));
                    let d = Diagnostic::new(Code::W201, at, message, "<fragment> as the root");
                    self.report(d);
                }
                _ => {}
            }
        }
        out
    }

    /// The project's `tokens`: layered token files, or a resolver document (SPEC §10.3).
    fn tokens(&mut self, member: &Json) -> (Option<IndexMap<String, Token>>, Vec<TokenModifier>) {
        let resolver = match (self.options.read, member) {
            (Some(_), Json::String(name)) => Some(name.as_str()),
            (None, Json::Object(_)) => Some(""),
            _ => None,
        };
        match resolver {
            Some(name) => self.resolver(member, name, &self.at("/tokens")),
            None => (self.token_layers(member), vec![]),
        }
    }

    /// Tokens from a resolver document: `name` is its file, or `""` for project content.
    pub(crate) fn resolver(
        &mut self,
        member: &Json,
        name: &str,
        pointer: &str,
    ) -> (Option<IndexMap<String, Token>>, Vec<TokenModifier>) {
        let Some(doc) = self.content(pointer, member, "\"tokens\"") else {
            return (None, vec![]);
        };
        if !doc.is_object() {
            self.report(Diagnostic::new(
                Code::W705,
                pointer,
                "A resolver document must be a JSON object.",
                "a DTCG resolver document",
            ));
            return (None, vec![]);
        }
        let read = self.options.read;
        let mode = self.options.mode;
        let mut found = Vec::new();
        let mut fetch = |file: &str, at: &str| -> Option<Json> {
            let read = read?;
            let text = read(file);
            match text.as_deref().map(parse_json) {
                Some(Ok(json)) => Some(json),
                _ => {
                    let message = if text.is_none() {
                        format!("The file {} cannot be read.", quote(file))
                    } else {
                        format!("The file {} is not JSON.", quote(file))
                    };
                    found.push(
                        Diagnostic::new(Code::W704, at, message, "a readable JSON file").got(file),
                    );
                    None
                }
            }
        };
        let mut reported = Vec::new();
        let mut report = |d: Diagnostic| reported.push(d.mode(mode));
        // References are relative to the resolver; project content has no files to reference.
        let dir = read.map(|_| {
            name.rsplit_once('/')
                .map_or(String::new(), |(d, _)| d.to_owned())
        });
        let resolver = read_resolver(
            &doc,
            &mut Env {
                fetch: &mut fetch,
                report: &mut report,
                at: pointer.to_owned(),
                dir,
            },
        );
        for d in found.into_iter().chain(reported) {
            self.report(d);
        }
        let mut seen: Vec<TokenProblem> = vec![];
        let base = load_tokens(&order_keys(resolver.tree(&Default::default())));
        self.token_problems(pointer, base.problems, None, &mut seen);
        let mut modifiers = vec![];
        for m in &resolver.modifiers {
            let mut contexts = IndexMap::new();
            for context in m.contexts.keys() {
                let input = [(m.name.as_str(), context.as_str())].into_iter().collect();
                let loaded = load_tokens(&order_keys(resolver.tree(&input)));
                let label = format!("{}={context}", m.name);
                self.token_problems(pointer, loaded.problems, Some(&label), &mut seen);
                for (path, token) in &loaded.tokens {
                    if let Some(base) = base.tokens.get(path)
                        && base.kind != token.kind
                    {
                        self.report(
                            Diagnostic::new(
                                Code::W705,
                                pointer,
                                format!(
                                    "{label}: {path}: The token is a {} here but a {} in the default context.",
                                    token.kind, base.kind
                                ),
                                "one $type for a token in every context",
                            )
                            .got(path.clone()),
                        );
                    }
                }
                contexts.insert(context.clone(), loaded.tokens);
            }
            modifiers.push(TokenModifier {
                name: m.name.clone(),
                default: m.default.clone(),
                contexts,
            });
        }
        (Some(base.tokens), modifiers)
    }

    /// Reports token problems once: a context repeats the problems its tokens share with the
    /// default context, which are said there.
    fn token_problems(
        &mut self,
        pointer: &str,
        problems: Vec<TokenProblem>,
        context: Option<&str>,
        seen: &mut Vec<TokenProblem>,
    ) {
        for p in problems {
            if context.is_some() && seen.contains(&p) {
                continue;
            }
            seen.push(p.clone());
            let mut message = if p.path.is_empty() {
                p.message
            } else {
                format!("{}: {}", p.path, p.message)
            };
            if let Some(context) = context {
                message = format!("{context}: {message}");
            }
            self.report(
                Diagnostic::new(Code::W705, pointer, message, token_expected(p.code)).got(p.path),
            );
        }
    }

    fn token_layers(&mut self, member: &Json) -> Option<IndexMap<String, Token>> {
        let pointer = self.at("/tokens");
        let (what, resolver) = if self.options.read.is_none() {
            ("token trees", "a resolver document")
        } else {
            ("token file names", "a resolver file name")
        };
        let Some(entries) = member.as_array() else {
            self.wrong_type(
                pointer,
                format!("\"tokens\" must be an array of {what} or {resolver}."),
                &format!("an array of {what} or {resolver}"),
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
        self.token_problems(&pointer, loaded.problems, None, &mut vec![]);
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

    /// The core with the project's catalogs merged in (SPEC §10.4), or `None` when `catalog`
    /// lists too many. Libraries merge first and the project catalog last, wherever it is listed,
    /// so the list order changes listings and diagnostics but never the merged catalog.
    fn catalogs(&mut self, member: &Json, core: &Json) -> Option<Merged> {
        let pointer = self.at("/catalog");
        let listed: Vec<(String, String, &Json)> = match member {
            Json::Array(items) if items.len() > MAX_CATALOGS => {
                self.wrong_type(
                    pointer,
                    format!(
                        "\"catalog\" lists {} catalogs; a project has at most {MAX_CATALOGS}.",
                        items.len()
                    ),
                    &format!("at most {MAX_CATALOGS} catalogs"),
                );
                return None;
            }
            Json::Array(items) => items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    (
                        format!("{pointer}/{i}"),
                        format!("Entry {i} of \"catalog\""),
                        item,
                    )
                })
                .collect(),
            // One catalog alone keeps the pointers it always had.
            one => vec![(pointer, "\"catalog\"".to_owned(), one)],
        };
        let base = self.read_catalog(String::new(), core, None)?;
        let mut entries: Vec<Entry> = Vec::new();
        for (at, what, item) in listed {
            let Some(found) = self.content(&at, item, &what) else {
                continue;
            };
            let source = self.options.read.and(item.as_str()).map(str::to_owned);
            if let Some(entry) = self.read_catalog(at, &found, source)
                && self.claims(&entry, &entries, &base)
            {
                entries.push(entry);
            }
        }
        self.requirements(&entries, &base);
        let mut components = base.components.clone();
        let mut kinds = base
            .components
            .keys()
            .map(|k| (k.clone(), defined_by(&base.source.name)))
            .collect();
        let (libraries, project): (Vec<&Entry>, Vec<&Entry>) =
            entries.iter().partition(|e| e.source.prefix.is_some());
        let core = &base.source.name;
        for entry in &libraries {
            self.merge_catalog(entry, &libraries, core, &mut components, &mut kinds);
        }
        // Before the project catalog widens anything: a library's fragments may use only what
        // every other project that loads the library also has.
        let shelves = (libraries.iter())
            .filter(|l| !l.fragments.is_empty())
            .filter_map(|l| library(l, &base, &components, &kinds))
            .collect();
        for entry in &project {
            self.merge_catalog(entry, &libraries, core, &mut components, &mut kinds);
        }
        // The project catalog names the result, else the last library, so that callers reading
        // one name see the catalog they wrote.
        let named = project.first().copied().or(entries.last()).unwrap_or(&base);
        let mut catalog = Object::new();
        catalog.insert("weft".to_owned(), Json::String(named.weft.clone()));
        catalog.insert("name".to_owned(), Json::String(named.source.name.clone()));
        catalog.insert(
            "version".to_owned(),
            Json::String(named.source.version.clone()),
        );
        catalog.insert(
            "components".to_owned(),
            order_keys(Json::Object(components)),
        );
        // Every entry was checked on its own, so this only fails if that check is wrong.
        let catalog = serde_json::from_value(Json::Object(catalog)).ok()?;
        Some((
            catalog,
            entries.into_iter().map(|e| e.source).collect(),
            kinds,
            shelves,
        ))
    }

    /// A catalog's own members (SPEC §5), or `None` with `W706` when they have the wrong shape.
    fn read_catalog(
        &mut self,
        pointer: String,
        extension: &Json,
        source: Option<String>,
    ) -> Option<Entry> {
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
        let prefix = ext.get("prefix");
        let requires = ext.get("requires").map(Json::as_object);
        let fragments = ext.get("fragments").map(Json::as_object);
        if ext.keys().any(|k| !CATALOG_MEMBERS.contains(&k.as_str()))
            || prefix.is_some_and(|p| !p.is_string())
            || requires.is_some_and(|r| r.is_none())
        {
            self.report_not_catalog(pointer);
            return None;
        }
        if let Some(fragments) = fragments {
            let problem = if prefix.is_none() {
                Some("The project catalog lists no fragments; the project file's \"fragments\" does, so the catalog is ignored.".to_owned())
            } else if fragments.is_none_or(|f| f.len() > MAX_LIBRARY_FRAGMENTS) {
                Some(format!(
                    "\"fragments\" must map at most {MAX_LIBRARY_FRAGMENTS} fragment names to files, so the catalog is ignored."
                ))
            } else {
                None
            };
            if let Some(message) = problem {
                let expected = format!(
                    "on a library only, an object of at most {MAX_LIBRARY_FRAGMENTS} fragment names and file names"
                );
                self.report(Diagnostic::new(
                    Code::W706,
                    format!("{pointer}/fragments"),
                    message,
                    expected,
                ));
                return None;
            }
        }
        let mut required = Vec::new();
        for (catalog, value) in requires.flatten().into_iter().flatten() {
            let Some(Ok(version)) = value.as_str().map(Version::parse) else {
                let at = format!("{pointer}/requires/{}", escape_pointer(catalog));
                let message = format!(
                    "The requirement on {} is not a version, so the catalog is ignored.",
                    quote(catalog)
                );
                let expected = "a version such as \"1.2.0\", read by Cargo's compatibility rule";
                self.report(
                    Diagnostic::new(Code::W706, at, message, expected).got(value.to_string()),
                );
                return None;
            };
            required.push((catalog.clone(), version));
        }
        Some(Entry {
            at: pointer,
            weft,
            source: CatalogSource {
                name,
                version,
                prefix: prefix.and_then(Json::as_str).map(str::to_owned),
                source,
            },
            requires: required,
            components: entries.clone(),
            fragments: fragments.flatten().cloned().unwrap_or_default(),
        })
    }

    /// The catalog's claims on its name and prefix (`W711`, `W712`). A catalog that loses one is
    /// ignored whole, since its kinds would otherwise land under an owner it did not declare.
    fn claims(&mut self, entry: &Entry, earlier: &[Entry], core: &Entry) -> bool {
        let (at, name) = (&entry.at, &entry.source.name);
        let taken = std::iter::once(core)
            .chain(earlier)
            .find(|e| e.source.name == *name);
        if let Some(owner) = taken {
            let owner = if owner.at.is_empty() {
                "the core catalog".to_owned()
            } else {
                format!("the catalog at {}", quote(&owner.at))
            };
            let message = format!(
                "The catalog name {} is already taken by {owner}, so this catalog is ignored.",
                quote(name)
            );
            let d = Diagnostic::new(
                Code::W711,
                format!("{at}/name"),
                message,
                "a catalog name that no other loaded catalog has",
            );
            self.report(d.got(name));
            return false;
        }
        let Some(prefix) = &entry.source.prefix else {
            let Some(project) = earlier.iter().find(|e| e.source.prefix.is_none()) else {
                return true;
            };
            let message = format!(
                "{} has no \"prefix\", but {} is already the project catalog, so {} is ignored.",
                quote(name),
                quote(&project.source.name),
                quote(name)
            );
            let d = Diagnostic::new(
                Code::W712,
                at,
                message,
                "a \"prefix\" on every catalog but the project's own",
            );
            self.report(d.hint("A shared catalog declares a \"prefix\" and owns the kinds named after it; only the project catalog has none."));
            return false;
        };
        let reserved = prefix == "x"
            || prefix == "weft"
            || core
                .components
                .keys()
                .any(|k| k.split('-').next() == Some(prefix));
        if !is_name(prefix) || prefix.contains('-') || reserved {
            let why = if reserved {
                "is reserved"
            } else {
                "is not one lowercase name segment"
            };
            let message = format!(
                "The prefix {} of {} {why}, so the catalog is ignored.",
                quote(prefix),
                quote(name)
            );
            let expected = "one lowercase segment such as \"acme\" that is not \"x\", \"weft\", a core kind or a core kind's first segment";
            self.report(
                Diagnostic::new(Code::W712, format!("{at}/prefix"), message, expected).got(prefix),
            );
            return false;
        }
        if let Some(owner) = earlier
            .iter()
            .find(|e| e.source.prefix.as_ref() == Some(prefix))
        {
            let message = format!(
                "The prefix {} is already owned by {} ({}), so {} is ignored.",
                quote(prefix),
                quote(&owner.source.name),
                quote(&owner.at),
                quote(name)
            );
            let d = Diagnostic::new(
                Code::W711,
                format!("{at}/prefix"),
                message,
                "a prefix that no other loaded catalog declares",
            );
            self.report(d.got(prefix));
            return false;
        }
        true
    }

    /// `W714` for each requirement no loaded catalog meets. Only a warning: the structural checks
    /// keep the merged catalog valid either way, and this says why an entry may have failed them.
    fn requirements(&mut self, entries: &[Entry], core: &Entry) {
        for entry in entries {
            for (required, version) in &entry.requires {
                let loaded = std::iter::once(core)
                    .chain(entries)
                    .find(|e| e.source.name == *required)
                    .map(|e| e.source.version.as_str());
                if loaded.is_some_and(|v| compatible(version, v)) {
                    continue;
                }
                let (catalog, needed) = (quote(&entry.source.name), quote(required));
                let message = match loaded {
                    None => format!("{catalog} requires {needed} {version}, which is not loaded."),
                    Some(v) => format!("{catalog} requires {needed} {version}, but {v} is loaded."),
                };
                let at = format!("{}/requires/{}", entry.at, escape_pointer(required));
                let expected = format!("{needed} loaded at a version compatible with {version}");
                self.report(
                    Diagnostic::new(Code::W714, at, message, expected)
                        .got_opt(loaded.map(str::to_owned)),
                );
            }
        }
    }

    /// One catalog's entries merged in: a library's whole definitions of kinds under its prefix,
    /// or the project catalog's new kinds and its extensions, which may only widen what they
    /// extend.
    fn merge_catalog(
        &mut self,
        catalog: &Entry,
        libraries: &[&Entry],
        core: &str,
        components: &mut Object<String, Json>,
        kinds: &mut IndexMap<String, KindSource>,
    ) {
        let pointer = &catalog.at;
        let kept = |owner: &str| {
            if owner == core {
                "its core definition".to_owned()
            } else {
                format!("its definition from {}", quote(owner))
            }
        };
        for (kind, entry) in &catalog.components {
            let at = format!("{pointer}/components/{}", escape_pointer(kind));
            if let Some((code, message, expected)) =
                trespass(catalog, kind, kinds.get(kind), libraries)
            {
                self.report(Diagnostic::new(code, at, message, expected).got(kind));
                continue;
            }
            let base = components.get(kind).and_then(Json::as_object);
            let invalid = |message: String, expected: &str| {
                Diagnostic::new(Code::W706, at.clone(), message, expected).got(kind)
            };
            if base.is_none() && (!is_name(kind) || STRUCTURAL.contains(&kind.as_str())) {
                let d = invalid(
                    format!("The new component name {} is not allowed.", quote(kind)),
                    "a lowercase name such as \"rating\" that is not a structural element (SPEC §10.4) and does not start with \"x-\"",
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
                                "The extension of {} would break existing screens, so it keeps {}: {}",
                                quote(kind),
                                kept(kinds.get(kind).map_or(core, |k| k.catalog.as_str())),
                                breaking.message
                            ),
                            "a change that only widens the definition it extends (SPEC §8)",
                        )
                        .got(breaking.path.clone()),
                    );
                    continue;
                }
            }
            components.insert(kind.clone(), merged);
            match kinds.get_mut(kind) {
                Some(source) => source.extended_by.push(catalog.source.name.clone()),
                None => {
                    kinds.insert(kind.clone(), defined_by(&catalog.source.name));
                }
            }
        }
    }

    fn report_not_catalog(&mut self, pointer: String) {
        self.report(Diagnostic::new(
            Code::W706,
            pointer,
            "The catalog extension is not a catalog.",
            "an object with \"weft\", \"name\", \"version\" and \"components\", and optionally \"prefix\" and \"requires\" (SPEC §5)",
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
    let catalog = core_catalog()?;
    Ok(Project {
        kinds: (catalog.components.keys())
            .map(|k| (k.clone(), defined_by(&catalog.name)))
            .collect(),
        catalog,
        catalogs: vec![],
        tokens: None,
        modifiers: vec![],
        actions: None,
        data: None,
        data_source: None,
        settings: Object::new(),
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

    let known = || RESOURCES.into_iter().chain(SECTIONS.iter().map(|s| s.name));
    for name in members.keys() {
        if known().any(|k| k == name) {
            continue;
        }
        loader.report(
            Diagnostic::new(
                Code::W702,
                loader.at(&format!("/{}", escape_pointer(name))),
                format!("Unknown member {} in the project file.", quote(name)),
                one_of(known()),
            )
            .got(name)
            .hint_opt(did_you_mean(name, known())),
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
        let (tokens, modifiers) = loader.tokens(member);
        project.tokens = tokens;
        project.modifiers = modifiers;
    }
    if let Some(member) = members.get("catalog") {
        let core: Json = serde_json::from_str(CORE_CATALOG_JSON)?;
        if let Some((catalog, catalogs, kinds, libraries)) = loader.catalogs(member, &core) {
            project.catalog = catalog;
            project.catalogs = catalogs;
            project.kinds = kinds;
            project.catalog.fragments = loader.library_fragments(&libraries, &project);
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
        for mut d in data_schema_diagnostics(&problems) {
            // The helper points into the schema; a project points into its `data` member.
            d.path = loader.at(&format!("/data{}", &d.path[1..]));
            loader.report(d);
        }
    }
    if let Some(member) = members.get("fragments") {
        project.catalog.fragments = loader.fragments(member, &project);
    }
    for section in SECTIONS {
        if let Some(member) = members.get(section.name) {
            let pointer = loader.at(&format!("/{}", section.name));
            let mut found = Vec::new();
            let clean = sanitize(section, member, &pointer, &mut |d| found.push(d));
            for d in found {
                loader.report(d);
            }
            if let Some(clean) = clean {
                project.settings.insert(section.name.to_owned(), clean);
            }
        }
    }
    Ok(ProjectLoad {
        project,
        diagnostics: loader.diagnostics,
    })
}
