//! Patches (SPEC §7): an agent edits a document by id instead of rewriting it. The whole list is
//! applied to a copy and validated, so a rejected list leaves no trace and the caller's document
//! is never touched.

use indexmap::{IndexMap, IndexSet};
use serde_json::Value as Json;

use crate::canonical::canonicalize;
use crate::context_patch::{Author, ContextPatch, apply_context, dangling_hint};
use crate::diagnostics::{
    Code, Diagnostic, Mode, did_you_mean, has_errors, nearest, one_of, quote,
};
use crate::fragment::{FRAGMENT, USE};
use crate::inline;
use crate::model::{Catalog, Child, Content, Document, Map, Node, Value};
use crate::parse::{ParseOptions, parse, parse_fragment};
use crate::rules::{EACH, is_name};
use crate::shape::{patch_issues, to_entry, value_of};
use crate::validate::{ValidateOptions, validate_document};

#[derive(Clone, Copy)]
pub struct ApplyOptions<'a> {
    pub catalog: &'a Catalog,
    pub mode: Mode,
    pub tokens: Option<&'a IndexMap<String, String>>,
    pub actions: Option<&'a [String]>,
    /// What `add-context` may claim; set by the host, never by the model.
    pub author: Option<&'a Author>,
    /// Refuses every context patch (`W512`).
    pub read_only_context: bool,
}

impl<'a> ApplyOptions<'a> {
    pub fn new(catalog: &'a Catalog) -> Self {
        ApplyOptions {
            catalog,
            mode: Mode::default(),
            tokens: None,
            actions: None,
            author: None,
            read_only_context: false,
        }
    }

    fn validate(&self) -> ValidateOptions<'a> {
        ValidateOptions {
            catalog: Some(self.catalog),
            mode: self.mode,
            tokens: self.tokens,
            actions: self.actions,
        }
    }
}

pub struct PatchResult {
    /// Canonical result; absent when anything was rejected. Holds no errors, possibly warnings.
    pub document: Option<Document>,
    pub diagnostics: Vec<Diagnostic>,
}

const FORMS: [(&str, &str); 11] = [
    (
        "set",
        r#"{"op":"set","id":"…","fragment"?:"…","prop":"…","value":<literal|{bind}|{token}|null>}"#,
    ),
    (
        "insert",
        r#"{"op":"insert","parent":"…","fragment"?:"…","slot"?:"…","index"?:0,"markup":"<…/>"}"#,
    ),
    ("remove", r#"{"op":"remove","id":"…","fragment"?:"…"}"#),
    (
        "move",
        r#"{"op":"move","id":"…","fragment"?:"…","parent":"…","slot"?:"…","index"?:0}"#,
    ),
    (
        "add-context",
        r#"{"op":"add-context","entry":{"id":"…","kind":"…","by":"agent","name":"…","for"?:"…","status"?:"open","text":"…"}}"#,
    ),
    (
        "set-context",
        r#"{"op":"set-context","id":"…","field":"text"|"kind"|"for","value":"…"|null}"#,
    ),
    ("resolve-context", r#"{"op":"resolve-context","id":"…"}"#),
    ("remove-context", r#"{"op":"remove-context","id":"…"}"#),
    (
        "add-fragment",
        r#"{"op":"add-fragment","markup":"<fragment name=\"…\">…</fragment>"}"#,
    ),
    ("remove-fragment", r#"{"op":"remove-fragment","name":"…"}"#),
    (
        "set-version",
        r#"{"op":"set-version","value":"MAJOR.MINOR.PATCH"|null}"#,
    ),
];

fn form(op: &str) -> Option<&'static str> {
    FORMS.iter().find(|(name, _)| *name == op).map(|(_, f)| *f)
}

enum Patch {
    Set {
        id: String,
        fragment: Option<String>,
        prop: String,
        value: Option<Value>,
    },
    Insert {
        parent: String,
        fragment: Option<String>,
        slot: Option<String>,
        index: Option<u64>,
        markup: String,
    },
    Remove {
        id: String,
        fragment: Option<String>,
    },
    Move {
        id: String,
        fragment: Option<String>,
        parent: String,
        slot: Option<String>,
        index: Option<u64>,
    },
    AddFragment {
        markup: String,
    },
    RemoveFragment {
        name: String,
    },
    Context(ContextPatch),
    SetVersion {
        value: Option<String>,
    },
}

impl Patch {
    /// The inline fragment this patch addresses, when it has one.
    fn fragment(&self) -> Option<&str> {
        match self {
            Patch::Set { fragment, .. }
            | Patch::Insert { fragment, .. }
            | Patch::Remove { fragment, .. }
            | Patch::Move { fragment, .. } => fragment.as_deref(),
            _ => None,
        }
    }
}

/// Reads a patch that passed [`patch_issues`].
fn to_patch(v: &Json) -> Patch {
    let s = |k: &str| {
        v.get(k)
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let opt = |k: &str| v.get(k).and_then(Json::as_str).map(str::to_owned);
    // The shape check admitted only safe non-negative integers, which fit u64 exactly; usize is
    // 32 bits on WebAssembly.
    let index = v.get("index").and_then(Json::as_f64).map(|n| n as u64);
    match v.get("op").and_then(Json::as_str) {
        Some("set") => Patch::Set {
            id: s("id"),
            fragment: opt("fragment"),
            prop: s("prop"),
            value: v.get("value").filter(|x| !x.is_null()).map(value_of),
        },
        Some("insert") => Patch::Insert {
            parent: s("parent"),
            fragment: opt("fragment"),
            slot: opt("slot"),
            index,
            markup: s("markup"),
        },
        Some("remove") => Patch::Remove {
            id: s("id"),
            fragment: opt("fragment"),
        },
        Some("add-fragment") => Patch::AddFragment {
            markup: s("markup"),
        },
        Some("remove-fragment") => Patch::RemoveFragment { name: s("name") },
        Some("add-context") => Patch::Context(ContextPatch::Add(
            v.get("entry").map(to_entry).unwrap_or_default(),
        )),
        Some("set-context") => Patch::Context(ContextPatch::Set {
            id: s("id"),
            field: s("field"),
            value: opt("value"),
        }),
        Some("resolve-context") => Patch::Context(ContextPatch::Resolve(s("id"))),
        Some("remove-context") => Patch::Context(ContextPatch::Remove(s("id"))),
        Some("set-version") => Patch::SetVersion {
            value: v.get("value").and_then(Json::as_str).map(str::to_owned),
        },
        _ => Patch::Move {
            id: s("id"),
            fragment: opt("fragment"),
            parent: s("parent"),
            slot: opt("slot"),
            index,
        },
    }
}

pub fn apply_patches(
    document: &Document,
    patches: &Json,
    options: &ApplyOptions<'_>,
) -> PatchResult {
    let read = match read_patches(patches) {
        Ok(read) => read,
        Err(diagnostics) => {
            return PatchResult {
                document: None,
                diagnostics,
            };
        }
    };
    let mut work = document.clone();
    for (i, patch) in read.iter().enumerate() {
        let outcome = match patch {
            Patch::Context(c) => {
                let ids = all_ids(&work.root);
                apply_context(
                    &mut work,
                    c,
                    ids,
                    options.author,
                    options.read_only_context,
                    i,
                )
            }
            Patch::AddFragment { markup } => add_fragment(&mut work, markup, i, options),
            Patch::RemoveFragment { name } => remove_fragment(&mut work, name, i),
            // The document root has no id, so the version is not an element patch.
            Patch::SetVersion { value } => {
                work.version = value.clone();
                Ok(())
            }
            _ => {
                let fragment = patch.fragment().map(str::to_owned);
                if let Some(name) = &fragment
                    && !work.fragments.contains_key(name)
                {
                    return PatchResult {
                        document: None,
                        diagnostics: unknown_fragment(&work, name, i),
                    };
                }
                let inline = work.fragments.clone();
                let root = match &fragment {
                    // The name was checked against the map just above.
                    Some(name) => match work.fragments.get_mut(name) {
                        Some(node) => node,
                        None => {
                            return PatchResult {
                                document: None,
                                diagnostics: vec![],
                            };
                        }
                    },
                    None => &mut work.root,
                };
                apply_one(root, patch, i, options, fragment.as_deref(), &inline)
            }
        };
        if let Err(failure) = outcome {
            return PatchResult {
                document: None,
                diagnostics: failure,
            };
        }
    }
    let result = canonicalize(&work);
    let mut diagnostics = validate_document(&result, &options.validate());
    diagnostics.iter_mut().for_each(dangling_hint);
    let document = (!has_errors(&diagnostics)).then_some(result);
    PatchResult {
        document,
        diagnostics,
    }
}

fn type_of(v: &Json) -> &'static str {
    match v {
        Json::Null | Json::Object(_) | Json::Array(_) => "object",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
    }
}

fn read_patches(patches: &Json) -> Result<Vec<Patch>, Vec<Diagnostic>> {
    let Json::Array(items) = patches else {
        let expected = format!("an array such as {}", form("remove").unwrap_or_default());
        return Err(vec![
            Diagnostic::new(
                Code::W501,
                "#/patches",
                "The patch list must be an array of patches.",
                expected,
            )
            .got(type_of(patches)),
        ]);
    };
    let mut diagnostics = Vec::new();
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let issues = patch_issues(item);
        if issues.is_empty() {
            out.push(to_patch(item));
            continue;
        }
        let form = item.get("op").and_then(Json::as_str).and_then(form);
        for issue in issues.iter().take(3) {
            let rest = issue.path_string();
            let path = if issue.path.is_empty() {
                format!("#/patches/{i}")
            } else {
                format!("#/patches/{i}/{rest}")
            };
            let expected =
                form.map_or_else(|| one_of(FORMS.iter().map(|(n, _)| *n)), str::to_owned);
            let hint = form.map_or_else(
                || format!("\"op\" must be {}", one_of(FORMS.iter().map(|(n, _)| *n))),
                |f| format!("write the patch as {f}"),
            );
            diagnostics.push(
                Diagnostic::new(
                    Code::W501,
                    path,
                    format!("Patch {i} is malformed: {}.", issue.message),
                    expected,
                )
                .hint(hint),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(out)
    } else {
        Err(diagnostics)
    }
}

#[derive(Clone, PartialEq)]
enum ListKey {
    Children,
    Slot(String),
}

/// Where a node sits: the list and index at each level below the root; empty for the root.
type Loc = Vec<(ListKey, usize)>;

fn child_lists(node: &Node) -> impl Iterator<Item = (ListKey, &Vec<Child>)> {
    std::iter::once((ListKey::Children, &node.children)).chain(
        node.slots
            .iter()
            .map(|(name, list)| (ListKey::Slot(name.clone()), list)),
    )
}

fn list_mut<'a>(node: &'a mut Node, key: &ListKey) -> Option<&'a mut Vec<Child>> {
    match key {
        ListKey::Children => Some(&mut node.children),
        ListKey::Slot(name) => node.slots.get_mut(name),
    }
}

fn node_ref<'a>(root: &'a Node, loc: &[(ListKey, usize)]) -> Option<&'a Node> {
    let mut node = root;
    for (key, index) in loc {
        let list = match key {
            ListKey::Children => Some(&node.children),
            ListKey::Slot(name) => node.slots.get(name),
        };
        node = list?.get(*index)?.as_node()?;
    }
    Some(node)
}

fn node_mut<'a>(root: &'a mut Node, loc: &[(ListKey, usize)]) -> Option<&'a mut Node> {
    let mut node = root;
    for (key, index) in loc {
        node = match list_mut(node, key)?.get_mut(*index)? {
            Child::Node(n) => n,
            Child::Text(_) => return None,
        };
    }
    Some(node)
}

/// Document order, the root included; the first element with the id wins.
fn find(top: &Node, id: &str) -> Option<Loc> {
    if top.id.as_deref() == Some(id) {
        return Some(vec![]);
    }
    for (key, list) in child_lists(top) {
        for (index, child) in list.iter().enumerate() {
            let Child::Node(child) = child else { continue };
            if let Some(mut deeper) = find(child, id) {
                deeper.insert(0, (key.clone(), index));
                return Some(deeper);
            }
        }
    }
    None
}

fn ids(top: &Node, into: &mut IndexSet<String>) {
    if let Some(id) = &top.id {
        into.insert(id.clone());
    }
    for (_, list) in child_lists(top) {
        for child in list {
            if let Child::Node(n) = child {
                ids(n, into);
            }
        }
    }
}

fn all_ids(top: &Node) -> IndexSet<String> {
    let mut set = IndexSet::new();
    ids(top, &mut set);
    set
}

fn free_id(id: &str, taken: &IndexSet<String>) -> String {
    let mut n = 2;
    while taken.contains(&format!("{id}-{n}")) {
        n += 1;
    }
    format!("{id}-{n}")
}

/// Where an id that is not on the screen actually lives, as a repair hint.
fn held_fragment(top: &Node, id: &str, catalog: &Catalog, inline: &Map<Node>) -> Option<String> {
    let local = id.rsplit('/').next().unwrap_or(id);
    if let Some(name) = inline
        .iter()
        .find(|(_, node)| all_ids(node).contains(local))
        .map(|(name, _)| name.clone())
    {
        return Some(format!(
            "{} is inside inline fragment \"{name}\"; address it with fragment \"{name}\"",
            quote(id)
        ));
    }
    fragment_holding(top, id, catalog).map(|name| {
        format!(
            "{} is inside fragment \"{name}\"; edit the fragment, or set a parameter of its <use>",
            quote(id)
        )
    })
}

fn unknown_fragment(document: &Document, name: &str, i: usize) -> Vec<Diagnostic> {
    let names: Vec<&str> = document.fragments.keys().map(String::as_str).collect();
    let hint = nearest(name, names.iter().copied())
        .map(|n| format!("did you mean {}?", quote(&n)))
        .unwrap_or_else(|| "an inline fragment of this screen".to_owned());
    vec![
        Diagnostic::new(
            Code::W502,
            format!("#/patches/{i}/fragment"),
            format!("No inline fragment is named {}.", quote(name)),
            "the name of an inline fragment",
        )
        .got(name)
        .hint(hint),
    ]
}

fn remove_fragment(document: &mut Document, name: &str, i: usize) -> Outcome {
    if document.fragments.shift_remove(name).is_none() {
        let names: Vec<&str> = document.fragments.keys().map(String::as_str).collect();
        let hint = nearest(name, names.iter().copied())
            .map(|n| format!("did you mean {}?", quote(&n)))
            .unwrap_or_else(|| "an inline fragment of this screen".to_owned());
        return Err(vec![
            Diagnostic::new(
                Code::W502,
                format!("#/patches/{i}/name"),
                format!("No inline fragment is named {}.", quote(name)),
                "the name of an inline fragment",
            )
            .got(name)
            .hint(hint),
        ]);
    }
    // A `<use>` that still names it fails validation of the result (`W801`).
    Ok(())
}

/// Reads `<fragment name="…">` markup and adds it. A name the screen already has is `W513`;
/// a name the project has is `W808`, and the fragment is not added.
fn add_fragment(
    document: &mut Document,
    markup: &str,
    i: usize,
    options: &ApplyOptions<'_>,
) -> Outcome {
    let at = format!("#/patches/{i}/markup");
    let parsed = parse(markup, &ParseOptions::default());
    if has_errors(&parsed.diagnostics) || parsed.document.is_none() {
        let mut diagnostics = parsed.diagnostics;
        if diagnostics.is_empty() {
            diagnostics.push(Diagnostic::new(
                Code::W808,
                at.clone(),
                "add-fragment markup must be one <fragment name=\"…\"> element.",
                "<fragment name=\"…\">…</fragment>",
            ));
        }
        for d in &mut diagnostics {
            if !d.path.starts_with("#/patches/") {
                d.path = format!("{at}{}", d.path);
            }
        }
        return Err(diagnostics);
    }
    let Some(built) = parsed.document else {
        return Err(vec![Diagnostic::new(
            Code::W808,
            at,
            "add-fragment markup must be one <fragment name=\"…\"> element.",
            "<fragment name=\"…\">…</fragment>",
        )]);
    };
    if !built.weft.is_empty() || built.root.kind != FRAGMENT || built.root.id.is_some() {
        return Err(vec![Diagnostic::new(
            Code::W808,
            at,
            "An inline <fragment> takes name, and optionally label and version.",
            "<fragment name=\"…\">…</fragment>",
        )]);
    }
    let mut node = built.root;
    let name = match node.props.shift_remove("name") {
        Some(Value::String(name)) if is_name(&name) => name,
        _ => {
            return Err(vec![Diagnostic::new(
                Code::W808,
                at,
                "An inline <fragment> needs a name that matches the name grammar.",
                "name=\"…\"",
            )]);
        }
    };
    if node.props.keys().any(|k| k != "label" && k != "version")
        || node
            .props
            .get("label")
            .is_some_and(|v| !matches!(v, Value::String(_)))
        || !node.on.is_empty()
    {
        return Err(vec![Diagnostic::new(
            Code::W808,
            at,
            "An inline <fragment> takes only name, label and version.",
            "<fragment name=\"…\" label=\"…\">",
        )]);
    }
    if document.fragments.contains_key(&name) {
        return Err(vec![
            Diagnostic::new(
                Code::W513,
                format!("#/patches/{i}/markup"),
                format!("Inline fragment \"{name}\" is already in this document."),
                "a name this screen does not use yet",
            )
            .got(name),
        ]);
    }
    if options.catalog.fragments.contains_key(&name) {
        return Err(vec![
            Diagnostic::new(
                Code::W808,
                format!("#/patches/{i}/markup"),
                format!("\"{name}\" is already a fragment of the project."),
                "a name the project does not use",
            )
            .got(name),
        ]);
    }
    let mut known = document.fragments.clone();
    known.insert(name.clone(), node.clone());
    inline::retype(&mut node, &known, Some(options.catalog));
    document.fragments.insert(name, node);
    Ok(())
}

/// The fragment used by the document whose body has `id`, given locally or as an instance path
/// (SPEC §10.7): patches never reach into a project fragment, so the hint names the file to edit.
fn fragment_holding(top: &Node, id: &str, catalog: &Catalog) -> Option<String> {
    if catalog.fragments.is_empty() {
        return None;
    }
    let local = id.rsplit('/').next().unwrap_or(id);
    let mut used = IndexSet::new();
    uses(top, &mut used);
    used.into_iter().find(|name| {
        catalog
            .fragments
            .get(name)
            .is_some_and(|f| all_ids(&f.document.root).contains(local))
    })
}

fn uses(top: &Node, into: &mut IndexSet<String>) {
    if let (USE, Some(Value::String(name))) = (top.kind.as_str(), top.props.get(FRAGMENT)) {
        into.insert(name.clone());
    }
    for (_, list) in child_lists(top) {
        for child in list {
            if let Child::Node(n) = child {
                uses(n, into);
            }
        }
    }
}

type Outcome = Result<(), Vec<Diagnostic>>;

fn apply_one(
    root: &mut Node,
    patch: &Patch,
    i: usize,
    options: &ApplyOptions<'_>,
    fragment: Option<&str>,
    inline: &Map<Node>,
) -> Outcome {
    let at = |field: &str| format!("#/patches/{i}/{field}");
    // Inside an inline fragment the fragment's own name addresses that `<fragment>` element,
    // which has no id of its own, so `version` and `label` can be set there.
    let find_or_fail = |root: &Node, id: &str, field: &str| -> Result<Loc, Vec<Diagnostic>> {
        let found = if fragment == Some(id) {
            Some(vec![])
        } else {
            find(root, id)
        };
        found.ok_or_else(|| {
            let hint = if fragment.is_some() {
                let mut ids = all_ids(root);
                if let Some(name) = fragment {
                    ids.insert(name.to_owned());
                }
                did_you_mean(id, &ids).unwrap_or_else(|| "copy an id from the fragment".into())
            } else {
                held_fragment(root, id, options.catalog, inline)
                    .or_else(|| did_you_mean(id, all_ids(root)))
                    .unwrap_or_else(|| "copy an id from the document".to_owned())
            };
            vec![
                Diagnostic::new(
                    Code::W502,
                    at(field),
                    format!("No element has the id {}.", quote(id)),
                    "the id of an element in the document",
                )
                .got(id)
                .hint(hint),
            ]
        })
    };

    let (parent_id, slot, index, moved) = match patch {
        Patch::Context(_)
        | Patch::AddFragment { .. }
        | Patch::RemoveFragment { .. }
        | Patch::SetVersion { .. } => {
            return Ok(());
        }
        Patch::Set {
            id, prop, value, ..
        } => {
            let loc = find_or_fail(root, id, "id")?;
            let is_root = loc.is_empty();
            let Some(node) = node_mut(root, &loc) else {
                return Ok(());
            };
            return set_prop(
                node,
                prop,
                value.as_ref(),
                is_root,
                &at("prop"),
                options.catalog,
            );
        }
        Patch::Remove { id, .. } => {
            let loc = find_or_fail(root, id, "id")?;
            let Some((last, parent_loc)) = loc.split_last() else {
                return Err(root_failure("removed", at("id")));
            };
            if let Some(list) = node_mut(root, parent_loc).and_then(|p| list_mut(p, &last.0)) {
                list.remove(last.1);
            }
            return Ok(());
        }
        Patch::Move {
            id,
            parent,
            slot,
            index,
            ..
        } => {
            find_or_fail(root, parent, "parent")?;
            let loc = find_or_fail(root, id, "id")?;
            let Some((last, from_loc)) = loc.split_last() else {
                return Err(root_failure("moved", at("id")));
            };
            if node_ref(root, &loc).is_some_and(|n| find(n, parent).is_some()) {
                return Err(vec![
                    Diagnostic::new(
                        Code::W506,
                        at("parent"),
                        format!(
                            "Element {} cannot move into itself or one of its descendants.",
                            quote(id)
                        ),
                        "a parent outside the moved element",
                    )
                    .got(parent.clone()),
                ]);
            }
            // The index counts the target list after the element left it, so a move inside one
            // list reads the same as the resulting order.
            let removed = node_mut(root, from_loc)
                .and_then(|p| list_mut(p, &last.0))
                .map(|list| list.remove(last.1));
            (parent, slot, index, removed.into_iter().collect::<Vec<_>>())
        }
        Patch::Insert {
            parent,
            slot,
            index,
            markup,
            ..
        } => {
            find_or_fail(root, parent, "parent")?;
            let nodes = read_fragment(markup, root, options.catalog, &at("markup"))?;
            (
                parent,
                slot,
                index,
                nodes
                    .into_iter()
                    .map(|n| Child::Node(Box::new(n)))
                    .collect(),
            )
        }
    };
    // The moved element held no element with the parent's id, so the first match is unchanged.
    let Some(target) = find(root, parent_id).and_then(|loc| node_mut(root, &loc)) else {
        return Ok(());
    };
    insert_into(target, slot.as_deref(), *index, options.catalog, &at, moved)
}

fn root_failure(verb: &str, path: String) -> Vec<Diagnostic> {
    let mut d = Diagnostic::new(
        Code::W507,
        path,
        format!("The root element cannot be {verb}."),
        "the id of an element below the root",
    );
    if verb == "removed" {
        d = d.hint("to replace the screen, send the whole markup instead");
    }
    vec![d]
}

fn set_prop(
    node: &mut Node,
    prop: &str,
    value: Option<&Value>,
    is_root: bool,
    path: &str,
    catalog: &Catalog,
) -> Outcome {
    let reject = |message: String, expected: &str, hint: Option<&str>| {
        Err(vec![
            Diagnostic::new(Code::W503, path, message, expected)
                .got(prop)
                .hint_opt(hint.map(str::to_owned)),
        ])
    };
    if prop == "id" {
        return reject(
            "An element's id cannot be changed by a patch.".into(),
            "a prop other than id",
            Some("remove the element and insert it again with the new id"),
        );
    }
    if is_root && prop == "weft" {
        return reject(
            "The format version cannot be changed by a patch.".into(),
            "a prop other than weft",
            None,
        );
    }
    let event = prop.strip_prefix("on-");
    if !is_name(event.unwrap_or(prop)) {
        return reject(
            format!("{} is not a valid prop name.", quote(prop)),
            "[a-z][a-z0-9]*(-[a-z0-9]+)*",
            Some("event bindings are written \"on-<event>\", for example \"on-press\""),
        );
    }
    if let Some(event) = event {
        match value {
            None => {
                node.on.shift_remove(event);
            }
            Some(Value::String(action)) => {
                node.on.insert(event.to_owned(), action.clone());
            }
            Some(_) => {
                return reject(
                    "An event binding is set to an action name.".into(),
                    "an action name string, or null to remove the binding",
                    None,
                );
            }
        }
        return Ok(());
    }
    if prop == "text" && holds_text_content(node, catalog) {
        // SPEC §7: text has two spellings, and `set` writes the one already in use. Content can
        // only hold a literal, so any other value (or null) replaces the content with the prop.
        if let Some(Value::String(text)) = value {
            node.children = vec![Child::Text(text.clone())];
            return Ok(());
        }
        node.children.clear();
    }
    match value {
        None => {
            node.props.shift_remove(prop);
        }
        Some(v) => {
            node.props.insert(prop.to_owned(), v.clone());
        }
    }
    Ok(())
}

/// A text-bearing component (SPEC §5.1) whose default content is text and nothing else.
fn holds_text_content(node: &Node, catalog: &Catalog) -> bool {
    let content = catalog.components.get(&node.kind).map(|c| c.content);
    matches!(content, Some(Content::Text | Content::Mixed))
        && !node.children.is_empty()
        && node.children.iter().all(|c| matches!(c, Child::Text(_)))
}

fn read_fragment(
    markup: &str,
    root: &Node,
    catalog: &Catalog,
    path: &str,
) -> Result<Vec<Node>, Vec<Diagnostic>> {
    let (wrapper, diagnostics) = parse_fragment(markup, catalog);
    let Some(wrapper) = wrapper else {
        return Err(diagnostics
            .into_iter()
            .map(|mut d| {
                d.path = format!("{path}{}", d.path);
                d
            })
            .collect());
    };
    let count = wrapper.children.len();
    let nodes: Vec<Node> = wrapper
        .children
        .into_iter()
        .filter_map(|c| match c {
            Child::Node(n) => Some(*n),
            Child::Text(_) => None,
        })
        .collect();
    if nodes.is_empty() || nodes.len() != count || !wrapper.slots.is_empty() {
        let got = if nodes.is_empty() {
            "no elements"
        } else {
            "text or <slot> next to elements"
        };
        return Err(vec![
            Diagnostic::new(
                Code::W508,
                path,
                "Inserted markup must be one or more elements and nothing else.",
                "elements such as <button id=\"…\">Text</button>, without loose text or <slot>",
            )
            .got(got),
        ]);
    }

    let taken = all_ids(root);
    let mut inserted = IndexSet::new();
    for node in &nodes {
        ids(node, &mut inserted);
    }
    let clashes: Vec<&String> = inserted.iter().filter(|id| taken.contains(*id)).collect();
    if !clashes.is_empty() {
        let mut both = taken.clone();
        both.extend(inserted.iter().cloned());
        return Err(clashes
            .into_iter()
            .map(|id| {
                Diagnostic::new(
                    Code::W509,
                    path,
                    format!("Id {} is already used in the document.", quote(id)),
                    "ids that no element of the document has",
                )
                .got(id.clone())
                .hint(format!("use {}", quote(&free_id(id, &both))))
            })
            .collect());
    }
    Ok(nodes)
}

/// `None` means any slot name: extension and unknown elements declare nothing.
fn declared_slots(node: &Node, catalog: &Catalog) -> Option<Vec<String>> {
    if node.kind == EACH {
        return Some(vec![]);
    }
    let component = catalog.def_of(node)?;
    Some(
        component
            .slots
            .iter()
            .flat_map(|s| s.keys().cloned())
            .collect(),
    )
}

fn insert_into(
    parent: &mut Node,
    slot: Option<&str>,
    index: Option<u64>,
    catalog: &Catalog,
    at: &dyn Fn(&str) -> String,
    items: Vec<Child>,
) -> Outcome {
    let list = match slot {
        None => &mut parent.children,
        Some(slot) => {
            let declared = declared_slots(parent, catalog);
            if !is_name(slot)
                || declared
                    .as_ref()
                    .is_some_and(|d| !d.iter().any(|n| n == slot))
            {
                let expected = match &declared {
                    Some(d) if !d.is_empty() => one_of(d),
                    _ => "no slot (omit `slot` to use the default slot)".to_owned(),
                };
                let hint = did_you_mean(slot, declared.as_deref().unwrap_or_default())
                    .unwrap_or_else(|| "omit `slot` to use the default slot".to_owned());
                return Err(vec![
                    Diagnostic::new(
                        Code::W504,
                        at("slot"),
                        format!("<{}> does not declare a slot {}.", parent.kind, quote(slot)),
                        expected,
                    )
                    .got(slot)
                    .hint(hint),
                ]);
            }
            parent.slots.entry(slot.to_owned()).or_default()
        }
    };
    let len = list.len();
    let position = index.map_or(len, |i| usize::try_from(i).unwrap_or(usize::MAX));
    if position > len {
        let position = index.unwrap_or_default();
        return Err(vec![
            Diagnostic::new(
                Code::W505,
                at("index"),
                format!("Index {position} is past the end of a list with {len} entries."),
                format!("an integer from 0 to {len} (text counts as an entry)"),
            )
            .got(position.to_string())
            .hint("omit `index` to append"),
        ]);
    }
    list.splice(position..position, items);
    Ok(())
}
