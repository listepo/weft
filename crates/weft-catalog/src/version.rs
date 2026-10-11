//! How far a document or library catalog must raise its version (SPEC §8).
//!
//! A fragment's public API is its parameter signature, classified by [`diff_catalogs`]. A screen's
//! version speaks to its host. Below 1.0.0 the least version follows Cargo's caret rule, as
//! catalog `requires` does: an incompatible change raises the minor number.

use std::collections::BTreeSet;

use semver::Version;
use serde_json::Value as Json;
use weft_core::{
    Catalog, Child, Document, Node, ValidateOptions, Value, canonicalize, fragment::signature,
    is_action, to_document, validate,
};

use crate::diff::{ChangeLevel, diff_catalogs};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VersionLevel {
    None,
    Patch,
    Minor,
    Major,
}

impl VersionLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            VersionLevel::None => "none",
            VersionLevel::Patch => "patch",
            VersionLevel::Minor => "minor",
            VersionLevel::Major => "major",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionChange {
    pub level: VersionLevel,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionCheck {
    pub level: VersionLevel,
    pub changes: Vec<VersionChange>,
    /// The least version the newer file may declare. Absent when nothing changed and the older
    /// file declared none.
    pub least: Option<String>,
    pub ok: bool,
}

pub fn check_documents(old: &Document, new: &Document, catalog: &Catalog) -> VersionCheck {
    let changes = if old.root.kind == "fragment" && new.root.kind == "fragment" {
        fragment_changes(old, new)
    } else if old.root.kind != new.root.kind {
        vec![change(
            VersionLevel::Major,
            "root",
            format!(
                "The root changed from {} to {}.",
                old.root.kind, new.root.kind
            ),
        )]
    } else {
        screen_changes(old, new, catalog)
    };
    finish(changes, old.version.as_deref(), new.version.as_deref())
}

pub fn check_catalogs(old: &Json, new: &Json, catalog: &Catalog) -> VersionCheck {
    let diff = diff_catalogs(old, new);
    let mut changes = diff
        .changes
        .into_iter()
        .map(|change| VersionChange {
            level: from_catalog(change.level),
            path: change.path,
            message: change.message,
        })
        .collect::<Vec<_>>();
    let old_fragments = object(old, "fragments");
    let new_fragments = object(new, "fragments");
    let mut names = BTreeSet::new();
    names.extend(old_fragments.keys().cloned());
    names.extend(new_fragments.keys().cloned());
    for name in names {
        match (old_fragments.get(&name), new_fragments.get(&name)) {
            (Some(_), None) => changes.push(change(
                VersionLevel::Major,
                &format!("fragments.{name}"),
                format!("Fragment \"{name}\" was removed."),
            )),
            (None, Some(added)) if as_document(added).is_some() => changes.push(change(
                VersionLevel::Minor,
                &format!("fragments.{name}"),
                format!("Fragment \"{name}\" was added."),
            )),
            (Some(before), Some(after)) => {
                let (Some(before), Some(after)) = (as_document(before), as_document(after)) else {
                    continue;
                };
                for mut item in check_documents(&before, &after, catalog).changes {
                    item.path = format!("fragments.{name}.{}", item.path);
                    changes.push(item);
                }
            }
            _ => {}
        }
    }
    finish(
        changes,
        old.get("version").and_then(Json::as_str),
        new.get("version").and_then(Json::as_str),
    )
}

fn object(value: &Json, key: &str) -> serde_json::Map<String, Json> {
    value
        .get(key)
        .and_then(Json::as_object)
        .cloned()
        .unwrap_or_default()
}

fn as_document(value: &Json) -> Option<Document> {
    // No catalog: only the JSON shape is checked, so a fragment value that is a document is read
    // and one that is a file name is left out.
    validate(value, &ValidateOptions::default())
        .is_empty()
        .then(|| to_document(value))
}

fn fragment_changes(old: &Document, new: &Document) -> Vec<VersionChange> {
    let diff = diff_catalogs(&signature_catalog(&old.root), &signature_catalog(&new.root));
    let mut changes = diff
        .changes
        .into_iter()
        .map(|change| VersionChange {
            level: from_catalog(change.level),
            path: change.path,
            message: change.message,
        })
        .collect::<Vec<_>>();
    let interface = changes
        .iter()
        .map(|change| change.level)
        .max()
        .unwrap_or(VersionLevel::None);
    // A label change is a description, which the catalog diff rates as none, and a body change
    // is a patch. Either way the canonical document, not the signature level, decides a patch.
    if interface < VersionLevel::Minor && content(old) != content(new) {
        changes.push(change(
            VersionLevel::Patch,
            "body",
            "The fragment body changed.".to_owned(),
        ));
    }
    changes
}

fn signature_catalog(root: &Node) -> Json {
    let component = serde_json::to_value(signature(root)).unwrap_or(Json::Null);
    serde_json::json!({
        "weft": "0.3",
        "name": "fragment",
        "version": "0.0.0",
        "components": { "fragment": component }
    })
}

fn screen_changes(old: &Document, new: &Document, catalog: &Catalog) -> Vec<VersionChange> {
    let before = contract(&old.root, catalog);
    let after = contract(&new.root, catalog);
    let mut changes = Vec::new();
    for action in after.actions.difference(&before.actions) {
        changes.push(change(
            VersionLevel::Major,
            "actions",
            format!("The screen names action \"{action}\"."),
        ));
    }
    for path in after.paths.keys() {
        if !before.paths.contains_key(path) {
            changes.push(change(
                VersionLevel::Major,
                "data",
                format!("The screen reads {path}."),
            ));
        }
    }
    for (path, types) in &after.paths {
        let Some(previous) = before.paths.get(path) else {
            continue;
        };
        if !types.is_subset(previous) {
            changes.push(change(
                VersionLevel::Major,
                &format!("data.{path}"),
                format!("{path} is read as a type it was not read as before."),
            ));
        }
    }
    for path in after.writable.difference(&before.writable) {
        changes.push(change(
            VersionLevel::Major,
            "writable",
            format!("The screen writes {path}."),
        ));
    }
    for id in before.ids.difference(&after.ids) {
        changes.push(change(
            VersionLevel::Major,
            "ids",
            format!("Element id \"{id}\" was removed."),
        ));
    }
    let gained = after.ids.difference(&before.ids).next().is_some();
    let major = changes
        .iter()
        .any(|change| change.level == VersionLevel::Major);
    if gained && !major {
        changes.push(change(
            VersionLevel::Minor,
            "ids",
            "The screen gained element ids.".to_owned(),
        ));
    }
    if changes.is_empty() && content(old) != content(new) {
        changes.push(change(
            VersionLevel::Patch,
            "body",
            "The canonical JSON changed.".to_owned(),
        ));
    }
    changes
}

struct Contract {
    actions: BTreeSet<String>,
    paths: std::collections::BTreeMap<String, BTreeSet<String>>,
    writable: BTreeSet<String>,
    ids: BTreeSet<String>,
}

fn contract(root: &Node, catalog: &Catalog) -> Contract {
    let mut out = Contract {
        actions: BTreeSet::new(),
        paths: std::collections::BTreeMap::new(),
        writable: BTreeSet::new(),
        ids: BTreeSet::new(),
    };
    walk(root, catalog, &mut out);
    out
}

fn walk(node: &Node, catalog: &Catalog, out: &mut Contract) {
    if let Some(id) = &node.id {
        out.ids.insert(id.clone());
    }
    let def = catalog.def_of(node);
    for (name, value) in &node.props {
        let Value::Bind { bind, .. } = value else {
            continue;
        };
        if !bind.starts_with("$.") {
            continue;
        }
        let prop = def.and_then(|def| def.prop(name));
        let ty = prop.map(|prop| prop.kind.as_str()).unwrap_or("string");
        out.paths
            .entry(bind.clone())
            .or_default()
            .insert(ty.to_owned());
        if prop.and_then(|prop| prop.writable) == Some(true) {
            out.writable.insert(bind.clone());
        }
    }
    for action in node.on.values() {
        if is_action(action) {
            out.actions.insert(action.clone());
        }
    }
    for child in &node.children {
        if let Child::Node(child) = child {
            walk(child, catalog, out);
        }
    }
    for list in node.slots.values() {
        for child in list {
            if let Child::Node(child) = child {
                walk(child, catalog, out);
            }
        }
    }
}

/// `version` and `weft` are the declaration being checked, not a change of the document.
fn content(document: &Document) -> Json {
    let mut value = serde_json::to_value(canonicalize(document)).unwrap_or(Json::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("weft");
        object.remove("version");
    }
    value
}

fn from_catalog(level: ChangeLevel) -> VersionLevel {
    match level {
        ChangeLevel::None => VersionLevel::None,
        ChangeLevel::Minor => VersionLevel::Minor,
        ChangeLevel::Major => VersionLevel::Major,
    }
}

fn change(level: VersionLevel, path: &str, message: String) -> VersionChange {
    VersionChange {
        level,
        path: path.to_owned(),
        message,
    }
}

fn finish(changes: Vec<VersionChange>, old: Option<&str>, declared: Option<&str>) -> VersionCheck {
    let level = changes
        .iter()
        .map(|change| change.level)
        .max()
        .unwrap_or(VersionLevel::None);
    let least = least_version(old, level);
    let ok = meets(declared, least.as_deref());
    VersionCheck {
        level,
        changes,
        least,
        ok,
    }
}

fn parse_version(text: &str) -> Option<Version> {
    let version = Version::parse(text).ok()?;
    (version.pre.is_empty() && version.build.is_empty()).then_some(version)
}

fn least_version(old: Option<&str>, level: VersionLevel) -> Option<String> {
    match (old.and_then(parse_version), level) {
        (parsed, VersionLevel::None) => parsed.map(|version| version.to_string()),
        (None, VersionLevel::Major) => Some("0.1.0".to_owned()),
        (None, _) => Some("0.0.1".to_owned()),
        (Some(version), level) => Some(bump(version, level).to_string()),
    }
}

fn bump(mut version: Version, level: VersionLevel) -> Version {
    if version.major == 0 {
        if level == VersionLevel::Major {
            version.minor = version.minor.saturating_add(1);
            version.patch = 0;
        } else if level != VersionLevel::None {
            version.patch = version.patch.saturating_add(1);
        }
    } else {
        match level {
            VersionLevel::Major => {
                version.major = version.major.saturating_add(1);
                version.minor = 0;
                version.patch = 0;
            }
            VersionLevel::Minor => {
                version.minor = version.minor.saturating_add(1);
                version.patch = 0;
            }
            VersionLevel::Patch => version.patch = version.patch.saturating_add(1),
            VersionLevel::None => {}
        }
    }
    version
}

fn meets(declared: Option<&str>, least: Option<&str>) -> bool {
    let Some(least) = least.and_then(parse_version) else {
        return true;
    };
    declared
        .and_then(parse_version)
        .is_some_and(|version| version >= least)
}
