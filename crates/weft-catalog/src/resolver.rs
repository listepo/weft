//! The DTCG Resolver Module 2025.10 (https://www.designtokens.org/TR/2025.10/resolver/) for a
//! project's `tokens` (SPEC §10.3): a resolver document names token sets and modifiers whose
//! contexts (such as `light` and `dark` of a `theme`) select further sources, and
//! `resolutionOrder` stacks them. Reading the document checks it and turns every entry into token
//! trees; `Resolver::tree` then flattens them for one input, and the token loader resolves aliases
//! on the result, as the module orders it. The document and every file it names are untrusted:
//! problems are diagnostics, and what they spoil is left out.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{Map as Object, Value as Json};
use weft_core::{Code, Diagnostic};

use crate::project::{MAX_TOKEN_FILES, escape_pointer, merge_token_trees, quote};
use crate::tokens::Token;

/// The only version the module defines.
pub const RESOLVER_VERSION: &str = "2025.10";
/// Every context of every modifier is resolved for the generators, so their number is bounded.
pub const MAX_CONTEXTS: usize = 64;
/// Reference chains deeper than this are treated as cycles.
const MAX_DEPTH: usize = 32;

/// A modifier of the project's resolver, resolved for each of its contexts.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TokenModifier {
    pub name: String,
    /// The context Weft's input selects: the modifier's `default`, else its first context.
    pub default: String,
    /// Every context's tokens, with every other modifier at its default, in document order.
    pub contexts: IndexMap<String, IndexMap<String, Token>>,
}

/// The light and dark tokens of the project's appearance modifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance<'a> {
    /// The modifier's name and its two contexts' names, as the resolver writes them.
    pub modifier: &'a str,
    pub light_context: &'a str,
    pub dark_context: &'a str,
    pub light: &'a IndexMap<String, Token>,
    pub dark: &'a IndexMap<String, Token>,
}

/// The appearance modifier (SPEC §10.3): the first modifier with a `light` and a `dark` context.
/// Context names compare without case, as the module recommends for inputs.
pub fn appearance(modifiers: &[TokenModifier]) -> Option<Appearance<'_>> {
    modifiers.iter().find_map(|m| {
        let find = |name: &str| {
            m.contexts
                .iter()
                .find(|(c, _)| c.eq_ignore_ascii_case(name))
        };
        let (light_context, light) = find("light")?;
        let (dark_context, dark) = find("dark")?;
        Some(Appearance {
            modifier: &m.name,
            light_context,
            dark_context,
            light,
            dark,
        })
    })
}

/// Whether a parsed token file is a resolver document: `resolutionOrder` is an array, which a
/// token tree cannot hold (every member of a group is an object), so the test never misreads one.
pub fn is_resolver(json: &Json) -> bool {
    json.get("resolutionOrder").is_some_and(Json::is_array)
}

pub(crate) struct ModifierDef {
    pub name: String,
    pub default: String,
    pub contexts: IndexMap<String, Json>,
}

enum Entry {
    Tree(Json),
    Modifier(usize),
}

/// A checked resolver document: what each entry of `resolutionOrder` contributes.
pub(crate) struct Resolver {
    order: Vec<Entry>,
    pub modifiers: Vec<ModifierDef>,
}

impl Resolver {
    /// The flattened token tree for an input of modifier name to context; a modifier the input
    /// leaves out takes its default.
    pub fn tree(&self, input: &HashMap<&str, &str>) -> Json {
        let empty = Json::Object(Object::new());
        let mut tree = empty.clone();
        for entry in &self.order {
            let part = match entry {
                Entry::Tree(t) => t,
                Entry::Modifier(i) => {
                    let m = &self.modifiers[*i];
                    let context = input.get(m.name.as_str()).copied().unwrap_or(&m.default);
                    m.contexts.get(context).unwrap_or(&empty)
                }
            };
            tree = merge_token_trees(tree, part.clone());
        }
        tree
    }
}

/// How the resolver reaches the files it references and reports problems.
pub(crate) struct Env<'a> {
    /// Reads a project file by its project-relative name, reporting at the given pointer when it
    /// cannot; `None` then.
    pub fetch: &'a mut dyn FnMut(&str, &str) -> Option<Json>,
    pub report: &'a mut dyn FnMut(Diagnostic),
    /// Where the resolver's pointers start, e.g. `#/tokens`.
    pub at: String,
    /// The resolver's directory in the project (`""` at the top); `None` for project content,
    /// which has no files to reference.
    pub dir: Option<String>,
}

/// Where a reference leads: the document (`None` is the resolver itself) and the pointer in it.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Place {
    file: Option<String>,
    pointer: String,
}

struct Reader<'e, 'a> {
    env: &'e mut Env<'a>,
    doc: &'e Json,
    files: HashMap<String, Option<Json>>,
    /// The references being followed, for cycles.
    stack: Vec<Place>,
}

fn object(value: &Json) -> Option<&Object<String, Json>> {
    value.as_object()
}

impl Reader<'_, '_> {
    fn problem(&mut self, pointer: &str, message: String, expected: &str) {
        let d = Diagnostic::new(
            Code::W705,
            format!("{}{pointer}", self.env.at),
            message,
            expected,
        );
        (self.env.report)(d);
    }

    /// A project-relative file name for a reference, or why there is none.
    fn file_name(&mut self, reference: &str, pointer: &str) -> Option<String> {
        let at = format!("{}{pointer}", self.env.at);
        let Some(dir) = self.env.dir.clone() else {
            let d = Diagnostic::new(
                Code::W704,
                at,
                format!(
                    "The reference {} names a file, but project content has no files.",
                    quote(reference)
                ),
                "a reference inside the resolver (\"#/sets/…\") or inline tokens",
            )
            .got(reference);
            (self.env.report)(d);
            return None;
        };
        let mut segments: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
        let mut escapes = reference.starts_with('/') || reference.contains(['\\', ':', '\0']);
        for segment in reference.split('/') {
            match segment {
                "" | "." => {}
                ".." => escapes |= segments.pop().is_none(),
                s => segments.push(s),
            }
        }
        if escapes || segments.is_empty() {
            let d = Diagnostic::new(
                Code::W703,
                at,
                format!(
                    "The reference {} is a URL, absolute, or leaves the project directory.",
                    quote(reference)
                ),
                "a file relative to the resolver, inside the project directory",
            )
            .got(reference);
            (self.env.report)(d);
            return None;
        }
        Some(segments.join("/"))
    }

    fn file(&mut self, name: &str, pointer: &str) -> Option<Json> {
        if let Some(found) = self.files.get(name) {
            return found.clone();
        }
        if self.files.len() >= MAX_TOKEN_FILES {
            self.problem(
                pointer,
                format!("The resolver references more than {MAX_TOKEN_FILES} files."),
                "at most 64 token files",
            );
            return None;
        }
        let at = format!("{}{pointer}", self.env.at);
        let found = (self.env.fetch)(name, &at);
        self.files.insert(name.to_owned(), found.clone());
        found
    }

    /// A token source of a set or a context: a reference object or an inline token tree.
    fn source(&mut self, value: &Json, pointer: &str) -> Option<Json> {
        let Some(map) = object(value) else {
            self.problem(
                pointer,
                "A source must be a reference object or a token tree.".to_owned(),
                "{ \"$ref\": … } or a JSON object of DTCG groups and tokens",
            );
            return None;
        };
        let Some(reference) = map.get("$ref") else {
            return Some(value.clone());
        };
        let Some(reference) = reference.as_str() else {
            self.problem(
                &format!("{pointer}/$ref"),
                "`$ref` must be a string.".to_owned(),
                "a JSON Pointer or a file name",
            );
            return None;
        };
        let mut tree = self.follow(reference, &format!("{pointer}/$ref"))?;
        // Keys beside `$ref` override the target shallowly (module §Extending).
        if let Json::Object(target) = &mut tree {
            for (key, local) in map.iter().filter(|(k, _)| *k != "$ref") {
                target.insert(key.clone(), local.clone());
            }
        }
        Some(tree)
    }

    fn follow(&mut self, reference: &str, pointer: &str) -> Option<Json> {
        let (file, fragment) = reference.split_once('#').unwrap_or((reference, ""));
        let target = match fragment {
            "" => String::new(),
            f if f.starts_with('/') => f.to_owned(),
            f => format!("/{f}"),
        };
        let place = Place {
            file: if file.is_empty() {
                None
            } else {
                Some(self.file_name(file, pointer)?)
            },
            pointer: target,
        };
        if self.stack.contains(&place) || self.stack.len() >= MAX_DEPTH {
            self.problem(
                pointer,
                format!("The reference {} is circular.", quote(reference)),
                "references that end at token sources",
            );
            return None;
        }
        let document = match &place.file {
            None => self.doc.clone(),
            Some(name) => self.file(&name.clone(), pointer)?,
        };
        if place.file.is_none() {
            let first = place.pointer.split('/').nth(1).unwrap_or("");
            if first == "modifiers" || first == "resolutionOrder" {
                self.problem(
                    pointer,
                    format!(
                        "The reference {} points into `{first}`, which only `resolutionOrder` may reference by a modifier.",
                        quote(reference)
                    ),
                    "a reference to a set or a token file",
                );
                return None;
            }
        }
        let Some(found) = document.pointer(&place.pointer).cloned() else {
            self.problem(
                pointer,
                format!("The reference {} points at nothing.", quote(reference)),
                "a reference to a set or a token file",
            );
            return None;
        };
        self.stack.push(place.clone());
        let set = place.file.is_none()
            && place.pointer.starts_with("/sets/")
            && place.pointer.matches('/').count() == 2;
        let result = if set {
            // A set referenced from a set or a context stands for its sources (module §Contexts).
            self.set(&found, &place.pointer)
        } else if found.get("$ref").is_some() {
            self.source(&found, pointer)
        } else if found.is_object() {
            Some(found)
        } else {
            self.problem(
                pointer,
                format!(
                    "The reference {} does not point at a token tree.",
                    quote(reference)
                ),
                "a JSON object of DTCG groups and tokens",
            );
            None
        };
        self.stack.pop();
        result
    }

    /// The merged sources of a set.
    fn set(&mut self, value: &Json, pointer: &str) -> Option<Json> {
        let Some(sources) = value.get("sources").and_then(Json::as_array) else {
            self.problem(
                pointer,
                "A set must have a `sources` array.".to_owned(),
                "{ \"sources\": [ … ] }",
            );
            return None;
        };
        Some(self.sources(sources, &format!("{pointer}/sources")))
    }

    fn sources(&mut self, sources: &[Json], pointer: &str) -> Json {
        let mut tree = Json::Object(Object::new());
        for (i, s) in sources.iter().enumerate() {
            if let Some(part) = self.source(s, &format!("{pointer}/{i}")) {
                tree = merge_token_trees(tree, part);
            }
        }
        tree
    }

    fn modifier(&mut self, name: &str, value: &Json, pointer: &str) -> Option<ModifierDef> {
        let Some(contexts) = value.get("contexts").and_then(Json::as_object) else {
            self.problem(
                pointer,
                format!(
                    "The modifier {} must have a `contexts` object.",
                    quote(name)
                ),
                "{ \"contexts\": { \"light\": [ … ], \"dark\": [ … ] } }",
            );
            return None;
        };
        if contexts.is_empty() {
            self.problem(
                &format!("{pointer}/contexts"),
                format!("The modifier {} has no contexts.", quote(name)),
                "two or more contexts",
            );
            return None;
        }
        if contexts.len() == 1 {
            self.problem(
                &format!("{pointer}/contexts"),
                format!(
                    "The modifier {} has one context, which makes it a set.",
                    quote(name)
                ),
                "two or more contexts",
            );
        }
        let mut resolved = IndexMap::new();
        for (context, sources) in contexts {
            let at = format!("{pointer}/contexts/{}", escape_pointer(context));
            let Some(sources) = sources.as_array() else {
                self.problem(
                    &at,
                    format!(
                        "The context {} must be an array of sources.",
                        quote(context)
                    ),
                    "an array of reference objects and token trees",
                );
                continue;
            };
            let tree = self.sources(sources, &at);
            resolved.insert(context.clone(), tree);
        }
        let first = resolved.keys().next()?.clone();
        let default = match value.get("default") {
            None => first,
            Some(Json::String(d)) if resolved.contains_key(d) => d.clone(),
            Some(_) => {
                self.problem(
                    &format!("{pointer}/default"),
                    format!(
                        "The default of {} is not one of its contexts; {} applies.",
                        quote(name),
                        quote(&first)
                    ),
                    "the name of one of the modifier's contexts",
                );
                first
            }
        };
        Some(ModifierDef {
            name: name.to_owned(),
            default,
            contexts: resolved,
        })
    }
}

/// Reads and checks a resolver document; what a problem spoils is left out.
pub(crate) fn read_resolver(doc: &Json, env: &mut Env<'_>) -> Resolver {
    let mut reader = Reader {
        env,
        doc,
        files: HashMap::new(),
        stack: vec![],
    };
    let mut resolver = Resolver {
        order: vec![],
        modifiers: vec![],
    };
    if doc.get("version").and_then(Json::as_str) != Some(RESOLVER_VERSION) {
        reader.problem(
            "/version",
            format!("A resolver document must have \"version\": \"{RESOLVER_VERSION}\"."),
            "\"version\": \"2025.10\"",
        );
    }
    for key in ["sets", "modifiers"] {
        if doc.get(key).is_some_and(|v| !v.is_object()) {
            reader.problem(
                &format!("/{key}"),
                format!("`{key}` must be an object."),
                "a JSON object keyed by name",
            );
        }
    }
    // Every declared modifier is checked, referenced or not; only referenced ones count.
    let declared: Vec<(String, Json)> = doc
        .get("modifiers")
        .and_then(Json::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let mut modifiers: HashMap<String, Option<ModifierDef>> = HashMap::new();
    for (name, value) in &declared {
        let at = format!("/modifiers/{}", escape_pointer(name));
        let def = reader.modifier(name, value, &at);
        modifiers.insert(name.clone(), def);
    }
    let Some(order) = doc.get("resolutionOrder").and_then(Json::as_array) else {
        reader.problem(
            "/resolutionOrder",
            "A resolver document must have a `resolutionOrder` array.".to_owned(),
            "\"resolutionOrder\": [ … ]",
        );
        return resolver;
    };
    let mut names: Vec<String> = vec![];
    for (i, item) in order.iter().enumerate() {
        let at = format!("/resolutionOrder/{i}");
        let Some(map) = object(item) else {
            reader.problem(
                &at,
                "An entry of `resolutionOrder` must be an object.".to_owned(),
                "a reference object or an inline set or modifier",
            );
            continue;
        };
        if let Some(reference) = map.get("$ref").and_then(Json::as_str) {
            let modifier = reference
                .strip_prefix("#/modifiers/")
                .filter(|n| !n.contains('/'))
                .map(|n| n.replace("~1", "/").replace("~0", "~"));
            if let Some(name) = modifier {
                match modifiers.get_mut(&name) {
                    Some(def) => {
                        if let Some(def) = def.take() {
                            resolver.modifiers.push(def);
                        }
                        if let Some(index) = resolver.modifiers.iter().position(|m| m.name == name)
                        {
                            resolver.order.push(Entry::Modifier(index));
                        }
                        // Taken by an earlier entry, the definition is shared by index above.
                        modifiers.insert(name, None);
                    }
                    None => reader.problem(
                        &format!("{at}/$ref"),
                        format!("The reference {} points at nothing.", quote(reference)),
                        "a reference to a declared set or modifier",
                    ),
                }
                continue;
            }
            if let Some(tree) = reader.source(item, &at) {
                resolver.order.push(Entry::Tree(tree));
            }
            continue;
        }
        let name = map.get("name").and_then(Json::as_str);
        let kind = map.get("type").and_then(Json::as_str);
        let (Some(name), Some(kind @ ("set" | "modifier"))) = (name, kind) else {
            reader.problem(
                &at,
                "An inline entry of `resolutionOrder` needs a `name` and a `type` of \"set\" or \"modifier\".".to_owned(),
                "{ \"type\": \"set\", \"name\": …, \"sources\": [ … ] }",
            );
            continue;
        };
        if names.iter().any(|n| n == name) {
            reader.problem(
                &format!("{at}/name"),
                format!(
                    "The name {} is used by another entry of `resolutionOrder`.",
                    quote(name)
                ),
                "a name no other entry uses",
            );
            continue;
        }
        names.push(name.to_owned());
        if kind == "set" {
            if let Some(tree) = reader.set(item, &at) {
                resolver.order.push(Entry::Tree(tree));
            }
        } else if let Some(def) = reader.modifier(name, item, &at) {
            if resolver.modifiers.iter().any(|m| m.name == name) {
                reader.problem(
                    &format!("{at}/name"),
                    format!("Two modifiers are named {}.", quote(name)),
                    "a name no other modifier uses",
                );
                continue;
            }
            resolver.modifiers.push(def);
            resolver
                .order
                .push(Entry::Modifier(resolver.modifiers.len() - 1));
        }
    }
    let contexts: usize = resolver.modifiers.iter().map(|m| m.contexts.len()).sum();
    if contexts > MAX_CONTEXTS {
        reader.problem(
            "/modifiers",
            format!("The resolver has {contexts} contexts; a project has at most {MAX_CONTEXTS}."),
            "at most 64 contexts across all modifiers",
        );
        resolver.modifiers.iter_mut().for_each(|m| {
            let default = m.default.clone();
            m.contexts.retain(|c, _| *c == default);
        });
    }
    resolver
}
