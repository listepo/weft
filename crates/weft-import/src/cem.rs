//! Custom Elements Manifest → Weft catalog (SPEC §9, "From a Custom Elements Manifest").
//!
//! The manifest is read as schema 2.x of `custom-elements-manifest` (2.1.0, npm, published
//! 2024-05-06, https://github.com/webcomponents/custom-elements-manifest). The result is a
//! catalog extension (SPEC §10.4), not a format of its own: each custom element is a kind, its
//! attributes and public fields are props, its slots are slots and its events are events. What a
//! catalog cannot hold is listed as losses, one entry per place.
//!
//! The manifest is untrusted: it is parsed as data, never evaluated, its size and its number of
//! elements are bounded (`W602`), and a name or a type it gives never reaches the catalog unless it
//! passes the grammar of SPEC §2 and §5, so the catalog always loads without diagnostics.

use std::collections::HashMap;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value as Json;
use weft_core::{
    Catalog, Code, ComponentDef, Content, Diagnostic, Map, PropDef, PropDefault, PropType, SlotDef,
    WEFT_VERSION, embedded_reference, has_non_xml_char, is_name, parse_json,
};

use crate::{Loss, LossKind, Losses, clean, squash};

pub const MAX_MANIFEST_LENGTH: usize = 10_000_000;
pub const MAX_KINDS: usize = 2_000;
/// Per list of one element: enough for any real component, and it bounds the merge of attributes
/// with fields.
pub const MAX_MEMBERS: usize = 1_000;
const MAX_DESCRIPTION: usize = 200;
const MAX_ENUM_VALUES: usize = 128;
const SHOWN: usize = 8;

pub struct CemOptions<'a> {
    pub name: &'a str,
    pub version: &'a str,
    /// The catalog the result extends (SPEC §10.4); an element whose tag it already has is left out,
    /// because an extension may only widen a kind.
    pub base: &'a Catalog,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CemImport {
    pub catalog: Catalog,
    pub losses: Vec<Loss>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Reads the text of a manifest. Never fails: input that cannot be read gives an empty catalog
/// and a `W601`, input over a limit is cut with a `W602`.
pub fn import_cem(text: &str, options: &CemOptions<'_>) -> CemImport {
    let mut run = Run::default();
    let components = run.read(text, options.base).unwrap_or_default();
    CemImport {
        catalog: Catalog {
            weft: WEFT_VERSION.to_owned(),
            name: options.name.to_owned(),
            version: options.version.to_owned(),
            components,
        },
        losses: run.losses.0,
        diagnostics: run.diagnostics,
    }
}

#[derive(Default)]
struct Run {
    losses: Losses,
    diagnostics: Vec<Diagnostic>,
}

/// What an element or the package carried that no catalog field holds, counted by name.
#[derive(Default)]
struct Dropped(IndexMap<String, usize>);

impl Dropped {
    /// Counts each of `keys` that `json` has: an array by its length, anything else as one.
    fn count(&mut self, json: &Json, keys: &[&str]) {
        for key in keys {
            self.add(
                key,
                json.get(key)
                    .map_or(0, |v| v.as_array().map_or(1, Vec::len)),
            );
        }
    }

    fn add(&mut self, what: &str, n: usize) {
        if n > 0 {
            *self.0.entry(what.to_owned()).or_default() += n;
        }
    }

    fn note(&self) -> Option<String> {
        let list = |(what, n): (&String, &usize)| match n {
            1 => what.clone(),
            n => format!("{what} ({n})"),
        };
        let list: Vec<String> = self.0.iter().map(list).collect();
        (!list.is_empty()).then(|| format!("not imported: {}", list.join(", ")))
    }
}

fn str_of<'a>(json: &'a Json, key: &str) -> Option<&'a str> {
    json.get(key).and_then(Json::as_str)
}

fn flag(json: &Json, key: &str) -> bool {
    json.get(key) == Some(&Json::Bool(true))
}

fn list<'a>(json: &'a Json, key: &str) -> &'a [Json] {
    json.get(key)
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A string that can sit in a catalog and be written into markup as it is.
fn is_safe(s: &str) -> bool {
    !has_non_xml_char(s) && embedded_reference(s).is_none()
}

/// The first sentence of the summary or the description, short enough for a model to read at a
/// glance; whitespace and characters markup cannot carry are gone. Empty when there is none.
fn describe(json: &Json) -> String {
    let summary = str_of(json, "summary").filter(|s| !s.trim().is_empty());
    let text = summary.or_else(|| str_of(json, "description"));
    let text = squash(&clean(text.unwrap_or_default()));
    let sentence = text.find(". ").map_or(text.as_str(), |i| &text[..=i]);
    let mut out: String = sentence.chars().take(MAX_DESCRIPTION).collect();
    if out.len() < sentence.len() {
        out.push('…');
    }
    out
}

/// `maxLength` → `max-length`: how an attribute name follows from a property or event name.
fn kebab(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() && !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// `id` and `role` have their own meaning on every element (SPEC §2.2), `weft` is the format
/// version, and `on-` starts an event.
fn is_prop_name(name: &str) -> bool {
    is_name(name) && !matches!(name, "id" | "role" | "weft") && !name.starts_with("on-")
}

fn is_number(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        && s.parse::<f64>().is_ok_and(f64::is_finite)
}

/// The content of a quoted string literal without escapes, which is all a default or an enum
/// value is read from.
fn unquote(s: &str) -> Option<&str> {
    let quoted = |q: char| s.strip_prefix(q)?.strip_suffix(q);
    let inner = quoted('\'').or_else(|| quoted('"'))?;
    (!inner.contains(['\'', '"', '\\'])).then_some(inner)
}

enum Ty {
    Boolean,
    Number,
    String,
    Enum(Vec<String>),
    Other,
}

/// The Weft type of a TypeScript type text; `None` when it names no type (`undefined` alone).
fn classify(text: &str) -> Option<Ty> {
    if text.contains(['<', '(', '{', '[', '&', '=', '?', '`']) {
        return Some(Ty::Other);
    }
    let (mut bools, mut numbers, mut strings) = (0, 0, 0);
    let mut literals: Vec<String> = vec![];
    for part in text.split('|').map(str::trim) {
        match part {
            "" | "undefined" | "null" | "void" => {}
            "boolean" | "true" | "false" => bools += 1,
            "number" => numbers += 1,
            "string" => strings += 1,
            _ => match unquote(part) {
                Some(value) => literals.push(value.to_owned()),
                None => return Some(Ty::Other),
            },
        }
    }
    let total = bools + numbers + strings + literals.len();
    Some(if total == 0 {
        return None;
    } else if bools == total {
        Ty::Boolean
    } else if numbers == total {
        Ty::Number
    } else if literals.len() == total {
        Ty::Enum(literals)
    } else if strings > 0 && bools + numbers == 0 {
        Ty::String
    } else {
        Ty::Other
    })
}

/// A prop being assembled from an attribute and the field behind it.
struct Candidate {
    key: String,
    field: Option<String>,
    /// Has an attribute in markup; a property alone is set from script.
    attribute: bool,
    description: String,
    ty: Option<String>,
    default: Option<String>,
    path: String,
}

impl Candidate {
    fn new(json: &Json, key: String, field: Option<&str>, attribute: bool, path: String) -> Self {
        Candidate {
            key,
            field: field.map(str::to_owned),
            attribute,
            description: describe(json),
            ty: json
                .get("type")
                .and_then(|t| str_of(t, "text"))
                .map(str::to_owned),
            default: str_of(json, "default").map(str::to_owned),
            path,
        }
    }

    /// Takes from a field what the attribute it belongs to did not say.
    fn fill(&mut self, field: Candidate) {
        self.field = self.field.take().or(field.field);
        self.ty = self.ty.take().or(field.ty);
        self.default = self.default.take().or(field.default);
        if self.description.is_empty() {
            self.description = field.description;
        }
    }
}

impl Run {
    fn loss(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    /// One loss for a list of names, the first few shown.
    fn listed(&mut self, kind: LossKind, path: &str, what: &str, names: &[String]) {
        if !names.is_empty() {
            let more = names.len().saturating_sub(SHOWN);
            let tail = if more > 0 {
                format!(" and {more} more")
            } else {
                String::new()
            };
            let shown = names[..names.len() - more].join(", ");
            self.loss(kind, path, format!("{what}: {shown}{tail}"));
        }
    }

    fn unreadable(&mut self, message: &str) {
        let expected = "a Custom Elements Manifest (JSON, schema 2.x)";
        self.diagnose(Code::W601, "#", message.to_owned(), expected.to_owned());
    }

    fn limit(&mut self, path: &str, what: &str) {
        let expected = format!("at most {MAX_MANIFEST_LENGTH} bytes, {MAX_KINDS} elements");
        let message = format!("The manifest {what}; the rest was not imported.");
        self.diagnose(Code::W602, path, message, expected);
    }

    /// A list of one element, cut at `MAX_MEMBERS`.
    fn members<'a>(&mut self, json: &'a Json, key: &str, at: &str) -> &'a [Json] {
        let all = list(json, key);
        if all.len() > MAX_MEMBERS {
            self.limit(
                at,
                &format!("has more than {MAX_MEMBERS} {key} in one element"),
            );
        }
        &all[..all.len().min(MAX_MEMBERS)]
    }

    fn diagnose(&mut self, code: Code, path: &str, message: String, expected: String) {
        self.diagnostics
            .push(Diagnostic::new(code, path, message, expected));
    }

    fn read(&mut self, text: &str, base: &Catalog) -> Option<Map<ComponentDef>> {
        if text.len() > MAX_MANIFEST_LENGTH {
            self.limit("#", &format!("is longer than {MAX_MANIFEST_LENGTH} bytes"));
            return None;
        }
        let json = match parse_json(text) {
            Ok(json) => json,
            Err(e) => {
                self.unreadable(&format!("The manifest is not JSON: {e}."));
                return None;
            }
        };
        let version = str_of(&json, "schemaVersion").unwrap_or_default();
        if !json.get("modules").is_some_and(Json::is_array) {
            self.unreadable("The manifest has no `modules` list.");
            return None;
        }
        if version.split('.').next() != Some("2") {
            self.unreadable(&format!(
                "The manifest has schema version {version:?}; only 2.x is read."
            ));
            return None;
        }
        let modules = list(&json, "modules");
        // The tag of a class is named by an export, not by the class.
        let mut tags: HashMap<(&str, &str), &str> = HashMap::new();
        for module in modules {
            let path = str_of(module, "path").unwrap_or_default();
            for export in list(module, "exports") {
                let class = export.get("declaration").unwrap_or(&Json::Null);
                if str_of(export, "kind") == Some("custom-element-definition")
                    && class.get("package").is_none()
                    && let (Some(tag), Some(name)) = (str_of(export, "name"), str_of(class, "name"))
                {
                    tags.entry((str_of(class, "module").unwrap_or(path), name))
                        .or_insert(tag);
                }
            }
        }
        let mut package = Dropped::default();
        package.count(&json, &["readme", "deprecated"]);
        let mut kinds = Map::new();
        'modules: for (m, module) in modules.iter().enumerate() {
            let path = str_of(module, "path").unwrap_or_default();
            package.count(module, &["summary", "description", "deprecated"]);
            let js = list(module, "exports")
                .iter()
                .filter(|e| str_of(e, "kind") == Some("js"));
            package.add("exports", js.count());
            for (d, decl) in list(module, "declarations").iter().enumerate() {
                let at = format!("#/modules/{m}/declarations/{d}");
                if !flag(decl, "customElement") {
                    package.add("declarations that are not elements", 1);
                } else if str_of(decl, "kind") != Some("class") {
                    self.loss(
                        LossKind::Kinds,
                        &at,
                        "a custom element mixin is not an element; not imported",
                    );
                } else if kinds.len() >= MAX_KINDS {
                    self.limit(&at, &format!("has more than {MAX_KINDS} elements"));
                    break 'modules;
                } else {
                    let class = str_of(decl, "name").unwrap_or_default();
                    let tag = str_of(decl, "tagName").or_else(|| tags.get(&(path, class)).copied());
                    if let Some((tag, def)) = self.element(decl, &at, tag, base, &kinds) {
                        kinds.insert(tag, def);
                    }
                }
            }
        }
        if !kinds.is_empty() {
            self.loss(
                LossKind::Structure,
                "#",
                "a manifest has no ARIA roles: every kind has the role `generic`",
            );
        }
        if let Some(note) = package.note() {
            self.loss(LossKind::Structure, "#", note);
        }
        Some(kinds)
    }

    fn element(
        &mut self,
        decl: &Json,
        at: &str,
        tag: Option<&str>,
        base: &Catalog,
        kinds: &Map<ComponentDef>,
    ) -> Option<(String, ComponentDef)> {
        let tag = tag.unwrap_or_default();
        let refusal = if tag.is_empty() {
            "the element has no tag name (no `tagName`, no custom-element-definition export)"
                .to_owned()
        } else if !is_name(tag) || matches!(tag, "each" | "slot") {
            format!("the tag {tag:?} is not a Weft name (lowercase letters, digits and hyphens)")
        } else if tag.starts_with("x-") {
            format!("the tag {tag:?} starts with `x-`, which every catalog treats as opaque")
        } else if base.components.contains_key(tag) {
            format!("the tag {tag:?} is already a kind of the catalog it extends")
        } else if kinds.contains_key(tag) {
            format!("the tag {tag:?} is defined twice; the first definition is kept")
        } else {
            String::new()
        };
        if !refusal.is_empty() {
            self.loss(LossKind::Kinds, at, format!("{refusal}; not imported"));
            return None;
        }
        let mut dropped = Dropped::default();
        let ignored = ["cssParts", "cssProperties", "cssStates", "demos", "mixins"];
        dropped.count(decl, &ignored);
        dropped.count(decl, &["superclass", "source", "deprecated"]);
        let props = self.props(decl, at, &mut dropped);
        let (slots, default_slot) = self.slots(decl, at);
        let events = self.events(decl, at, &mut dropped);
        if let Some(note) = dropped.note() {
            self.loss(LossKind::Structure, at, note);
        }
        let description = describe(decl);
        let def = ComponentDef {
            description: if description.is_empty() {
                format!("The <{tag}> custom element.")
            } else {
                description
            },
            role: "generic".to_owned(),
            content: if default_slot {
                Content::Mixed
            } else {
                Content::None
            },
            allowed_children: None,
            allowed_parents: None,
            requires_label: None,
            props: (!props.is_empty()).then_some(props),
            slots: (!slots.is_empty()).then_some(slots),
            states: None,
            events: (!events.is_empty()).then_some(events),
        };
        Some((tag.to_owned(), def))
    }

    /// The named slots, and whether the element takes content: an element that documents no
    /// slots may still take children, and a manifest cannot say.
    fn slots(&mut self, decl: &Json, at: &str) -> (Map<SlotDef>, bool) {
        let mut slots = Map::new();
        if decl.get("slots").is_none() {
            self.loss(
                LossKind::Slots,
                at,
                "the element documents no slots; its content is `mixed`",
            );
            return (slots, true);
        }
        let mut default_slot = false;
        for (i, slot) in self.members(decl, "slots", at).iter().enumerate() {
            let at = format!("{at}/slots/{i}");
            match str_of(slot, "name") {
                Some("") => default_slot = true,
                Some(name) if is_name(name) && !slots.contains_key(name) => {
                    let mut description = describe(slot);
                    if description.is_empty() {
                        description = format!("The `{name}` slot.");
                    }
                    let def = SlotDef {
                        description,
                        allowed_children: None,
                        required: None,
                    };
                    slots.insert(name.to_owned(), def);
                }
                name => {
                    let note = format!(
                        "the slot {:?} is not a Weft name or is repeated",
                        name.unwrap_or_default()
                    );
                    self.loss(LossKind::Slots, &at, format!("{note}; not imported"));
                }
            }
        }
        (slots, default_slot)
    }

    fn events(&mut self, decl: &Json, at: &str, dropped: &mut Dropped) -> Vec<String> {
        let mut events: Vec<String> = vec![];
        for (i, event) in self.members(decl, "events", at).iter().enumerate() {
            let at = format!("{at}/events/{i}");
            dropped.count(event, &["deprecated", "inheritedFrom"]);
            dropped.add("event types", usize::from(event.get("type").is_some()));
            let name = str_of(event, "name").unwrap_or_default();
            let key = kebab(name);
            if !is_name(&key) || events.contains(&key) {
                let note =
                    format!("the event {name:?} is not a Weft name or is repeated; not imported");
                self.loss(LossKind::Names, &at, note);
            } else {
                if key != name {
                    self.loss(
                        LossKind::Names,
                        &at,
                        format!("the event {name:?} is `on-{key}`"),
                    );
                }
                events.push(key);
            }
        }
        events
    }

    fn props(&mut self, decl: &Json, at: &str, dropped: &mut Dropped) -> Map<PropDef> {
        let mut found: Vec<Candidate> = vec![];
        for (i, attribute) in self.members(decl, "attributes", at).iter().enumerate() {
            let at = format!("{at}/attributes/{i}");
            dropped.count(attribute, &["deprecated", "inheritedFrom"]);
            let key = str_of(attribute, "name").unwrap_or_default();
            if !is_prop_name(key) || found.iter().any(|c| c.key == key) {
                let note = format!(
                    "the attribute {key:?} is not a Weft prop name or is repeated; not imported"
                );
                self.loss(LossKind::Names, &at, note);
                continue;
            }
            let field = str_of(attribute, "fieldName");
            found.push(Candidate::new(attribute, key.to_owned(), field, true, at));
        }
        let mut skipped = vec![];
        for (i, member) in self.members(decl, "members", at).iter().enumerate() {
            let at = format!("{at}/members/{i}");
            let name = str_of(member, "name").unwrap_or_default();
            let private = matches!(str_of(member, "privacy"), Some("private" | "protected"));
            let skip = match str_of(member, "kind") {
                Some("field") if flag(member, "readonly") => Some("read-only"),
                Some("field") if private => Some("private"),
                Some("field") if flag(member, "static") => Some("static"),
                Some("field") => None,
                _ => Some("method"),
            };
            if let Some(why) = skip {
                skipped.push(format!("{why} {name}"));
                continue;
            }
            dropped.count(member, &["deprecated", "inheritedFrom", "reflects"]);
            let attribute = str_of(member, "attribute");
            let key = attribute.map_or_else(|| kebab(name), str::to_owned);
            let field = Candidate::new(member, key, Some(name), attribute.is_some(), at.clone());
            let same = |c: &Candidate| c.field.as_deref() == Some(name) || c.key == field.key;
            if let Some(c) = found.iter_mut().find(|c| same(c)) {
                c.fill(field);
            } else if !is_prop_name(&field.key) {
                let note = format!("the property {name:?} is not a Weft prop name; not imported");
                self.loss(LossKind::Names, &at, note);
            } else {
                found.push(field);
            }
        }
        self.listed(LossKind::Props, at, "members not imported", &skipped);
        let (mut props, mut script_only) = (Map::new(), vec![]);
        for c in found {
            if let Some(def) = self.prop(&c) {
                if !c.attribute {
                    let field = c.field.as_deref().unwrap_or_default();
                    script_only.push(if c.key == field {
                        c.key.clone()
                    } else {
                        format!("{field} → {}", c.key)
                    });
                }
                props.insert(c.key, def);
            }
        }
        let what = "properties without an attribute, imported as props (kebab-case)";
        self.listed(LossKind::Props, at, what, &script_only);
        props
    }

    fn prop(&mut self, c: &Candidate) -> Option<PropDef> {
        let shown = c.ty.as_deref().unwrap_or("(none)");
        let default = c.default.as_deref().map(str::trim);
        let default = default.filter(|d| !matches!(*d, "" | "undefined" | "null"));
        let ty = match c.ty.as_deref().map(classify) {
            Some(Some(ty)) => ty,
            // No type: the default's literal says what it is, else an attribute is text.
            _ => match default {
                Some("true" | "false") => Ty::Boolean,
                Some(d) if is_number(d) => Ty::Number,
                Some(d) if unquote(d).is_some() => Ty::String,
                _ if c.attribute => Ty::String,
                _ => Ty::Other,
            },
        };
        let (kind, values) = match ty {
            Ty::Boolean => (PropType::Boolean, None),
            Ty::Number => (PropType::Number, None),
            Ty::String => (PropType::String, None),
            Ty::Enum(values)
                if values.len() <= MAX_ENUM_VALUES && values.iter().all(|v| is_safe(v)) =>
            {
                let unique: IndexSet<String> = values.into_iter().collect();
                (PropType::Enum, Some(Vec::from_iter(unique)))
            }
            Ty::Enum(_) | Ty::Other if c.attribute => {
                let note = format!(
                    "the type `{shown}` is not a Weft prop type; the attribute is read as a string"
                );
                self.loss(LossKind::Props, &c.path, note);
                (PropType::String, None)
            }
            Ty::Enum(_) | Ty::Other => {
                let note = format!(
                    "the property {:?} has the type `{shown}`, which a prop cannot hold; not imported",
                    c.key
                );
                self.loss(LossKind::Props, &c.path, note);
                return None;
            }
        };
        let fallback = format!(
            "The `{}` {}.",
            c.key,
            if c.attribute { "attribute" } else { "property" }
        );
        let description = if c.description.is_empty() {
            &fallback
        } else {
            &c.description
        };
        let mut def = PropDef::new(description, kind);
        def.default = default.and_then(|d| {
            let value = match kind {
                PropType::Boolean => {
                    matches!(d, "true" | "false").then(|| PropDefault::Bool(d == "true"))
                }
                PropType::Number => d
                    .parse()
                    .ok()
                    .filter(|_| is_number(d))
                    .map(PropDefault::Number),
                _ => unquote(d)
                    .filter(|v| is_safe(v))
                    .filter(|v| values.as_ref().is_none_or(|all| all.iter().any(|a| a == v)))
                    .map(|v| PropDefault::String(v.to_owned())),
            };
            if value.is_none() {
                let note =
                    format!("the default `{d}` is not a literal of the prop's type; not imported");
                self.loss(LossKind::Values, &c.path, note);
            }
            value
        });
        def.values = values;
        Some(def)
    }
}
