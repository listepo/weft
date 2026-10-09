//! The context block of SPEC §2.3: notes that people and agents leave about the screen and its
//! elements. Read from markup and written back here; never rendered and never followed.

use std::collections::HashMap;

use crate::canonical::{append_text, normalize_text};
use crate::diagnostics::{Code, Diagnostic, Position};
use crate::model::{Child, Entry};
use crate::rules::{CONTEXT, ENTRY};
use crate::source::{NodeSource, Source, path_segment};
use crate::syntax::{RawChild, RawElement, SyntaxResult};

const ATTRIBUTES: [&str; 6] = ["id", "kind", "by", "name", "for", "status"];

pub fn misplaced_context(pos: Position, path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        Code::W120,
        path,
        message,
        "one <context> as the first child of the root, holding <entry> elements only",
    )
    .pos(Some(pos))
}

pub fn misplaced_entry(pos: Position, path: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(
        Code::W121,
        path,
        message,
        "<entry id=\"…\" kind=\"…\" by=\"…\" name=\"…\">text</entry> inside <context>",
    )
    .pos(Some(pos))
}

/// Reads a `<context>` that stands directly under the root. `first` is false for a second block,
/// which is reported and skipped.
pub fn read_block(
    syntax: &SyntaxResult,
    raw: &RawElement,
    root_path: &str,
    first: bool,
    entries: &mut Vec<Entry>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let path = format!("{root_path}/{CONTEXT}");
    if !first {
        diagnostics.push(
            misplaced_context(raw.pos, &path, "The root holds a second <context>.")
                .hint("merge the entries into the first <context> block"),
        );
        return;
    }
    if let Some(attr) = raw.attrs.first() {
        diagnostics.push(
            misplaced_context(
                attr.pos,
                &format!("{path}/@{}", attr.name),
                format!("<context> takes no attributes, not \"{}\".", attr.name),
            )
            .hint("remove the attribute"),
        );
    }
    for child in &raw.children {
        match child {
            RawChild::Text { text, pos } => {
                if !normalize_text(text).is_empty() {
                    diagnostics.push(
                        misplaced_context(*pos, &path, "<context> holds text outside an <entry>.")
                            .hint("put the text inside an <entry>"),
                    );
                }
            }
            RawChild::Element(i) => {
                let child = &syntax.elements[*i];
                if child.name == ENTRY {
                    let index = entries.len();
                    entries.push(read_entry(syntax, child, &path, index, diagnostics));
                } else {
                    diagnostics.push(
                        misplaced_context(
                            child.pos,
                            &format!("{path}/{}", child.segment()),
                            format!(
                                "<context> holds <entry> elements only, not <{}>.",
                                child.name
                            ),
                        )
                        .hint("move the element out of <context>"),
                    );
                }
            }
        }
    }
}

fn read_entry(
    syntax: &SyntaxResult,
    raw: &RawElement,
    context_path: &str,
    index: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Entry {
    let mut entry = Entry::default();
    let mut attrs: HashMap<String, Position> = HashMap::new();
    for attr in &raw.attrs {
        attrs.insert(attr.name.clone(), attr.pos);
        let value = Some(attr.value.clone());
        match attr.name.as_str() {
            "id" => entry.id = value,
            "kind" => entry.kind = attr.value.clone(),
            "by" => entry.by = attr.value.clone(),
            "name" => entry.name = attr.value.clone(),
            "for" => entry.target = value,
            "status" => entry.status = value,
            _ => {}
        }
    }
    let path = format!(
        "{context_path}/{}",
        path_segment(ENTRY, entry.id.as_deref(), Some(index))
    );
    for attr in raw
        .attrs
        .iter()
        .filter(|a| !ATTRIBUTES.contains(&a.name.as_str()))
    {
        diagnostics.push(
            misplaced_entry(
                attr.pos,
                &format!("{path}/@{}", attr.name),
                format!(
                    "<entry> takes only id, kind, by, name, for and status, not \"{}\".",
                    attr.name
                ),
            )
            .hint("remove the attribute"),
        );
    }
    for required in ["kind", "by", "name"] {
        if raw.attr(required).is_none() {
            diagnostics.push(
                misplaced_entry(raw.pos, &path, format!("<entry> has no `{required}`."))
                    .hint(format!("add {required}=\"…\"")),
            );
        }
    }
    let mut text = Vec::new();
    for child in &raw.children {
        match child {
            RawChild::Text { text: t, .. } => {
                append_text(&mut text, normalize_text(t));
            }
            RawChild::Element(i) => {
                let inner = &syntax.elements[*i];
                diagnostics.push(
                    misplaced_entry(
                        inner.pos,
                        &format!("{path}/{}", inner.segment()),
                        format!("<entry> holds text only, not <{}>.", inner.name),
                    )
                    .hint("write the note as plain text, escaping & and < as &amp; and &lt;"),
                );
            }
        }
    }
    if let Some(Child::Text(t)) = text.pop() {
        entry.text = t;
    }
    entry.source = Source(Some(Box::new(NodeSource {
        pos: raw.pos,
        attrs,
        ..NodeSource::default()
    })));
    entry
}

/// The canonical form: text whitespace-normalized as all text is (SPEC §3), positions dropped.
pub fn canonical_entry(entry: &Entry) -> Entry {
    Entry {
        text: normalize_text(&entry.text),
        source: Source::default(),
        ..entry.clone()
    }
}

/// One line of canonical markup: `id` first, the other attributes sorted, the text as content.
pub fn entry_markup(
    entry: &Entry,
    attribute: impl Fn(&str) -> String,
    text: impl Fn(&str) -> String,
) -> String {
    let mut head = String::from("<entry");
    let fields = [
        ("id", entry.id.as_deref()),
        ("by", Some(entry.by.as_str())),
        ("for", entry.target.as_deref()),
        ("kind", Some(entry.kind.as_str())),
        ("name", Some(entry.name.as_str())),
        ("status", entry.status.as_deref()),
    ];
    for (name, value) in fields {
        if let Some(value) = value {
            head.push_str(&format!(" {name}=\"{}\"", attribute(value)));
        }
    }
    if entry.text.is_empty() {
        format!("{head}/>")
    } else {
        format!("{head}>{}</entry>", text(&entry.text))
    }
}
