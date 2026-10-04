//! The generated theme type: every design token a screen references, as nested `CGFloat`
//! properties named after the token path, so `{token.space.md}` reads `theme.space.md`.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::Value as Json;
use weft_catalog::Token;

use crate::Unsupported;
use crate::swift;

/// CSS `rem` against the browser default; SwiftUI points match CSS pixels.
const POINTS_PER_REM: f64 = 16.0;

#[derive(Default)]
struct Group {
    leaves: BTreeMap<String, f64>,
    groups: BTreeMap<String, Group>,
}

/// `theme.space.md` for the token path `space.md`.
pub fn token_expr(path: &str) -> String {
    let mut out = String::from("theme");
    for segment in path.split('.') {
        out.push('.');
        out.push_str(&swift::member(segment));
    }
    out
}

fn points(token: &Token) -> Option<f64> {
    if token.kind != "dimension" {
        return None;
    }
    let Json::Object(value) = &token.value else {
        return None;
    };
    let number = value.get("value")?.as_f64()?;
    match value.get("unit")?.as_str()? {
        "px" => Some(number),
        "rem" => Some(number * POINTS_PER_REM),
        _ => None,
    }
}

/// The theme struct's lines, indented by the caller.
pub fn theme_struct(
    name: &str,
    used: &[String],
    tokens: &IndexMap<String, Token>,
    problems: &mut Vec<Unsupported>,
) -> Vec<String> {
    let mut root = Group::default();
    for path in used {
        let Some(value) = tokens.get(path).and_then(points) else {
            problems.push(Unsupported::new(
                format!("{{token.{path}}}"),
                "the token is not a dimension in px or rem in the token set",
            ));
            continue;
        };
        let segments: Vec<&str> = path.split('.').collect();
        let Some((last, groups)) = segments.split_last() else {
            continue;
        };
        let mut group = &mut root;
        for segment in groups {
            group = group.groups.entry((*segment).to_owned()).or_default();
        }
        group.leaves.insert((*last).to_owned(), value);
    }
    let mut lines = vec![];
    print_group(name, &root, &mut lines, "", problems);
    lines
}

fn print_group(
    name: &str,
    group: &Group,
    out: &mut Vec<String>,
    indent: &str,
    problems: &mut Vec<Unsupported>,
) {
    out.push(format!("{indent}struct {name}: Sendable {{"));
    let inner = format!("{indent}    ");
    let mut types: Vec<(String, &Group)> = vec![];
    for (segment, group) in &group.groups {
        if group.leaves.is_empty() && group.groups.is_empty() {
            continue;
        }
        let type_name = group_type_name(segment, types.iter().map(|(t, _)| t.as_str()));
        out.push(format!(
            "{inner}var {} = {type_name}()",
            swift::member(segment)
        ));
        types.push((type_name, group));
    }
    for (segment, value) in &group.leaves {
        if group.groups.contains_key(segment) {
            problems.push(Unsupported::new(
                format!("{{token.{segment}}}"),
                "a token path is both a token and a group of the referenced tokens",
            ));
            continue;
        }
        out.push(format!(
            "{inner}var {}: CGFloat = {}",
            swift::member(segment),
            swift::number_literal(*value)
        ));
    }
    for (type_name, group) in types {
        out.push(String::new());
        print_group(&type_name, group, out, &inner, problems);
    }
    out.push(format!("{indent}}}"));
}

fn group_type_name<'a>(segment: &str, taken: impl Iterator<Item = &'a str> + Clone) -> String {
    let mut chars = segment.chars();
    let base = match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {
            let candidate: String = first.to_uppercase().chain(chars).collect();
            if swift::is_plain_identifier(&candidate) {
                candidate
            } else {
                "Group".to_owned()
            }
        }
        _ => "Group".to_owned(),
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.clone().any(|t| t == name) {
        name = format!("{base}{n}");
        n += 1;
    }
    name
}
