//! Plain-language readback of what a document's bindings, tokens, events and loops mean.
//! Validation cannot see intent: an edit that keeps a `!` while changing the condition is valid
//! markup with the opposite meaning. Reading every reference back as a sentence lets a model or a
//! person compare the meaning with the instruction. Context entries (SPEC §2.3) read back after
//! the element they are about, so a reviewer sees every note an edit added, changed or removed.
//! Pure: no I/O.

use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;

use crate::context::entry_path;
use crate::context_check::MAX_TEXT;
use crate::fragment::{self, USE};
use crate::model::{Catalog, Child, Document, Entry, Node, PropDef, PropType, Value};
use crate::rules::{EACH, is_id, universal_prop};
use crate::source::path_segment;
use crate::values::format_value;

/// One prop, event or loop of one element, read as a sentence.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Readback {
    /// The element's diagnostic path (SPEC §6.1), e.g. `/screen#login/form#f1/button#go`.
    pub path: String,
    /// `kind#id`, or `path` when the element has no valid id: ids are document-unique, so the
    /// short form is enough wherever it exists.
    pub target: String,
    /// The attribute as written: a prop name, `on-<event>` for an event, `in` for a loop.
    pub name: String,
    pub sentence: String,
}

impl fmt::Display for Readback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.target, self.name, self.sentence)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
}

/// A prop, event or loop whose value differs between two versions of a document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Change {
    /// Path and target in the newer document, or in the older one for a removal.
    pub path: String,
    pub target: String,
    pub name: String,
    pub kind: ChangeKind,
    /// Absent for an addition.
    pub before: Option<String>,
    /// Absent for a removal.
    pub after: Option<String>,
}

impl fmt::Display for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (target, name) = (&self.target, &self.name);
        let before = self.before.as_deref().unwrap_or_default();
        let after = self.after.as_deref().unwrap_or_default();
        match self.kind {
            ChangeKind::Added => write!(f, "{target} {name} added: {after}"),
            ChangeKind::Removed => write!(f, "{target} {name} removed: was {before}"),
            ChangeKind::Changed => {
                write!(f, "{target} {name} changed: was {before}; now {after}")
            }
        }
    }
}

/// Reads back every binding, token reference, event and loop in document order. Literals are left
/// out: they mean what they say. The catalog tells boolean and writable props apart; without one,
/// a plain binding reads as `reads $.path`.
pub fn explain(document: &Document, catalog: Option<&Catalog>) -> Vec<Readback> {
    collect(document, catalog)
        .into_values()
        .filter(|item| !item.literal && !item.context)
        .map(|item| item.readback)
        .collect()
}

/// `explain`, with each element's context entries after its own readbacks, and the entries about
/// the screen after the root's. An entry reads as `context <id>: <kind> by <by> <name>: <text>`.
pub fn explain_with_context(document: &Document, catalog: Option<&Catalog>) -> Vec<Readback> {
    collect(document, catalog)
        .into_values()
        .filter(|item| !item.literal)
        .map(|item| item.readback)
        .collect()
}

/// Lists the props (literals included), events, loops and context entries that were added,
/// removed or changed between two versions of a document, with the readback before and after.
/// Elements are matched by id, or by path when they have none, so a moved element with unchanged
/// props is not listed.
/// Additions and changes come in the newer document's order, then removals in the older one's.
pub fn explain_changes(
    before: &Document,
    after: &Document,
    catalog: Option<&Catalog>,
) -> Vec<Change> {
    let old = collect(before, catalog);
    let new = collect(after, catalog);
    let mut changes = vec![];
    for (key, item) in &new {
        let previous = old.get(key);
        if previous.is_some_and(|p| p.raw == item.raw) {
            continue;
        }
        changes.push(Change {
            path: item.readback.path.clone(),
            target: item.readback.target.clone(),
            name: item.readback.name.clone(),
            kind: if previous.is_some() {
                ChangeKind::Changed
            } else {
                ChangeKind::Added
            },
            before: previous.map(|p| p.readback.sentence.clone()),
            after: Some(item.readback.sentence.clone()),
        });
    }
    for (key, item) in &old {
        if !new.contains_key(key) {
            changes.push(Change {
                path: item.readback.path.clone(),
                target: item.readback.target.clone(),
                name: item.readback.name.clone(),
                kind: ChangeKind::Removed,
                before: Some(item.readback.sentence.clone()),
                after: None,
            });
        }
    }
    changes
}

/// What a change is decided on: the stored value, not the sentence, so that renaming a loop's
/// array does not list every binding inside the loop as changed.
#[derive(PartialEq)]
enum Raw<'a> {
    Value(&'a Value),
    Action(&'a str),
    Loop(Option<&'a Value>, Option<&'a Value>),
    Entry(&'a Entry),
    Note(&'a str),
}

struct Item<'a> {
    readback: Readback,
    raw: Raw<'a>,
    literal: bool,
    /// A context entry: read back by `explain_with_context` and compared by `explain_changes`.
    context: bool,
}

/// A loop variable in scope: its name without `$`, and the array it walks as written.
struct Scope {
    name: String,
    items: String,
}

struct Walker<'a> {
    catalog: Option<&'a Catalog>,
    scopes: Vec<Scope>,
    /// Keyed by element (`#id`, or the path without one) and attribute name; entries by
    /// `context#id`.
    items: IndexMap<String, Item<'a>>,
    entries: &'a [Entry],
    root_path: String,
    /// True until the root has been visited: entries without `for` are about it.
    at_root: bool,
    /// The root's target, for entries whose `for` names no element.
    root_target: String,
}

fn collect<'a>(document: &'a Document, catalog: Option<&'a Catalog>) -> IndexMap<String, Item<'a>> {
    let root = &document.root;
    let path = format!("/{}", path_segment(&root.kind, root.id.as_deref(), None));
    let mut walker = Walker {
        catalog,
        scopes: vec![],
        items: IndexMap::new(),
        entries: &document.context,
        root_path: path.clone(),
        at_root: true,
        root_target: String::new(),
    };
    walker.node(root, path);
    // A dangling `for` (W229) still reads back, against the screen: a reviewer must see every
    // note, including one whose element an edit removed.
    let target = walker.root_target.clone();
    for (index, entry) in document.context.iter().enumerate() {
        if !walker.items.contains_key(&entry_key(entry, index)) {
            walker.entry(index, entry, &target);
        }
    }
    walker.items
}

fn entry_key(entry: &Entry, index: usize) -> String {
    match entry.id.as_deref().filter(|id| is_id(id)) {
        Some(id) => format!("context#{id}"),
        None => format!("context[{index}]"),
    }
}

/// `kind (status) by <by> <name>: text`: who claims to have written it is part of the line, so a
/// reviewer weighs it as a claim (AGENT-SPEC §2.9).
/// Text over the `W228` limit is cut and says so (SPEC §2.3): explain is lenient.
fn entry_sentence(entry: &Entry) -> String {
    let text = if entry.text.encode_utf16().count() > MAX_TEXT {
        let mut units = 0;
        let kept: String = entry
            .text
            .chars()
            .take_while(|c| {
                units += c.len_utf16();
                units <= MAX_TEXT
            })
            .collect();
        format!("{kept}… (cut at {MAX_TEXT} characters)")
    } else {
        entry.text.clone()
    };
    let status = entry
        .status
        .as_deref()
        .map(|s| format!(" ({s})"))
        .unwrap_or_default();
    format!(
        "{}{status} by {} {}: {text}",
        entry.kind, entry.by, entry.name
    )
}

enum NoteRaw<'a> {
    Value(&'a Value),
    Name(&'a str),
}

struct Note<'a> {
    name: String,
    sentence: String,
    raw: NoteRaw<'a>,
}

/// Which variant a `<use>` chose, and a slot the chosen body drops because it has no outlet.
fn variant_notes<'a>(catalog: Option<&'a Catalog>, node: &'a Node) -> Vec<Note<'a>> {
    let Some(fragment) = catalog.and_then(|c| c.fragment_of(node)) else {
        return vec![];
    };
    let root = &fragment.document.root;
    let Some(choice) = fragment::choice(root, node) else {
        return vec![];
    };
    let fragment_name = match node.props.get("fragment") {
        Some(Value::String(name)) => name.as_str(),
        _ => "",
    };
    let sentence = match (choice.value, choice.defaulted, choice.raw) {
        (Some(value), false, _) => format!("uses the \"{value}\" variant of {fragment_name}"),
        (Some(value), true, _) => {
            format!("uses the \"{value}\" variant of {fragment_name}, the default")
        }
        (None, _, Some(_)) => format!(
            "no variant of {fragment_name} is chosen, because \"{}\" is not a literal",
            choice.name
        ),
        (None, _, None) => format!(
            "no variant of {fragment_name} is chosen, because \"{}\" is missing",
            choice.name
        ),
    };
    let raw = match choice.raw {
        Some(value) => NoteRaw::Value(value),
        None => NoteRaw::Name(choice.name),
    };
    let mut notes = vec![Note {
        name: "variant".to_owned(),
        sentence,
        raw,
    }];
    let Some(value) = choice.value else {
        return notes;
    };
    let Some(body) = fragment::variant_body(root, value) else {
        return notes;
    };
    for (slot, _) in &node.slots {
        if fragment::has_outlet(body, slot) {
            continue;
        }
        notes.push(Note {
            name: format!("slot {slot}"),
            sentence: format!(
                "drops the \"{slot}\" slot: the \"{value}\" variant has no outlet for it"
            ),
            raw: NoteRaw::Name(slot),
        });
    }
    notes
}

impl<'a> Walker<'a> {
    fn node(&mut self, node: &'a Node, path: String) {
        let is_root = std::mem::take(&mut self.at_root);
        let valid_id = node.id.as_deref().filter(|id| is_id(id));
        let target = match valid_id {
            Some(id) => format!("{}#{id}", node.kind),
            None => path.clone(),
        };
        let key = match valid_id {
            Some(id) => format!("#{id}"),
            None => path.clone(),
        };
        let add = |items: &mut IndexMap<String, Item<'a>>,
                   name: &str,
                   sentence: String,
                   raw: Raw<'a>,
                   literal: bool| {
            let readback = Readback {
                path: path.clone(),
                target: target.clone(),
                name: name.to_owned(),
                sentence,
            };
            items.insert(
                format!("{key} {name}"),
                Item {
                    readback,
                    raw,
                    literal,
                    context: false,
                },
            );
        };

        let is_each = node.kind == EACH;
        if is_each {
            let items = node.props.get("in");
            let variable = node.props.get("as");
            let sentence = self.loop_sentence(items, variable);
            add(
                &mut self.items,
                "in",
                sentence,
                Raw::Loop(items, variable),
                false,
            );
        }
        let component = self.catalog.and_then(|c| c.def_of(node));
        for (name, value) in &node.props {
            if is_each && (name == "in" || name == "as") {
                continue;
            }
            let def = component
                .and_then(|c| c.prop(name))
                .or_else(|| universal_prop(name));
            let literal = !matches!(value, Value::Bind { .. } | Value::Token(_));
            let sentence = self.value_sentence(value, def);
            add(&mut self.items, name, sentence, Raw::Value(value), literal);
        }
        if node.kind == USE {
            for note in variant_notes(self.catalog, node) {
                let raw = match note.raw {
                    NoteRaw::Value(value) => Raw::Value(value),
                    NoteRaw::Name(name) => Raw::Note(name),
                };
                add(&mut self.items, &note.name, note.sentence, raw, false);
            }
        }
        for (event, action) in &node.on {
            let sentence = match self.scopes.last() {
                Some(scope) => format!(
                    "runs action {action} for the current item of {}",
                    scope.items
                ),
                None => format!("runs action {action}"),
            };
            add(
                &mut self.items,
                &format!("on-{event}"),
                sentence,
                Raw::Action(action),
                false,
            );
        }

        if is_root {
            self.root_target.clone_from(&target);
        }
        for (index, entry) in self.entries.iter().enumerate() {
            let about = match entry.target.as_deref() {
                Some(id) => valid_id == Some(id),
                None => is_root,
            };
            if about {
                self.entry(index, entry, &target);
            }
        }
        let pushed = match (is_each, node.props.get("as")) {
            (true, Some(Value::String(name))) => {
                let items = match node.props.get("in") {
                    Some(Value::Bind { bind, not: false }) => bind.clone(),
                    other => other.map(format_value).unwrap_or_default(),
                };
                self.scopes.push(Scope {
                    name: name.clone(),
                    items,
                });
                true
            }
            _ => false,
        };
        self.children(&node.children, &path);
        for (name, list) in &node.slots {
            self.children(list, &format!("{path}/slot[{name}]"));
        }
        if pushed {
            self.scopes.pop();
        }
    }

    fn entry(&mut self, index: usize, entry: &'a Entry, target: &str) {
        let name = match entry.id.as_deref().filter(|id| is_id(id)) {
            Some(id) => format!("context {id}"),
            None => "context".to_owned(),
        };
        let readback = Readback {
            path: entry_path(&self.root_path, entry, index),
            target: target.to_owned(),
            name,
            sentence: entry_sentence(entry),
        };
        self.items.insert(
            entry_key(entry, index),
            Item {
                readback,
                raw: Raw::Entry(entry),
                literal: false,
                context: true,
            },
        );
    }
    fn children(&mut self, list: &'a [Child], path: &str) {
        for (index, child) in list.iter().enumerate() {
            if let Child::Node(n) = child {
                let segment = path_segment(&n.kind, n.id.as_deref(), Some(index));
                self.node(n, format!("{path}/{segment}"));
            }
        }
    }

    /// The forms are fixed so a negation cannot be missed: a negated binding always says
    /// `is falsy` and `NOT`, a plain one on a boolean prop `is truthy`. Truthiness is the host
    /// language's, as the JSX mapping of SPEC §9 makes `not` a `!`.
    fn value_sentence(&self, value: &Value, def: Option<&PropDef>) -> String {
        match value {
            Value::Bind { bind, not: true } => {
                format!(
                    "true while {bind} is falsy (NOT {bind}){}",
                    self.origin(bind)
                )
            }
            Value::Bind { bind, not: false } => {
                let boolean = def.is_some_and(|d| d.kind == PropType::Boolean);
                let writable = def.and_then(|d| d.writable) == Some(true);
                let reading = match (boolean, writable) {
                    (true, false) => format!("true while {bind} is truthy"),
                    (true, true) => {
                        format!("true while {bind} is truthy, and user input writes {bind}")
                    }
                    (false, false) => format!("reads {bind}"),
                    (false, true) => format!("reads and writes {bind}"),
                };
                format!("{reading}{}", self.origin(bind))
            }
            Value::Token(token) => format!("design token {token}"),
            Value::String(s) => format!("always {}", serde_json::Value::String(s.clone())),
            Value::Number(_) | Value::Bool(_) => format!("always {}", format_value(value)),
        }
    }

    fn loop_sentence(&self, items: Option<&Value>, variable: Option<&Value>) -> String {
        let over = match items {
            Some(Value::Bind { bind, not: false }) => {
                format!("once per item of {bind}{}", self.origin(bind))
            }
            Some(other) => format!(
                "over {}, which is not an array binding",
                format_value(other)
            ),
            None => "over nothing: `in` is missing".to_owned(),
        };
        match variable {
            Some(Value::String(name)) => format!("repeats its children {over}, as ${name}"),
            _ => format!("repeats its children {over}, with no loop variable"),
        }
    }

    /// Names the array a loop variable walks, so `$todo.title` reads without the `<each>` in view.
    fn origin(&self, bind: &str) -> String {
        let name = bind
            .strip_prefix('$')
            .and_then(|rest| rest.split('.').next())
            .filter(|name| !name.is_empty());
        let scope = name.and_then(|name| self.scopes.iter().rev().find(|s| s.name == name));
        match scope {
            Some(scope) => format!(" (${} is the current item of {})", scope.name, scope.items),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseOptions, parse};

    const FIXTURE: &str = include_str!("../tests/fixtures/differential.json");

    fn catalog() -> Catalog {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        serde_json::from_value(fixture["catalogs"]["test"].clone()).unwrap()
    }

    fn doc(markup: &str, catalog: Option<&Catalog>) -> Document {
        let result = parse(
            markup,
            &ParseOptions {
                catalog,
                ..Default::default()
            },
        );
        result
            .document
            .unwrap_or_else(|| panic!("{:?}", result.diagnostics))
    }

    fn lines(markup: &str) -> Vec<String> {
        let catalog = catalog();
        explain(&doc(markup, Some(&catalog)), Some(&catalog))
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn changes(before: &str, after: &str) -> Vec<String> {
        let catalog = catalog();
        let (before, after) = (doc(before, Some(&catalog)), doc(after, Some(&catalog)));
        explain_changes(&before, &after, Some(&catalog))
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn login(disabled: &str) -> String {
        format!(
            "<screen id=\"login\" weft=\"0.1\"><form id=\"f1\" on-submit=\"auth.submit\">\
             <field id=\"email\" label=\"Email\" value=\"{{$.email}}\"/>\
             <button id=\"go\" disabled=\"{disabled}\" submit=\"true\">Sign in</button>\
             </form></screen>"
        )
    }

    #[test]
    fn a_negated_binding_reads_as_true_while_the_path_is_falsy() {
        assert!(
            lines(&login("{!$.busy}")).contains(
                &"button#go disabled: true while $.busy is falsy (NOT $.busy)".to_owned()
            )
        );
    }

    #[test]
    fn a_plain_binding_on_a_boolean_prop_reads_as_true_while_the_path_is_truthy() {
        assert!(
            lines(&login("{$.busy}"))
                .contains(&"button#go disabled: true while $.busy is truthy".to_owned())
        );
    }

    #[test]
    fn the_login_inversion_reads_back_as_falsy_where_the_instruction_says_true() {
        // The benchmark's login.e2 answer: the condition moved to $.busy but kept its `!`.
        assert_eq!(
            changes(&login("{!$.email}"), &login("{!$.busy}")),
            [
                "button#go disabled changed: was true while $.email is falsy (NOT $.email); \
              now true while $.busy is falsy (NOT $.busy)"
            ]
        );
        assert_eq!(
            changes(&login("{!$.email}"), &login("{$.busy}")),
            [
                "button#go disabled changed: was true while $.email is falsy (NOT $.email); \
              now true while $.busy is truthy"
            ]
        );
    }

    #[test]
    fn explain_reads_bindings_events_and_tokens_in_document_order_and_leaves_literals_out() {
        let markup = "<screen id=\"s\" weft=\"0.1\"><stack id=\"col\" gap=\"{token.space.md}\" wrap=\"true\">\
             <text id=\"hi\" text=\"{$.greeting}\" tone=\"muted\"/>\
             <field id=\"name\" label=\"{$.nameLabel}\" value=\"{$.name}\" on-change=\"profile.edit\"/>\
             <dialog id=\"d\" label=\"Edit\" open=\"{$.editing}\" on-close=\"profile.cancel\"/>\
             </stack></screen>";
        assert_eq!(
            lines(markup),
            [
                "stack#col gap: design token space.md",
                "text#hi text: reads $.greeting",
                "field#name label: reads $.nameLabel",
                "field#name value: reads and writes $.name",
                "field#name on-change: runs action profile.edit",
                "dialog#d open: true while $.editing is truthy, and user input writes $.editing",
                "dialog#d on-close: runs action profile.cancel",
            ]
        );
    }

    #[test]
    fn loop_variables_name_the_array_they_walk() {
        let markup = "<screen id=\"s\" weft=\"0.1\"><list id=\"todos\">\
             <each id=\"todo-each\" as=\"todo\" in=\"{$.todos}\">\
             <item id=\"todo-item\" on-press=\"todo.open\"><text id=\"todo-title\" text=\"{$todo.title}\"/></item>\
             </each></list></screen>";
        assert_eq!(
            lines(markup),
            [
                "each#todo-each in: repeats its children once per item of $.todos, as $todo",
                "item#todo-item on-press: runs action todo.open for the current item of $.todos",
                "text#todo-title text: reads $todo.title ($todo is the current item of $.todos)",
            ]
        );
    }

    #[test]
    fn elements_without_an_id_are_named_by_their_diagnostic_path_through_slots() {
        let catalog = catalog();
        let mut document = doc(
            "<screen id=\"s\" weft=\"0.1\"><form id=\"f\"><slot name=\"footer\">\
             <link id=\"reset\" href=\"{$.resetUrl}\">Forgot?</link></slot></form></screen>",
            Some(&catalog),
        );
        let Some(Child::Node(form)) = document.root.children.first_mut() else {
            panic!("form");
        };
        let Some(Child::Node(link)) = form.slots.get_mut("footer").and_then(|l| l.first_mut())
        else {
            panic!("link");
        };
        link.id = None;
        let readback = explain(&document, Some(&catalog));
        assert_eq!(
            readback.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["/screen#s/form#f/slot[footer]/link[0] href: reads $.resetUrl"]
        );
        assert_eq!(readback[0].path, readback[0].target);
    }

    #[test]
    fn without_a_catalog_a_plain_binding_reads_as_a_read() {
        let document = doc(&login("{$.busy}"), None);
        let readback: Vec<String> = explain(&document, None)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(readback.contains(&"button#go disabled: reads $.busy".to_owned()));
    }

    #[test]
    fn explain_changes_lists_added_removed_and_changed_props_and_events_only() {
        let before = "<screen id=\"s\" weft=\"0.1\">\
             <button id=\"a\" disabled=\"true\" on-press=\"x.go\">A</button>\
             <button id=\"b\" variant=\"primary\">B</button>\
             <button id=\"c\" disabled=\"{$.c}\">C</button></screen>";
        let after = "<screen id=\"s\" weft=\"0.1\">\
             <button id=\"c\" disabled=\"{$.c}\">C</button>\
             <button id=\"a\" disabled=\"{$.busy}\">A</button>\
             <button id=\"b\" variant=\"secondary\" on-press=\"x.b\">B</button>\
             <button id=\"d\" disabled=\"{!$.ready}\">D</button></screen>";
        assert_eq!(
            changes(before, after),
            [
                "button#a disabled changed: was always true; now true while $.busy is truthy",
                "button#b variant changed: was always \"primary\"; now always \"secondary\"",
                "button#b on-press added: runs action x.b",
                "button#d disabled added: true while $.ready is falsy (NOT $.ready)",
                "button#a on-press removed: was runs action x.go",
            ]
        );
    }

    #[test]
    fn renaming_a_loop_array_lists_the_loop_but_not_the_bindings_inside_it() {
        let screen = |items: &str| {
            format!(
                "<screen id=\"s\" weft=\"0.1\"><list id=\"l\"><each id=\"e\" as=\"t\" in=\"{{{items}}}\">\
                 <item id=\"i\"><text id=\"x\" text=\"{{$t.title}}\"/></item></each></list></screen>"
            )
        };
        assert_eq!(
            changes(&screen("$.todos"), &screen("$.done")),
            [
                "each#e in changed: was repeats its children once per item of $.todos, as $t; \
              now repeats its children once per item of $.done, as $t"
            ]
        );
    }

    #[test]
    fn an_unchanged_document_has_no_changes() {
        assert!(changes(&login("{!$.email}"), &login("{!$.email}")).is_empty());
    }
}
