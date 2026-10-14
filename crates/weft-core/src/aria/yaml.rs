//! The YAML `locator.ariaSnapshot()` prints for a tree, so a tree a document declares reads (and
//! diffs) like one taken from a page.

use serde_json::Value as Json;

use super::{AriaNode, normalize_aria};
use crate::{js_number, js_trim};

const STATE_ORDER: [&str; 7] = [
    "checked", "disabled", "expanded", "invalid", "level", "pressed", "selected",
];

/// The tree as the YAML Playwright prints; a `fragment` root prints its children.
pub fn format_aria_snapshot(tree: &AriaNode) -> String {
    let root = normalize_aria(tree);
    let mut lines = Vec::new();
    if root.role == "fragment" {
        for n in &root.children {
            write_node(n, "", &mut lines);
        }
    } else {
        write_node(&root, "", &mut lines);
    }
    lines.join("\n")
}

fn write_node(n: &AriaNode, indent: &str, lines: &mut Vec<String>) {
    if n.role == "text" {
        lines.push(format!("{indent}- text: {}", yaml_value(&n.name)));
        return;
    }
    let mut key = n.role.clone();
    if !n.name.is_empty() {
        key = format!("{key} {}", json_string(&n.name));
    }
    for state in STATE_ORDER {
        match n.states.get(state) {
            None | Some(Json::Bool(false)) => {}
            Some(Json::Bool(true)) => key.push_str(&format!(" [{state}]")),
            Some(v) => key.push_str(&format!(" [{state}={}]", js_string(v))),
        }
    }
    let head = format!("{indent}- {}", yaml_key(&key));
    match (&n.url, &n.children[..]) {
        (None, [only]) if only.role == "text" => {
            lines.push(format!("{head}: {}", yaml_value(&only.name)));
        }
        (None, []) => lines.push(head),
        (url, children) => {
            lines.push(format!("{head}:"));
            if let Some(url) = url {
                lines.push(format!("{indent}  - /url: {}", yaml_value(url)));
            }
            let inner = format!("{indent}  ");
            for c in children {
                write_node(c, &inner, lines);
            }
        }
    }
}

/// JavaScript's `String(value)` for the state values a tree holds.
fn js_string(v: &Json) -> String {
    match v {
        Json::String(s) => s.clone(),
        Json::Number(n) => n.as_f64().map(js_number).unwrap_or_default(),
        other => other.to_string(),
    }
}

/// `JSON.stringify` of a string: serde_json escapes the same characters.
fn json_string(s: &str) -> String {
    Json::String(s.to_owned()).to_string()
}

/// A key YAML would split at ": " or read as a comment is single-quoted, as Playwright does.
fn yaml_key(key: &str) -> String {
    if key.contains(": ") || key.ends_with(':') || key.contains(" #") {
        format!("'{}'", key.replace('\'', "''"))
    } else {
        key.to_owned()
    }
}

/// A plain scalar YAML would read as something other than the string is double-quoted.
fn yaml_value(value: &str) -> String {
    let indicator = value.starts_with([
        '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@',
        '`',
    ]);
    let breaks = value.contains(": ")
        || value.ends_with(':')
        || value.contains(" #")
        || value.contains(['\n', '\r', '\t']);
    let plain = !value.is_empty()
        && js_trim(value) == value
        && !indicator
        && !breaks
        && !reads_as_scalar(value);
    if plain {
        value.to_owned()
    } else {
        json_string(value)
    }
}

/// `true`, `false`, `null`, `~` or a YAML number, in any letter case.
fn reads_as_scalar(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if matches!(lower.as_str(), "true" | "false" | "null" | "~") {
        return true;
    }
    // [-+]?(\d[\d_]*(\.\d*)?|\.\d+)([eE][-+]?\d+)?
    let s = lower.strip_prefix(['-', '+']).unwrap_or(&lower);
    let (mantissa, exponent) = match s.find('e') {
        Some(at) => (&s[..at], Some(&s[at + 1..])),
        None => (s, None),
    };
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some(("", fraction)) => digits(fraction),
        Some((whole, fraction)) => {
            whole_part(whole) && fraction.bytes().all(|b| b.is_ascii_digit())
        }
        None => whole_part(mantissa),
    };
    mantissa_ok && exponent.is_none_or(|e| digits(e.strip_prefix(['-', '+']).unwrap_or(e)))
}

/// `\d[\d_]*`
fn whole_part(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_digit)
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'_')
}
