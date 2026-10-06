//! Classifies the difference between two catalog versions by the SPEC §8 rule, so that a catalog
//! author learns whether the next version is a major, minor or no-op bump. Ported from
//! packages/catalog/src/diff.ts with the same paths and messages.
//!
//! It reads JSON rather than the typed `Catalog`: a field the model does not know yet is itself a
//! breaking change, and the typed model would reject the catalog instead of reporting it.

use serde::Serialize;
use serde_json::{Map as Object, Value as Json};
use weft_core::{order_keys, to_compact};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeLevel {
    None,
    Minor,
    Major,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CatalogChange {
    pub path: String,
    pub level: ChangeLevel,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CatalogDiff {
    pub level: ChangeLevel,
    pub changes: Vec<CatalogChange>,
}

const KNOWN_PROP_FIELDS: [&str; 11] = [
    "description",
    "type",
    "values",
    "tokenType",
    "required",
    "default",
    "bindable",
    "writable",
    "min",
    "max",
    "references",
];

type Out = Vec<CatalogChange>;

fn record(out: &mut Out, path: &str, level: ChangeLevel, message: String) {
    out.push(CatalogChange {
        path: path.to_owned(),
        level,
        message,
    });
}

fn field<'a>(v: Option<&'a Json>, name: &str) -> Option<&'a Json> {
    v?.as_object()?.get(name)
}

/// `JSON.stringify(value) ?? "absent"`.
fn show(v: Option<&Json>) -> String {
    v.map_or_else(|| "absent".to_owned(), to_compact)
}

/// A value inside a template literal: `String(value)`.
fn text(v: Option<&Json>) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(Json::String(s)) => s.clone(),
        Some(Json::Array(items)) => items
            .iter()
            .map(|i| {
                if i.is_null() {
                    String::new()
                } else {
                    text(Some(i))
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Some(Json::Object(_)) => "[object Object]".to_owned(),
        Some(other) => to_compact(other),
    }
}

/// `===`: objects and arrays from two catalogs are never the same reference.
fn strict_eq(a: Option<&Json>, b: Option<&Json>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Json::Number(x)), Some(Json::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Json::Array(_) | Json::Object(_)), _)
        | (_, Some(Json::Array(_) | Json::Object(_))) => false,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `util.isDeepStrictEqual` on JSON values.
fn deep_eq(a: Option<&Json>, b: Option<&Json>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Json::Number(x)), Some(Json::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(Json::Array(x)), Some(Json::Array(y))) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| deep_eq(Some(p), Some(q)))
        }
        (Some(Json::Object(x)), Some(Json::Object(y))) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| deep_eq(Some(v), Some(w))))
        }
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `value ?? fallback` for a boolean flag.
fn flag(v: Option<&Json>, fallback: bool) -> Json {
    match v {
        None | Some(Json::Null) => Json::Bool(fallback),
        Some(other) => other.clone(),
    }
}

fn is_true(v: Option<&Json>) -> bool {
    v == Some(&Json::Bool(true))
}

fn is_false(v: Option<&Json>) -> bool {
    v == Some(&Json::Bool(false))
}

fn items(v: Option<&Json>) -> &[Json] {
    v.and_then(Json::as_array).map_or(&[], Vec::as_slice)
}

fn includes(list: Option<&Json>, item: &Json) -> bool {
    items(list).iter().any(|i| strict_eq(Some(i), Some(item)))
}

/// Which kinds of content a content model accepts; a model is wider when it accepts a superset.
fn accepts(content: Option<&Json>) -> &'static [&'static str] {
    match content.and_then(Json::as_str) {
        Some("text") => &["text"],
        Some("nodes") => &["nodes"],
        Some("mixed") => &["text", "nodes"],
        _ => &[],
    }
}

/// Removing a name is major, adding one is minor.
fn diff_names(out: &mut Out, path: &str, noun: &str, previous: Option<&Json>, next: Option<&Json>) {
    for name in items(previous) {
        if !includes(next, name) {
            record(
                out,
                path,
                ChangeLevel::Major,
                format!("{noun} \"{}\" was removed.", text(Some(name))),
            );
        }
    }
    for name in items(next) {
        if !includes(previous, name) {
            record(
                out,
                path,
                ChangeLevel::Minor,
                format!("{noun} \"{}\" was added.", text(Some(name))),
            );
        }
    }
}

/// Absent means "any", so a list narrows what absent allowed and widens what a list allowed.
fn diff_restriction(
    out: &mut Out,
    path: &str,
    noun: &str,
    previous: Option<&Json>,
    next: Option<&Json>,
) {
    match (previous, next) {
        (None, None) => {}
        (None, Some(_)) => {
            record(
                out,
                path,
                ChangeLevel::Major,
                format!("{noun} was restricted to {}.", show(next)),
            );
        }
        (Some(_), None) => record(
            out,
            path,
            ChangeLevel::Minor,
            format!("{noun} restriction was lifted."),
        ),
        (Some(_), Some(_)) => {
            for name in items(previous) {
                if !includes(next, name) {
                    let message = format!("{noun} no longer allows \"{}\".", text(Some(name)));
                    record(out, path, ChangeLevel::Major, message);
                }
            }
            for name in items(next) {
                if !includes(previous, name) {
                    record(
                        out,
                        path,
                        ChangeLevel::Minor,
                        format!("{noun} now allows \"{}\".", text(Some(name))),
                    );
                }
            }
        }
    }
}

fn diff_bound(out: &mut Out, path: &str, name: &str, previous: Option<&Json>, next: Option<&Json>) {
    if strict_eq(previous, next) {
        return;
    }
    let message = format!("{name} changed from {} to {}.", show(previous), show(next));
    let (Some(a), Some(b)) = (previous.and_then(Json::as_f64), next.and_then(Json::as_f64)) else {
        // A bound appearing narrows the accepted range and one disappearing widens it.
        let level = if next.is_none() {
            ChangeLevel::Minor
        } else {
            ChangeLevel::Major
        };
        return record(out, path, level, message);
    };
    let narrower = if name == "min" { b > a } else { b < a };
    record(
        out,
        path,
        if narrower {
            ChangeLevel::Major
        } else {
            ChangeLevel::Minor
        },
        message,
    );
}

fn diff_prop(out: &mut Out, path: &str, previous: &Json, next: &Json) {
    let (a, b) = (Some(previous), Some(next));
    let get = |v: Option<&'_ Json>, name: &str| field(v, name).cloned();
    if !strict_eq(field(a, "description"), field(b, "description")) {
        record(
            out,
            &format!("{path}.description"),
            ChangeLevel::None,
            "The description changed.".to_owned(),
        );
    }
    if !strict_eq(field(a, "type"), field(b, "type")) {
        let message = format!(
            "type changed from {} to {}.",
            text(field(a, "type")),
            text(field(b, "type"))
        );
        record(out, &format!("{path}.type"), ChangeLevel::Major, message);
    }
    diff_names(
        out,
        &format!("{path}.values"),
        "Enum value",
        field(a, "values"),
        field(b, "values"),
    );
    if !strict_eq(field(a, "tokenType"), field(b, "tokenType")) {
        let message = format!(
            "tokenType changed from {} to {}.",
            show(field(a, "tokenType")),
            show(field(b, "tokenType"))
        );
        record(
            out,
            &format!("{path}.tokenType"),
            ChangeLevel::Major,
            message,
        );
    }
    if !strict_eq(
        Some(&flag(field(a, "required"), false)),
        Some(&flag(field(b, "required"), false)),
    ) {
        let (level, message) = if is_true(field(b, "required")) {
            (ChangeLevel::Major, "The prop became required.")
        } else {
            (ChangeLevel::Minor, "The prop is no longer required.")
        };
        record(out, &format!("{path}.required"), level, message.to_owned());
    }
    if !deep_eq(field(a, "default"), field(b, "default")) {
        let message = format!(
            "default changed from {} to {}.",
            show(field(a, "default")),
            show(field(b, "default"))
        );
        record(out, &format!("{path}.default"), ChangeLevel::Major, message);
    }
    // `bindable` defaults to true and `writable` to false: only the move away from the default
    // that takes a capability away is breaking.
    if !strict_eq(
        Some(&flag(field(a, "bindable"), true)),
        Some(&flag(field(b, "bindable"), true)),
    ) {
        let (level, message) = if is_false(field(b, "bindable")) {
            (ChangeLevel::Major, "The prop no longer accepts bindings.")
        } else {
            (ChangeLevel::Minor, "The prop accepts bindings.")
        };
        record(out, &format!("{path}.bindable"), level, message.to_owned());
    }
    if !strict_eq(
        Some(&flag(field(a, "writable"), false)),
        Some(&flag(field(b, "writable"), false)),
    ) {
        let (level, message) = if is_true(field(b, "writable")) {
            (ChangeLevel::Minor, "The prop became a two-way target.")
        } else {
            (
                ChangeLevel::Major,
                "The prop is no longer a two-way target.",
            )
        };
        record(out, &format!("{path}.writable"), level, message.to_owned());
    }
    if !strict_eq(field(a, "references"), field(b, "references")) {
        // Dropping the reference lets any value through; naming or changing a kind rejects some.
        let (level, message) = if field(b, "references").is_none() {
            (
                ChangeLevel::Minor,
                "The prop no longer names an element.".to_owned(),
            )
        } else {
            (
                ChangeLevel::Major,
                format!(
                    "references changed from {} to {}.",
                    text(field(a, "references")),
                    text(field(b, "references"))
                ),
            )
        };
        record(out, &format!("{path}.references"), level, message);
    }
    diff_bound(
        out,
        &format!("{path}.min"),
        "min",
        field(a, "min"),
        field(b, "min"),
    );
    diff_bound(
        out,
        &format!("{path}.max"),
        "max",
        field(a, "max"),
        field(b, "max"),
    );
    // A field this classifier does not know yet may constrain documents, so it is breaking until
    // the rule for it is written down.
    let empty = Object::new();
    let keys_a = previous.as_object().unwrap_or(&empty).keys();
    let keys_b = next.as_object().unwrap_or(&empty).keys();
    let mut seen: Vec<&String> = Vec::new();
    for name in keys_a.chain(keys_b) {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let (x, y) = (get(a, name), get(b, name));
        if !KNOWN_PROP_FIELDS.contains(&name.as_str()) && !deep_eq(x.as_ref(), y.as_ref()) {
            let message = format!(
                "{name} changed from {} to {}.",
                show(x.as_ref()),
                show(y.as_ref())
            );
            record(out, &format!("{path}.{name}"), ChangeLevel::Major, message);
        }
    }
}

fn diff_slot(out: &mut Out, path: &str, previous: &Json, next: &Json) {
    let (a, b) = (Some(previous), Some(next));
    if !strict_eq(field(a, "description"), field(b, "description")) {
        record(
            out,
            &format!("{path}.description"),
            ChangeLevel::None,
            "The description changed.".to_owned(),
        );
    }
    let allowed = format!("{path}.allowedChildren");
    diff_restriction(
        out,
        &allowed,
        "Slot content",
        field(a, "allowedChildren"),
        field(b, "allowedChildren"),
    );
    if !strict_eq(
        Some(&flag(field(a, "required"), false)),
        Some(&flag(field(b, "required"), false)),
    ) {
        let (level, message) = if is_true(field(b, "required")) {
            (ChangeLevel::Major, "The slot became required.")
        } else {
            (ChangeLevel::Minor, "The slot is no longer required.")
        };
        record(out, &format!("{path}.required"), level, message.to_owned());
    }
}

/// Names on one side only are removed (major) or added (`added` decides); shared names go to `both`.
fn diff_record(
    out: &mut Out,
    path: &str,
    noun: &str,
    previous: Option<&Json>,
    next: Option<&Json>,
    both: fn(&mut Out, &str, &Json, &Json),
    added: fn(&Json) -> ChangeLevel,
) {
    let previous = previous.and_then(Json::as_object);
    let next_map = next.and_then(Json::as_object);
    for name in previous.into_iter().flat_map(Object::keys) {
        if !next_map.is_some_and(|n| n.contains_key(name)) {
            record(
                out,
                &format!("{path}.{name}"),
                ChangeLevel::Major,
                format!("{noun} \"{name}\" was removed."),
            );
        }
    }
    for (name, value) in next_map.into_iter().flatten() {
        let child = format!("{path}.{name}");
        match previous.and_then(|p| p.get(name)) {
            None => record(
                out,
                &child,
                added(value),
                format!("{noun} \"{name}\" was added."),
            ),
            Some(before) => both(out, &child, before, value),
        }
    }
}

/// Existing documents lack a prop or slot that is newly required.
fn added_required(value: &Json) -> ChangeLevel {
    if is_true(field(Some(value), "required")) {
        ChangeLevel::Major
    } else {
        ChangeLevel::Minor
    }
}

fn diff_component(out: &mut Out, path: &str, previous: &Json, next: &Json) {
    let (a, b) = (Some(previous), Some(next));
    if !strict_eq(field(a, "description"), field(b, "description")) {
        record(
            out,
            &format!("{path}.description"),
            ChangeLevel::None,
            "The description changed.".to_owned(),
        );
    }
    if !strict_eq(field(a, "role"), field(b, "role")) {
        let message = format!(
            "role changed from \"{}\" to \"{}\".",
            text(field(a, "role")),
            text(field(b, "role"))
        );
        record(out, &format!("{path}.role"), ChangeLevel::Major, message);
    }
    if !strict_eq(field(a, "content"), field(b, "content")) {
        let after = accepts(field(b, "content"));
        let wider = accepts(field(a, "content"))
            .iter()
            .all(|kind| after.contains(kind));
        let message = format!(
            "content changed from \"{}\" to \"{}\", {} the content model.",
            text(field(a, "content")),
            text(field(b, "content")),
            if wider { "widening" } else { "narrowing" }
        );
        let level = if wider {
            ChangeLevel::Minor
        } else {
            ChangeLevel::Major
        };
        record(out, &format!("{path}.content"), level, message);
    }
    let restriction = |out: &mut Out, name: &str, noun: &str| {
        diff_restriction(
            out,
            &format!("{path}.{name}"),
            noun,
            field(a, name),
            field(b, name),
        );
    };
    restriction(out, "allowedChildren", "Child kinds");
    restriction(out, "allowedParents", "Parent kinds");
    if !strict_eq(
        Some(&flag(field(a, "requiresLabel"), false)),
        Some(&flag(field(b, "requiresLabel"), false)),
    ) {
        let (level, message) = if is_true(field(b, "requiresLabel")) {
            (ChangeLevel::Major, "A label became required.")
        } else {
            (ChangeLevel::Minor, "A label is no longer required.")
        };
        record(
            out,
            &format!("{path}.requiresLabel"),
            level,
            message.to_owned(),
        );
    }
    if !strict_eq(
        Some(&flag(field(a, "root"), false)),
        Some(&flag(field(b, "root"), false)),
    ) {
        let (level, message) = if is_true(field(b, "root")) {
            (
                ChangeLevel::Major,
                "The component became the document root.",
            )
        } else {
            (
                ChangeLevel::Minor,
                "The component is no longer the document root.",
            )
        };
        record(out, &format!("{path}.root"), level, message.to_owned());
    }
    let props = format!("{path}.props");
    diff_record(
        out,
        &props,
        "Prop",
        field(a, "props"),
        field(b, "props"),
        diff_prop,
        added_required,
    );
    let slots = format!("{path}.slots");
    diff_record(
        out,
        &slots,
        "Slot",
        field(a, "slots"),
        field(b, "slots"),
        diff_slot,
        added_required,
    );
    diff_names(
        out,
        &format!("{path}.states"),
        "State",
        field(a, "states"),
        field(b, "states"),
    );
    diff_names(
        out,
        &format!("{path}.events"),
        "Event",
        field(a, "events"),
        field(b, "events"),
    );
}

pub fn diff_catalogs(previous: &Json, next: &Json) -> CatalogDiff {
    // Object.keys lists integer-like names first; the change order follows it.
    let (previous, next) = (order_keys(previous.clone()), order_keys(next.clone()));
    let mut changes = Vec::new();
    diff_record(
        &mut changes,
        "components",
        "Component",
        field(Some(&previous), "components"),
        field(Some(&next), "components"),
        diff_component,
        |_| ChangeLevel::Minor,
    );
    let level = changes
        .iter()
        .map(|c| c.level)
        .max()
        .unwrap_or(ChangeLevel::None);
    CatalogDiff { level, changes }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn base() -> Json {
        json!({ "weft": "0.1", "name": "t", "version": "1.0.0", "components": {
            "button": { "description": "Button.", "role": "button", "content": "text",
                "props": { "count": { "description": "Count.", "type": "number", "min": 0, "max": 10 } } }
        } })
    }

    fn paths(diff: &CatalogDiff) -> Vec<&str> {
        diff.changes.iter().map(|c| c.path.as_str()).collect()
    }

    #[test]
    fn a_catalog_against_itself_is_none() {
        let diff = diff_catalogs(&base(), &base());
        assert_eq!(
            diff,
            CatalogDiff {
                level: ChangeLevel::None,
                changes: vec![]
            }
        );
    }

    #[test]
    fn removing_a_component_is_major_and_adding_one_is_minor() {
        let mut next = base();
        next["components"].as_object_mut().unwrap().remove("button");
        assert_eq!(diff_catalogs(&base(), &next).level, ChangeLevel::Major);
        assert_eq!(diff_catalogs(&next, &base()).level, ChangeLevel::Minor);
    }

    #[test]
    fn a_higher_min_narrows_and_a_dropped_max_widens() {
        let mut next = base();
        next["components"]["button"]["props"]["count"]["min"] = json!(2);
        let diff = diff_catalogs(&base(), &next);
        assert_eq!(
            (diff.level, paths(&diff)),
            (
                ChangeLevel::Major,
                vec!["components.button.props.count.min"]
            )
        );
        assert_eq!(diff.changes[0].message, "min changed from 0 to 2.");
        let mut next = base();
        next["components"]["button"]["props"]["count"]
            .as_object_mut()
            .unwrap()
            .remove("max");
        assert_eq!(diff_catalogs(&base(), &next).level, ChangeLevel::Minor);
    }

    #[test]
    fn an_unknown_prop_field_is_major() {
        let mut next = base();
        next["components"]["button"]["props"]["count"]["pattern"] = json!("^a");
        let diff = diff_catalogs(&base(), &next);
        assert_eq!(
            diff.changes[0].message,
            "pattern changed from absent to \"^a\"."
        );
        assert_eq!(diff.level, ChangeLevel::Major);
    }

    #[test]
    fn widening_text_to_mixed_content_is_minor() {
        let mut next = base();
        next["components"]["button"]["content"] = json!("mixed");
        let diff = diff_catalogs(&base(), &next);
        assert_eq!(
            diff.changes[0].message,
            "content changed from \"text\" to \"mixed\", widening the content model."
        );
        assert_eq!(diff.level, ChangeLevel::Minor);
    }
}
