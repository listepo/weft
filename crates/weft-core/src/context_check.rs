//! The checks of the context block (SPEC §2.3): kinds, authors, status, `for`, the strings and
//! the limits. Entries are data for the next reader, so the checks keep them small and well-formed
//! and never act on what they say.

use indexmap::IndexMap;

use crate::canonical::normalize_text;
use crate::context::entry_path;
use crate::diagnostics::{Code, Diagnostic, Position, did_you_mean, one_of, quote};
use crate::model::Entry;
use crate::rules::{CONTEXT, ENTRY, has_non_xml_char, is_id};

pub const KINDS: [&str; 6] = [
    "intent",
    "decision",
    "constraint",
    "question",
    "todo",
    "source",
];
pub const AUTHORS: [&str; 2] = ["human", "agent"];
pub const STATUSES: [&str; 2] = ["open", "resolved"];
/// The limits keep the one block of free prose small enough that it cannot flood a model's
/// window or carry a long injected script (SPEC §2.3).
pub const MAX_ENTRIES: usize = 100;
pub const MAX_TEXT: usize = 500;
pub const MAX_TOTAL: usize = 16_000;
pub const MAX_NAME: usize = 64;

/// `question` and `todo` say whether they are still open; no other kind does.
pub fn has_status(kind: &str) -> bool {
    matches!(kind, "question" | "todo")
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Letters and digits of any script, then also space and `._@/+-`.
fn is_author_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphanumeric() => {
            chars.all(|c| c.is_alphanumeric() || " ._@/+-".contains(c))
        }
        _ => false,
    }
}

struct Check<'a> {
    entry: &'a Entry,
    path: String,
}

impl Check<'_> {
    fn at(&self, attr: Option<&str>) -> (String, Option<Position>) {
        let source = self.entry.source.0.as_deref();
        match attr {
            None => (self.path.clone(), source.map(|s| s.pos)),
            Some(a) => (
                format!("{}/@{a}", self.path),
                source
                    .and_then(|s| s.attrs.get(a).copied())
                    .or(source.map(|s| s.pos)),
            ),
        }
    }

    fn diag(
        &self,
        code: Code,
        attr: Option<&str>,
        message: impl Into<String>,
        expected: impl Into<String>,
    ) -> Diagnostic {
        let (path, pos) = self.at(attr);
        Diagnostic::new(code, path, message, expected).pos(pos)
    }

    fn one_of(&self, attr: &str, value: &str, values: &[&str]) -> Option<Diagnostic> {
        if values.contains(&value) {
            return None;
        }
        Some(
            self.diag(
                Code::W203,
                Some(attr),
                format!("{} is not an allowed `{attr}`.", quote(value)),
                one_of(values),
            )
            .got(quote(value))
            .hint_opt(did_you_mean(value, values)),
        )
    }
}

/// Checks the entries of a document whose elements are already in `ids` (id → kind, path).
/// Entry ids join `ids`, since `for` and patches share one id space with elements.
pub(crate) fn check_entries(
    entries: &[Entry],
    root_path: &str,
    root_id: Option<&str>,
    ids: &mut IndexMap<String, (String, String)>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let elements: Vec<String> = ids.keys().cloned().collect();
    let mut total = 0;
    for (index, entry) in entries.iter().enumerate() {
        let c = Check {
            entry,
            path: entry_path(root_path, entry, index),
        };
        check_id(&c, ids, &mut out);
        out.extend(c.one_of("kind", &entry.kind, &KINDS));
        out.extend(c.one_of("by", &entry.by, &AUTHORS));
        if let Some(status) = &entry.status {
            out.extend(c.one_of("status", status, &STATUSES));
        }
        if KINDS.contains(&entry.kind.as_str()) {
            match (has_status(&entry.kind), entry.status.is_some()) {
                (true, false) => out.push(
                    c.diag(
                        Code::W227,
                        None,
                        format!("A `{}` entry has no `status`.", entry.kind),
                        "status=\"open\" or status=\"resolved\"",
                    )
                    .hint("add status=\"open\""),
                ),
                (false, true) => out.push(
                    c.diag(
                        Code::W227,
                        Some("status"),
                        format!("A `{}` entry takes no `status`.", entry.kind),
                        "status only on a question or a todo",
                    )
                    .hint("remove status"),
                ),
                _ => {}
            }
        }
        if let Some(target) = &entry.target {
            check_target(&c, target, root_id, &elements, &mut out);
        }
        check_strings(&c, &mut out);
        let text = normalize_text(&entry.text);
        let length = utf16_len(&text);
        total += length;
        if length == 0 {
            out.push(
                c.diag(
                    Code::W229,
                    None,
                    "The entry has no text.",
                    "1 to 500 characters of text",
                )
                .hint("write the note, or remove the entry"),
            );
        } else if length > MAX_TEXT {
            out.push(
                c.diag(
                    Code::W228,
                    None,
                    format!("The entry text has {length} characters."),
                    format!("at most {MAX_TEXT} characters"),
                )
                .hint("shorten the note, or split it into several entries"),
            );
        }
    }
    let context_path = format!("{root_path}/{CONTEXT}");
    if entries.len() > MAX_ENTRIES {
        out.push(
            Diagnostic::new(
                Code::W228,
                context_path.clone(),
                format!("The context has {} entries.", entries.len()),
                format!("at most {MAX_ENTRIES} entries"),
            )
            .hint("remove resolved or outdated entries"),
        );
    }
    if total > MAX_TOTAL {
        out.push(
            Diagnostic::new(
                Code::W228,
                context_path,
                format!("The context holds {total} characters of text."),
                format!("at most {MAX_TOTAL} characters in all"),
            )
            .hint("remove resolved or outdated entries"),
        );
    }
    out
}

fn check_id(
    c: &Check<'_>,
    ids: &mut IndexMap<String, (String, String)>,
    out: &mut Vec<Diagnostic>,
) {
    match &c.entry.id {
        None => out.push(
            c.diag(Code::W202, None, "<entry> has no id.", "id=\"…\"")
                .hint("add an id that no element or entry has"),
        ),
        Some(id) if !is_id(id) => out.push(
            c.diag(
                Code::W212,
                Some("id"),
                format!("Id {} breaks the id grammar.", quote(id)),
                "[A-Za-z][A-Za-z0-9_-]*",
            )
            .got(id.clone()),
        ),
        Some(id) => match ids.get(id) {
            None => {
                ids.insert(id.clone(), (ENTRY.to_owned(), c.path.clone()));
            }
            Some((_, first)) => out.push(
                c.diag(
                    Code::W301,
                    Some("id"),
                    format!("Id \"{id}\" is already used."),
                    "a document-unique id",
                )
                .got(id.clone())
                .hint(format!("first used at {first}; pick another id")),
            ),
        },
    }
}

fn check_target(
    c: &Check<'_>,
    target: &str,
    root_id: Option<&str>,
    elements: &[String],
    out: &mut Vec<Diagnostic>,
) {
    let others: Vec<&String> = elements
        .iter()
        .filter(|id| Some(id.as_str()) != root_id)
        .collect();
    let (message, hint) = if Some(target) == root_id {
        (
            format!("\"{target}\" is the root; an entry about the screen has no `for`."),
            "remove for".to_owned(),
        )
    } else if elements.iter().any(|id| id == target) {
        return;
    } else {
        (
            format!("\"{target}\" is not the id of an element."),
            did_you_mean(target, others.iter().map(|s| s.as_str())).unwrap_or_else(|| {
                "remove for to make the entry about the screen, or name an existing element"
                    .to_owned()
            }),
        )
    };
    out.push(
        c.diag(
            Code::W309,
            Some("for"),
            message,
            "the id of an element other than the root, or no for",
        )
        .got(target.to_owned())
        .hint(hint),
    );
}

/// `name` follows its grammar; strings from JSON hold only characters markup can carry.
fn check_strings(c: &Check<'_>, out: &mut Vec<Diagnostic>) {
    let name = &c.entry.name;
    let length = utf16_len(name);
    if length == 0 || length > MAX_NAME || !is_author_name(name) {
        out.push(
            c.diag(
                Code::W229,
                Some("name"),
                format!("Author name {} is not allowed.", quote(name)),
                "1 to 64 letters, digits, spaces or ._@/+-, starting with a letter or digit",
            )
            .got(quote(name))
            .hint("write a person's name or handle, or the model id"),
        );
    }
    let fields = [
        ("text", Some(&c.entry.text)),
        ("for", c.entry.target.as_ref()),
    ];
    for (field, value) in fields {
        if value.is_some_and(|v| has_non_xml_char(v)) {
            let attr = (field != "text").then_some(field);
            out.push(
                c.diag(
                    Code::W221,
                    attr,
                    format!("The entry's `{field}` holds a character markup cannot carry."),
                    "characters XML 1.0 allows",
                )
                .hint("remove control characters and unpaired surrogates"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_names_follow_their_grammar() {
        for good in ["Ivan", "claude-opus-5-5", "Иван Тугай", "a.b_c@d/e+f"] {
            assert!(is_author_name(good), "{good}");
        }
        for bad in ["", " lead", "-x", "a<b", "x\u{0}"] {
            assert!(!is_author_name(bad), "{bad}");
        }
    }
}
