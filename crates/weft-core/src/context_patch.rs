//! The context operations of SPEC §7: `add-context`, `set-context`, `resolve-context` and
//! `remove-context`. Values are left to validation as `set` leaves them; what is checked here is
//! addressing and what the host allows, because a forged author must never reach the document.

use indexmap::IndexSet;

use crate::context_check::has_status;
use crate::diagnostics::{Code, Diagnostic, did_you_mean, quote};
use crate::model::{Document, Entry};

/// Who the host lets `add-context` claim to be. The model never sets this: the host stamps it,
/// so an agent channel cannot write `by="human"` (SPEC §7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Author {
    pub by: String,
    pub name: Option<String>,
}

pub(crate) enum ContextPatch {
    Add(Entry),
    Set {
        id: String,
        field: String,
        value: Option<String>,
    },
    Resolve(String),
    Remove(String),
}

type Outcome = Result<(), Vec<Diagnostic>>;

pub(crate) fn apply_context(
    document: &mut Document,
    patch: &ContextPatch,
    element_ids: IndexSet<String>,
    author: Option<&Author>,
    read_only: bool,
    i: usize,
) -> Outcome {
    let at = |field: &str| format!("#/patches/{i}/{field}");
    if read_only {
        return Err(vec![
            Diagnostic::new(
                Code::W512,
                at("op"),
                "This host keeps context read-only; context patches are refused.",
                "set, insert, remove or move",
            )
            .hint("leave the context as it is, and tell the user what you would change"),
        ]);
    }
    let entry_ids = || {
        document
            .context
            .iter()
            .filter_map(|e| e.id.clone())
            .collect::<Vec<_>>()
    };
    match patch {
        ContextPatch::Add(entry) => {
            if let Some(author) = author {
                check_author(entry, author, &at("entry"))?;
            }
            if let Some(id) = &entry.id {
                let mut taken = element_ids;
                taken.extend(entry_ids());
                if taken.contains(id) {
                    return Err(vec![
                        Diagnostic::new(
                            Code::W510,
                            at("entry/id"),
                            format!("The id {} is already taken.", quote(id)),
                            "an id that no element or entry has",
                        )
                        .got(id.clone())
                        .hint(format!("use {}", quote(&free_id(id, &taken)))),
                    ]);
                }
            }
            document.context.push(entry.clone());
        }
        ContextPatch::Set { id, field, value } => {
            let index = find(document, id, &element_ids, &at("id"))?;
            let entry = &mut document.context[index];
            match field.as_str() {
                "text" => entry.text = value.clone().unwrap_or_default(),
                "kind" => {
                    entry.kind = value.clone().unwrap_or_default();
                    if has_status(&entry.kind) {
                        entry.status.get_or_insert_with(|| "open".to_owned());
                    } else {
                        entry.status = None;
                    }
                }
                _ => entry.target = value.clone(),
            }
        }
        ContextPatch::Resolve(id) => {
            let index = find(document, id, &element_ids, &at("id"))?;
            document.context[index].status = Some("resolved".to_owned());
        }
        ContextPatch::Remove(id) => {
            let index = find(document, id, &element_ids, &at("id"))?;
            document.context.remove(index);
        }
    }
    Ok(())
}

fn check_author(entry: &Entry, author: &Author, at: &str) -> Outcome {
    let refuse = |field: &str, want: &str, got: &str| {
        Err(vec![
            Diagnostic::new(
                Code::W512,
                format!("{at}/{field}"),
                format!("This host writes context as {field}={}.", quote(want)),
                quote(want),
            )
            .got(quote(got))
            .hint(format!("write \"{field}\": {}", quote(want))),
        ])
    };
    if entry.by != author.by {
        return refuse("by", &author.by, &entry.by);
    }
    match &author.name {
        Some(name) if *name != entry.name => refuse("name", name, &entry.name),
        _ => Ok(()),
    }
}

fn find(
    document: &Document,
    id: &str,
    element_ids: &IndexSet<String>,
    at: &str,
) -> Result<usize, Vec<Diagnostic>> {
    if let Some(index) = document
        .context
        .iter()
        .position(|e| e.id.as_deref() == Some(id))
    {
        return Ok(index);
    }
    let hint = if element_ids.contains(id) {
        format!(
            "{} is an element's id; context patches name entries",
            quote(id)
        )
    } else {
        did_you_mean(id, document.context.iter().filter_map(|e| e.id.as_deref()))
            .unwrap_or_else(|| "copy an id from the context block".to_owned())
    };
    Err(vec![
        Diagnostic::new(
            Code::W511,
            at,
            format!("No context entry has the id {}.", quote(id)),
            "the id of a context entry",
        )
        .got(id)
        .hint(hint),
    ])
}

fn free_id(id: &str, taken: &IndexSet<String>) -> String {
    let mut n = 2;
    while taken.contains(&format!("{id}-{n}")) {
        n += 1;
    }
    format!("{id}-{n}")
}

/// The hint of a `W309` on an entry whose element a patch removed: name the two ways out, since
/// dropping the note silently could lose a constraint with the element it protected.
pub(crate) fn dangling_hint(diagnostic: &mut Diagnostic) {
    if diagnostic.code != Code::W309 {
        return;
    }
    let Some((_, rest)) = diagnostic.path.rsplit_once("/entry#") else {
        return;
    };
    let id = rest.split('/').next().unwrap_or(rest);
    diagnostic.hint = Some(format!("remove-context {id}, or set-context its for"));
}
