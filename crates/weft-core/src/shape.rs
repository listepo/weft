//! The JSON shape checks of SPEC §3 and §7 (W200, W501), with the issue messages and order of
//! the zod 4 schemas in the TypeScript core, so that both cores report the same diagnostics.
//! Also converts JSON that passed the check into the model.

use serde_json::Value as Json;

use crate::model::{Child, Document, Entry, Map, Node, Value};

pub enum Segment {
    Key(String),
    Index(usize),
}

pub struct Issue {
    pub path: Vec<Segment>,
    pub message: String,
    /// An unknown key only. A union keeps an option whose issues are all soft (zod "nonaborted").
    soft: bool,
}

impl Issue {
    pub fn path_string(&self) -> String {
        self.path
            .iter()
            .map(|s| match s {
                Segment::Key(k) => k.clone(),
                Segment::Index(i) => i.to_string(),
            })
            .collect::<Vec<_>>()
            .join("/")
    }
}

type Path<'a> = &'a [Segment];

fn at(path: Path<'_>, segment: Segment) -> Vec<Segment> {
    let mut out: Vec<Segment> = path.iter().map(clone_segment).collect();
    out.push(segment);
    out
}

fn clone_segment(s: &Segment) -> Segment {
    match s {
        Segment::Key(k) => Segment::Key(k.clone()),
        Segment::Index(i) => Segment::Index(*i),
    }
}

fn hard(path: Path<'_>, message: impl Into<String>) -> Issue {
    Issue {
        path: path.iter().map(clone_segment).collect(),
        message: message.into(),
        soft: false,
    }
}

fn received(v: Option<&Json>) -> &'static str {
    match v {
        None => "undefined",
        Some(Json::Null) => "null",
        Some(Json::Bool(_)) => "boolean",
        Some(Json::Number(_)) => "number",
        Some(Json::String(_)) => "string",
        Some(Json::Array(_)) => "array",
        Some(Json::Object(_)) => "object",
    }
}

fn invalid_type(path: Path<'_>, expected: &str, v: Option<&Json>) -> Issue {
    hard(
        path,
        format!(
            "Invalid input: expected {expected}, received {}",
            received(v)
        ),
    )
}

const INVALID: &str = "Invalid input";

type Object = serde_json::Map<String, Json>;
/// Checks one field of a strict object.
type Field<'f> = (
    &'f str,
    &'f dyn Fn(Option<&Json>, Path<'_>, &mut Vec<Issue>),
);

/// A strict object: declared fields in order, then one issue listing undeclared keys.
fn strict_object(
    v: Option<&Json>,
    path: Path<'_>,
    fields: &[Field<'_>],
    issues: &mut Vec<Issue>,
) -> Option<()> {
    let Some(Json::Object(map)) = v else {
        issues.push(invalid_type(path, "object", v));
        return None;
    };
    for (name, check) in fields {
        check(
            map.get(*name),
            &at(path, Segment::Key((*name).to_owned())),
            issues,
        );
    }
    unknown_keys(map, fields.iter().map(|(n, _)| *n), path, issues);
    Some(())
}

fn unknown_keys<'a>(
    map: &Object,
    declared: impl Iterator<Item = &'a str> + Clone,
    path: Path<'_>,
    issues: &mut Vec<Issue>,
) {
    let unknown: Vec<String> = map
        .keys()
        .filter(|k| !declared.clone().any(|d| d == k.as_str()))
        .map(|k| format!("\"{k}\""))
        .collect();
    if !unknown.is_empty() {
        let plural = if unknown.len() > 1 { "s" } else { "" };
        issues.push(Issue {
            path: path.iter().map(clone_segment).collect(),
            message: format!("Unrecognized key{plural}: {}", unknown.join(", ")),
            soft: true,
        });
    }
}

fn string(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    if !matches!(v, Some(Json::String(_))) {
        issues.push(invalid_type(path, "string", v));
    }
}

fn optional(
    check: fn(Option<&Json>, Path<'_>, &mut Vec<Issue>),
) -> impl Fn(Option<&Json>, Path<'_>, &mut Vec<Issue>) {
    move |v, path, issues| {
        if v.is_some() {
            check(v, path, issues);
        }
    }
}

fn record(
    v: Option<&Json>,
    path: Path<'_>,
    issues: &mut Vec<Issue>,
    item: fn(Option<&Json>, Path<'_>, &mut Vec<Issue>),
) {
    let Some(Json::Object(map)) = v else {
        issues.push(invalid_type(path, "record", v));
        return;
    };
    for (k, value) in map {
        item(Some(value), &at(path, Segment::Key(k.clone())), issues);
    }
}

fn array(
    v: Option<&Json>,
    path: Path<'_>,
    issues: &mut Vec<Issue>,
    item: fn(Option<&Json>, Path<'_>, &mut Vec<Issue>),
) {
    let Some(Json::Array(items)) = v else {
        issues.push(invalid_type(path, "array", v));
        return;
    };
    for (i, value) in items.iter().enumerate() {
        item(Some(value), &at(path, Segment::Index(i)), issues);
    }
}

/// A zod union: passes if any option passes; otherwise the issues of the only option that failed
/// on unknown keys alone, or one "Invalid input".
fn union(path: Path<'_>, options: Vec<Vec<Issue>>, issues: &mut Vec<Issue>) {
    if options.iter().any(Vec::is_empty) {
        return;
    }
    let mut nonaborted: Vec<Vec<Issue>> = options
        .into_iter()
        .filter(|o| o.iter().all(|i| i.soft))
        .collect();
    match (nonaborted.pop(), nonaborted.is_empty()) {
        (Some(only), true) => issues.extend(only),
        _ => issues.push(hard(path, INVALID)),
    }
}

fn binding(v: Option<&Json>, path: Path<'_>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let not = |v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>| {
        if v.is_some() && v != Some(&Json::Bool(true)) {
            issues.push(hard(path, "Invalid input: expected true"));
        }
    };
    strict_object(v, path, &[("bind", &string), ("not", &not)], &mut issues);
    issues
}

fn token(v: Option<&Json>, path: Path<'_>) -> Vec<Issue> {
    let mut issues = Vec::new();
    strict_object(v, path, &[("token", &string)], &mut issues);
    issues
}

fn value(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    match v {
        Some(Json::String(_) | Json::Number(_) | Json::Bool(_)) => {}
        Some(Json::Object(_)) => union(path, vec![binding(v, path), token(v, path)], issues),
        _ => issues.push(hard(path, INVALID)),
    }
}

fn node(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    let props = |v: Option<&Json>, p: Path<'_>, i: &mut Vec<Issue>| {
        if v.is_some() {
            record(v, p, i, value);
        }
    };
    let on = |v: Option<&Json>, p: Path<'_>, i: &mut Vec<Issue>| {
        if v.is_some() {
            record(v, p, i, string);
        }
    };
    let slots = |v: Option<&Json>, p: Path<'_>, i: &mut Vec<Issue>| {
        if v.is_some() {
            record(v, p, i, children);
        }
    };
    let id = optional(string);
    let kids = optional(children);
    strict_object(
        v,
        path,
        &[
            ("kind", &string),
            ("id", &id),
            ("props", &props),
            ("on", &on),
            ("slots", &slots),
            ("children", &kids),
        ],
        issues,
    );
}

fn children(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    array(v, path, issues, child);
}

fn child(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    if matches!(v, Some(Json::String(_))) {
        return;
    }
    let mut as_node = Vec::new();
    node(v, path, &mut as_node);
    union(path, vec![as_node, vec![hard(path, INVALID)]], issues);
}

/// `EntrySchema`: every member a string, so that validation names a bad value (SPEC §2.3).
fn entry(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    let text = optional(string);
    strict_object(
        v,
        path,
        &[
            ("id", &text),
            ("kind", &string),
            ("by", &string),
            ("name", &string),
            ("for", &text),
            ("status", &text),
            ("text", &string),
        ],
        issues,
    );
}

fn context(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    array(v, path, issues, entry);
}

/// Issues of `DocumentSchema`, in zod's order.
pub fn document_issues(v: &Json) -> Vec<Issue> {
    let mut issues = Vec::new();
    let block = optional(context);
    strict_object(
        Some(v),
        &[],
        &[("weft", &string), ("context", &block), ("root", &node)],
        &mut issues,
    );
    issues
}

const OPS: [&str; 8] = [
    "set",
    "insert",
    "remove",
    "move",
    "add-context",
    "set-context",
    "resolve-context",
    "remove-context",
];
const FIELDS: [&str; 3] = ["text", "kind", "for"];
/// `Number.MAX_SAFE_INTEGER`: zod's `int()` admits safe integers only.
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

fn index(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    let Some(v) = v else { return };
    let Some(n) = v.as_f64().filter(|_| v.is_number()) else {
        issues.push(invalid_type(path, "number", Some(v)));
        return;
    };
    if n.fract() != 0.0 {
        issues.push(hard(path, "Invalid input: expected int, received number"));
        return;
    }
    if n < -MAX_SAFE_INTEGER {
        issues.push(hard(
            path,
            "Too small: expected int to be >=-9007199254740991",
        ));
    } else if n > MAX_SAFE_INTEGER {
        issues.push(hard(path, "Too big: expected int to be <=9007199254740991"));
    }
    if n < 0.0 {
        issues.push(hard(path, "Too small: expected number to be >=0"));
    }
}

/// Issues of `PatchSchema`, a discriminated union on `op`.
pub fn patch_issues(v: &Json) -> Vec<Issue> {
    let mut issues = Vec::new();
    let Json::Object(map) = v else {
        issues.push(invalid_type(&[], "object", Some(v)));
        return issues;
    };
    let op = map
        .get("op")
        .and_then(Json::as_str)
        .filter(|op| OPS.contains(op));
    let Some(op) = op else {
        let expected: Vec<String> = OPS.iter().map(|op| format!("'{op}'")).collect();
        issues.push(hard(
            &[Segment::Key("op".into())],
            format!(
                "Invalid discriminator value. Expected {}",
                expected.join(" | ")
            ),
        ));
        return issues;
    };
    let nothing = |_: Option<&Json>, _: Path<'_>, _: &mut Vec<Issue>| {};
    let nullable_value = |v: Option<&Json>, p: Path<'_>, i: &mut Vec<Issue>| {
        if v != Some(&Json::Null) {
            value(v, p, i);
        }
    };
    let slot = optional(string);
    let fields: Vec<Field<'_>> = match op {
        "set" => vec![
            ("op", &nothing),
            ("id", &string),
            ("prop", &string),
            ("value", &nullable_value),
        ],
        "insert" => vec![
            ("op", &nothing),
            ("parent", &string),
            ("slot", &slot),
            ("index", &index),
            ("markup", &string),
        ],
        "remove" | "resolve-context" | "remove-context" => vec![("op", &nothing), ("id", &string)],
        "move" => vec![
            ("op", &nothing),
            ("id", &string),
            ("parent", &string),
            ("slot", &slot),
            ("index", &index),
        ],
        "add-context" => vec![("op", &nothing), ("entry", &entry)],
        _ => vec![
            ("op", &nothing),
            ("id", &string),
            ("field", &field),
            ("value", &nullable_string),
        ],
    };
    strict_object(Some(v), &[], &fields, &mut issues);
    // Only `for` can be cleared: an entry always has a kind and a text (SPEC §7).
    let clears = map.get("value") == Some(&Json::Null);
    let field = map.get("field").and_then(Json::as_str);
    if op == "set-context" && clears && field.is_some_and(|f| f != "for" && FIELDS.contains(&f)) {
        issues.push(invalid_type(
            &[Segment::Key("value".into())],
            "string",
            Some(&Json::Null),
        ));
    }
    issues
}

fn field(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    if !v
        .and_then(Json::as_str)
        .is_some_and(|f| FIELDS.contains(&f))
    {
        issues.push(hard(
            path,
            r#"Invalid option: expected one of "text"|"kind"|"for""#,
        ));
    }
}

fn nullable_string(v: Option<&Json>, path: Path<'_>, issues: &mut Vec<Issue>) {
    if v != Some(&Json::Null) {
        string(v, path, issues);
    }
}

/// A JSON value as a `Value`, read leniently like the props of `to_document`.
pub fn to_value(v: &Json) -> Value {
    match v {
        Json::String(s) => Value::String(s.clone()),
        Json::Number(n) => Value::Number(n.as_f64().unwrap_or_default()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Object(m) => match (
            m.get("bind").and_then(Json::as_str),
            m.get("token").and_then(Json::as_str),
        ) {
            (Some(bind), _) => Value::Bind {
                bind: bind.to_owned(),
                not: m.get("not") == Some(&Json::Bool(true)),
            },
            (None, Some(token)) => Value::Token(token.to_owned()),
            (None, None) => Value::String(String::new()),
        },
        Json::Null | Json::Array(_) => Value::String(String::new()),
    }
}

pub fn to_values(v: Option<&Json>) -> Map<Value> {
    v.and_then(Json::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), to_value(v))).collect())
        .unwrap_or_default()
}

fn to_children(v: Option<&Json>) -> Vec<Child> {
    v.and_then(Json::as_array)
        .map(|items| {
            items
                .iter()
                .map(|c| match c {
                    Json::String(s) => Child::Text(s.clone()),
                    other => Child::Node(Box::new(to_node(other))),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn to_node(v: &Json) -> Node {
    let str_of = |k: &str| v.get(k).and_then(Json::as_str).map(str::to_owned);
    let object = |k: &str| v.get(k).and_then(Json::as_object);
    Node {
        kind: str_of("kind").unwrap_or_default(),
        id: str_of("id"),
        props: to_values(v.get("props")),
        on: object("on")
            .map(|m| {
                m.iter()
                    .map(|(k, a)| (k.clone(), a.as_str().unwrap_or_default().to_owned()))
                    .collect()
            })
            .unwrap_or_default(),
        slots: object("slots")
            .map(|m| {
                m.iter()
                    .map(|(k, l)| (k.clone(), to_children(Some(l))))
                    .collect()
            })
            .unwrap_or_default(),
        children: to_children(v.get("children")),
        source: Default::default(),
    }
}

pub(crate) fn to_entry(v: &Json) -> Entry {
    let text = |k: &str| v.get(k).and_then(Json::as_str).map(str::to_owned);
    Entry {
        id: text("id"),
        kind: text("kind").unwrap_or_default(),
        by: text("by").unwrap_or_default(),
        name: text("name").unwrap_or_default(),
        target: text("for"),
        status: text("status"),
        text: text("text").unwrap_or_default(),
        source: Default::default(),
    }
}

/// The model of JSON that passed [`document_issues`]; members it lacks read as empty.
pub fn to_document(v: &Json) -> Document {
    Document {
        weft: v
            .get("weft")
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned(),
        context: v
            .get("context")
            .and_then(Json::as_array)
            .map(|list| list.iter().map(to_entry).collect())
            .unwrap_or_default(),
        root: v.get("root").map(to_node).unwrap_or_default(),
    }
}

pub fn value_of(v: &Json) -> Value {
    to_value(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn messages(issues: Vec<Issue>) -> Vec<(String, String)> {
        issues
            .into_iter()
            .map(|i| (i.path_string(), i.message))
            .collect()
    }

    #[test]
    fn a_valid_document_has_no_issues() {
        let doc = json!({"weft": "0.1", "root": {"kind": "screen", "props": {"a": {"bind": "$.x", "not": true}, "b": 1}, "slots": {"s": ["t", {"kind": "x"}]}}});
        assert!(document_issues(&doc).is_empty());
    }

    #[test]
    fn object_issues_follow_field_order_then_unknown_keys() {
        let doc = json!({"zz": 1, "root": {"kind": 1, "extra": 2, "more": 3}, "weft": 2});
        assert_eq!(
            messages(document_issues(&doc)),
            [
                (
                    "weft".into(),
                    "Invalid input: expected string, received number".into()
                ),
                (
                    "root/kind".into(),
                    "Invalid input: expected string, received number".into()
                ),
                (
                    "root".into(),
                    "Unrecognized keys: \"extra\", \"more\"".into()
                ),
                ("".into(), "Unrecognized key: \"zz\"".into()),
            ]
        );
    }

    #[test]
    fn unions_keep_the_only_option_that_failed_softly() {
        let doc = json!({"weft": "0.1", "root": {"kind": "s", "props": {
            "a": {"bind": "x", "extra": 1},
            "b": {"bind": "x", "token": "y"},
            "c": {"bind": "x", "not": false},
            "d": null
        }, "children": [{"id": 1}, {"kind": "b", "extra": 1}]}});
        assert_eq!(
            messages(document_issues(&doc)),
            [
                ("root/props/a".into(), "Unrecognized key: \"extra\"".into()),
                ("root/props/b".into(), "Invalid input".into()),
                ("root/props/c".into(), "Invalid input".into()),
                ("root/props/d".into(), "Invalid input".into()),
                ("root/children/0".into(), "Invalid input".into()),
                (
                    "root/children/1".into(),
                    "Unrecognized key: \"extra\"".into()
                ),
            ]
        );
    }

    #[test]
    fn patch_issues_match_the_discriminated_union() {
        assert_eq!(
            messages(patch_issues(&json!({"op": "explode"}))),
            [(
                "op".into(),
                "Invalid discriminator value. Expected 'set' | 'insert' | 'remove' | 'move' | 'add-context' | 'set-context' | 'resolve-context' | 'remove-context'".into()
            )]
        );
        assert_eq!(
            messages(patch_issues(&json!([]))),
            [(
                "".into(),
                "Invalid input: expected object, received array".into()
            )]
        );
        assert_eq!(
            messages(patch_issues(&json!({"op": "set", "id": "a", "prop": "b"}))),
            [("value".into(), "Invalid input".into())]
        );
        assert!(
            patch_issues(&json!({"op": "set", "id": "a", "prop": "b", "value": null})).is_empty()
        );
        assert_eq!(
            messages(patch_issues(
                &json!({"op": "move", "id": "a", "parent": "p", "index": -1e300, "x": 1})
            )),
            [
                (
                    "index".into(),
                    "Too small: expected int to be >=-9007199254740991".into()
                ),
                (
                    "index".into(),
                    "Too small: expected number to be >=0".into()
                ),
                ("".into(), "Unrecognized key: \"x\"".into()),
            ]
        );
        assert_eq!(
            messages(patch_issues(
                &json!({"op": "insert", "parent": "p", "index": 1.5, "markup": ""})
            )),
            [(
                "index".into(),
                "Invalid input: expected int, received number".into()
            )]
        );
    }
}
