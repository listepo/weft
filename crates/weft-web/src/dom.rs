//! Rendered HTML → Weft (SPEC §9, "From a running UI"): pages from a Weft renderer, whose
//! `data-weft-id` attributes give the ids back, or any semantic HTML. The HTML is untrusted: it is
//! only parsed, never run, and size, depth and node count are bounded.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use weft_catalog::Token;
use weft_core::{
    ARIA_ROLES, Catalog, Child, Diagnostic, Node, Value, is_action, is_id, read_value,
};
use weft_import::{
    BuildOptions, Built, ImportResult, LossKind, MAX_DEPTH, MAX_NODES, Note, Scalar, Sem,
    build_document, is_js_space, js_length, js_number_from, js_prefix, js_trim, limit_reached,
    squash,
};

use crate::html::token_var;

use crate::tree::{DOCUMENT, Dom, HNode, parse_html};

/// In UTF-16 code units, as JavaScript measures the string the TypeScript API receives.
pub const MAX_HTML_LENGTH: usize = 5_000_000;

/// HTML-AAM implicit roles of the elements user interfaces commonly use. Elements not listed (and
/// the text-level ones such as strong or code) are generic: their content is read in place.
pub const IMPLICIT_ROLES: &[(&str, &str)] = &[
    ("article", "article"),
    ("aside", "complementary"),
    ("button", "button"),
    ("dialog", "dialog"),
    ("fieldset", "group"),
    // A form or section is a landmark only when named; here they always keep their kind.
    ("form", "form"),
    ("h1", "heading"),
    ("h2", "heading"),
    ("h3", "heading"),
    ("h4", "heading"),
    ("h5", "heading"),
    ("h6", "heading"),
    ("hr", "separator"),
    ("li", "listitem"),
    ("main", "main"),
    ("nav", "navigation"),
    ("ol", "list"),
    ("option", "option"),
    ("output", "status"),
    ("p", "paragraph"),
    ("progress", "progressbar"),
    ("section", "region"),
    ("table", "table"),
    ("tbody", "rowgroup"),
    ("td", "cell"),
    ("textarea", "textbox"),
    ("tfoot", "rowgroup"),
    ("thead", "rowgroup"),
    ("tr", "row"),
    ("ul", "list"),
];

/// `<input>` roles by `type`; any other type (text, email, password, tel, url, …) is a textbox.
pub const INPUT_ROLES: &[(&str, &str)] = &[
    ("button", "button"),
    ("checkbox", "checkbox"),
    ("image", "button"),
    ("number", "spinbutton"),
    ("radio", "radio"),
    ("range", "slider"),
    ("reset", "button"),
    ("search", "searchbox"),
    ("submit", "button"),
];

const SKIPPED: &[&str] = &[
    "base", "head", "link", "meta", "noscript", "script", "style", "template", "title",
];
const CONTROLS: &[&str] = &["input", "select", "textarea", "button"];
const ALIGN: &[(&str, &str)] = &[
    ("flex-start", "start"),
    ("center", "center"),
    ("flex-end", "end"),
    ("stretch", "stretch"),
];

fn lookup(table: &[(&str, &'static str)], key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// What `s.split(/\s+/)` returns: the pieces between whitespace runs, empty at either end when the
/// string starts or ends with whitespace.
fn split_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_space = false;
    for (i, c) in s.char_indices() {
        if is_js_space(c) {
            if !in_space {
                out.push(&s[start..i]);
                in_space = true;
            }
        } else if in_space {
            start = i;
            in_space = false;
        }
    }
    out.push(if in_space { "" } else { &s[start..] });
    out
}

/// The id a rendered instance `id[2][0]` gets in the import: the indexes appended with hyphens.
pub fn instance_id(raw: &str) -> Option<String> {
    let base_end = raw.find('[').unwrap_or(raw.len());
    let (base, mut rest) = raw.split_at(base_end);
    let mut chars = base.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let mut out = base.to_owned();
    while !rest.is_empty() {
        let inner = rest.strip_prefix('[')?;
        let close = inner.find(']')?;
        let digits = &inner[..close];
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        out.push('-');
        out.push_str(digits);
        rest = &inner[close + 1..];
    }
    Some(out)
}

/// What source code states and a rendered page does not: bindings (`data-bind`), actions
/// (`data-action`), repetition (`<template data-each>`), slots (`data-weft-slot`, `<footer>`,
/// `<template data-empty>`), token references, presentation states and hidden elements. These are
/// the conventions of corpus/README.md, which the HTML generator writes too.
pub(crate) struct Conventions {
    /// `--weft-…` custom property → token path.
    vars: HashMap<String, String>,
    paths: HashSet<String>,
}

impl Conventions {
    pub(crate) fn new(tokens: &IndexMap<String, Token>) -> Self {
        Conventions {
            vars: tokens
                .keys()
                .filter_map(|path| token_var(path).map(|var| (var, path.clone())))
                .collect(),
            paths: tokens.keys().cloned().collect(),
        }
    }
}

/// `prop:value; prop:value` lists of `data-bind` and `data-action`.
fn entries(list: &str) -> impl Iterator<Item = (&str, &str)> {
    list.split(';').filter_map(|entry| {
        let (key, value) = entry.split_once(':')?;
        let (key, value) = (js_trim(key), js_trim(value));
        (!key.is_empty() && !value.is_empty()).then_some((key, value))
    })
}

/// `$.path` or `!$.path` as a binding value.
fn bind_value(raw: &str) -> Option<Value> {
    match read_value(&format!("{{{raw}}}"), None) {
        Ok(value @ Value::Bind { .. }) => Some(value),
        _ => None,
    }
}

struct Ctx<'d> {
    dom: &'d Dom,
    conventions: Option<&'d Conventions>,
    by_html_id: HashMap<&'d str, usize>,
    label_for: HashMap<&'d str, usize>,
    ids: HashMap<usize, String>,
    /// The Weft ids in document order: generated ids must avoid all of them.
    reserved: Vec<String>,
    notes: HashMap<usize, Vec<Note>>,
    consumed: HashSet<usize>,
    /// Open <form> elements around the current one: only there does a submit button submit.
    forms: usize,
    nodes: usize,
    truncated: bool,
}

impl<'d> Ctx<'d> {
    fn attr(&self, el: usize, key: &str) -> Option<&'d str> {
        self.dom.attr(el, key)
    }

    fn is_control(&self, el: usize) -> bool {
        self.dom.name(el).is_some_and(|n| CONTROLS.contains(&n))
    }

    /// One pass over the whole tree before conversion: ids and labels may be referenced from
    /// anywhere, and generated ids must avoid every id the page carries.
    fn prescan(&mut self) {
        let dom = self.dom;
        let mut seen = HashSet::new();
        let mut stack: Vec<usize> = dom.elements(DOCUMENT).rev().collect();
        let mut count = 0;
        while count < MAX_NODES * 4 {
            count += 1;
            let Some(el) = stack.pop() else { break };
            if let Some(id) = dom.attr(el, "id") {
                self.by_html_id.entry(id).or_insert(el);
            }
            if dom.name(el) == Some("label")
                && let Some(target) = dom.attr(el, "for")
            {
                self.label_for.entry(target).or_insert(el);
            }
            if let Some(raw) = dom.attr(el, "data-weft-id") {
                let shown = js_prefix(raw, 80);
                let mut notes = Vec::new();
                match instance_id(raw).filter(|id| is_id(id)) {
                    None => notes.push(Note {
                        kind: LossKind::Ids,
                        note: format!(
                            "data-weft-id \"{shown}\" is not a valid id; a new id is generated"
                        ),
                    }),
                    Some(id) if seen.contains(&id) => notes.push(Note {
                        kind: LossKind::Ids,
                        note: format!(
                            "data-weft-id \"{shown}\" repeats an earlier id; a new id is generated"
                        ),
                    }),
                    Some(id) => {
                        seen.insert(id.clone());
                        if id != raw {
                            notes.push(Note {
                                kind: LossKind::Repetition,
                                note: format!(
                                    "instance {raw} of a repeated element is imported as the static element {id}"
                                ),
                            });
                        }
                        self.reserved.push(id.clone());
                        self.ids.insert(el, id);
                    }
                }
                if !notes.is_empty() {
                    self.notes.entry(el).or_default().extend(notes);
                }
            }
            stack.extend(dom.elements(el).rev());
        }
    }

    /// Text a person sees in a subtree, without hidden parts; `skip_controls` leaves out the
    /// control a label wraps.
    fn text_content(&self, n: usize, depth: usize, skip_controls: bool, out: &mut String) {
        match self.dom.nodes.get(n) {
            Some(HNode::Text(t)) => out.push_str(t),
            Some(HNode::Element { name, children, .. }) => {
                if depth > MAX_DEPTH
                    || SKIPPED.contains(&name.as_str())
                    || self.attr(n, "aria-hidden") == Some("true")
                    || self.attr(n, "hidden").is_some()
                    || (skip_controls && CONTROLS.contains(&name.as_str()))
                {
                    return;
                }
                for &c in children {
                    self.text_content(c, depth + 1, skip_controls, out);
                }
            }
            None => {}
        }
    }

    fn text_of(&self, n: usize, skip_controls: bool) -> String {
        let mut out = String::new();
        self.text_content(n, 0, skip_controls, &mut out);
        out
    }

    /// A simplified accessible name (not full accname 1.2): aria-labelledby, aria-label, the alt
    /// of an image, the label of a form control (`<label for>` or a wrapping `<label>`), a table
    /// caption, the value of an input button, then title. Names computed from content are taken
    /// from the content itself where the catalog says a kind shows text.
    fn acc_name(&self, el: usize, tag: &str, role: &str, label: Option<usize>) -> String {
        let by: Vec<String> = split_ws(self.attr(el, "aria-labelledby").unwrap_or(""))
            .into_iter()
            .filter(|id| !id.is_empty())
            .filter_map(|id| self.by_html_id.get(id))
            .map(|&target| self.text_of(target, false))
            .collect();
        let by = squash(&by.join(" "));
        if !by.is_empty() {
            return by;
        }
        let aria_label = squash(self.attr(el, "aria-label").unwrap_or(""));
        if !aria_label.is_empty() {
            return aria_label;
        }
        if role == "img"
            && let Some(alt) = self.attr(el, "alt")
        {
            return squash(alt);
        }
        if CONTROLS.contains(&tag) && tag != "button" {
            let owner = self
                .attr(el, "id")
                .and_then(|id| self.label_for.get(id).copied())
                .or(label);
            let text = owner.map_or_else(String::new, |o| squash(&self.text_of(o, true)));
            if !text.is_empty() {
                return text;
            }
            if tag == "input" && role == "button" {
                return squash(self.attr(el, "value").unwrap_or(""));
            }
        }
        if tag == "table"
            && let Some(caption) = self
                .dom
                .elements(el)
                .find(|&c| self.dom.name(c) == Some("caption"))
        {
            return squash(&self.text_of(caption, false));
        }
        squash(self.attr(el, "title").unwrap_or(""))
    }

    fn element(&mut self, el: usize, depth: usize, label: Option<usize>) -> Option<Sem> {
        let dom = self.dom;
        let tag = dom.name(el)?.to_lowercase();
        let tag = tag.as_str();
        let attr = |key: &str| dom.attr(el, key);
        if tag == "template" && self.conventions.is_some() && !self.consumed.contains(&el) {
            return self.template(el, depth, label);
        }
        if SKIPPED.contains(&tag)
            || self.consumed.contains(&el)
            || attr("aria-hidden") == Some("true")
        {
            return None;
        }
        if depth > MAX_DEPTH || {
            self.nodes += 1;
            self.nodes > MAX_NODES
        } {
            self.truncated = true;
            return None;
        }
        let role = role_of(dom, el, tag, self.conventions.is_some());
        // Hidden content is not part of the UI, except inactive tab panels, which hold a tab's
        // content.
        let hidden = attr("hidden").is_some() && role != "tabpanel";
        if hidden && self.conventions.is_none() {
            return None;
        }
        if tag == "input" && attr("type").map(str::to_lowercase).as_deref() == Some("hidden") {
            return None;
        }
        if (tag == "img" && role == "presentation") || tag == "caption" {
            return None;
        }

        let mut s = Sem::new(role, self.acc_name(el, tag, role, label));
        if hidden {
            s.values.insert("hidden".into(), Value::Bool(true));
        }
        let id = self.ids.get(&el).cloned();
        s.id.clone_from(&id);
        if let Some(notes) = self.notes.get(&el) {
            s.notes = notes.clone();
        }
        s.reference = attr("id").map(str::to_owned);
        s.labelled_by =
            attr("aria-labelledby").map(|v| split_ws(v).into_iter().map(str::to_owned).collect());

        let states = &mut s.states;
        if attr("disabled").is_some() || attr("aria-disabled") == Some("true") {
            states.insert("disabled".into(), Scalar::Bool(true));
        }
        if attr("checked").is_some() || attr("aria-checked") == Some("true") {
            states.insert("checked".into(), Scalar::Bool(true));
        }
        if (tag == "option" && attr("selected").is_some()) || attr("aria-selected") == Some("true")
        {
            states.insert("selected".into(), Scalar::Bool(true));
        }
        let heading = tag
            .strip_prefix('h')
            .filter(|d| d.len() == 1 && ('1'..='6').contains(&d.chars().next().unwrap_or('0')));
        if let Some(level) = heading.or_else(|| attr("aria-level"))
            && !level.is_empty()
            && level.bytes().all(|b| b.is_ascii_digit())
        {
            states.insert("level".into(), Scalar::Number(js_number_from(level)));
        }
        if attr("aria-busy") == Some("true") {
            states.insert("busy".into(), Scalar::Bool(true));
        }

        let p = &mut s.props;
        let mut set = |key: &str, value: &str| {
            p.insert(key.into(), value.into());
        };
        for (from, to) in [
            ("data-state", "state"),
            ("data-variant", "variant"),
            ("data-tone", "tone"),
            ("aria-sort", "sort"),
        ] {
            if let Some(v) = attr(from) {
                set(to, v);
            }
        }
        if let Some(modal) = attr("aria-modal") {
            set("modal", if modal == "true" { "true" } else { "false" });
        }
        if attr("required").is_some() || attr("aria-required") == Some("true") {
            set("required", "true");
        }
        if let Some(v) = attr("placeholder") {
            set("placeholder", v);
        }
        if tag == "a"
            && let Some(v) = attr("href")
        {
            set("href", v);
        }
        if tag == "img"
            && let Some(v) = attr("src")
        {
            set("src", v);
        }
        if tag == "ol" {
            set("ordered", "true");
        }
        if tag == "input" {
            let kind = attr("type").unwrap_or("text").to_lowercase();
            if matches!(role, "textbox" | "spinbutton" | "searchbox") && kind != "text" {
                set("type", &kind);
            }
            if let Some(v) = attr("value")
                && role != "checkbox"
                && role != "switch"
            {
                set("value", v);
            }
            if (kind == "submit" || kind == "image") && self.forms > 0 {
                set("submit", "true");
            }
        }
        if tag == "textarea" {
            set("type", "multiline");
            set("value", &self.text_of(el, false));
        }
        if tag == "option" {
            match attr("value") {
                Some(v) => set("value", v),
                None => set("value", &squash(&self.text_of(el, false))),
            }
        }
        // HTML: a button without a type submits its form.
        if tag == "button"
            && attr("type").unwrap_or("submit").to_lowercase() == "submit"
            && self.forms > 0
        {
            set("submit", "true");
        }

        // An error message the control points at is that control's `error`, not separate text.
        let described: Vec<usize> = split_ws(attr("aria-describedby").unwrap_or(""))
            .into_iter()
            .filter(|r| !r.is_empty())
            .filter_map(|r| self.by_html_id.get(r).copied())
            .collect();
        if !described.is_empty() && CONTROLS.contains(&tag) {
            let texts: Vec<String> = described.iter().map(|&d| self.text_of(d, false)).collect();
            set("error", &squash(&texts.join(" ")));
            self.consumed.extend(described);
        }
        if attr("aria-invalid") == Some("true") && !s.props.contains_key("error") {
            s.states.insert("invalid".into(), Scalar::Bool(true));
        }

        if let Some(conventions) = self.conventions {
            self.read_conventions(conventions, el, tag, role, label, &mut s);
        }
        let styled = self.conventions.is_some()
            && (layout_class(attr("class")).is_some()
                || matches!(
                    parse_style(attr("style"))
                        .get("display")
                        .map(String::as_str),
                    Some("flex" | "grid")
                ));
        if (id.is_some() || styled)
            && role == "generic"
            && attr("role").is_none()
            && matches!(tag, "div" | "span")
        {
            layout(&mut s, dom, el, self.conventions);
        }
        if self.conventions.is_some() {
            left_out_style(&mut s, attr("style"));
        }

        if !matches!(tag, "input" | "textarea" | "img") {
            if tag == "form" {
                self.forms += 1;
            }
            // What a control holds (a select's options) is its own content, not the name a
            // wrapping label gives it.
            let within = if tag == "label" {
                Some(el)
            } else if self.is_control(el) {
                None
            } else {
                label
            };
            s.children = self.children(el, depth, within);
            if tag == "form" {
                self.forms -= 1;
            }
            if self.conventions.is_some() {
                lift_bound_text(&mut s);
                // A bare wrapper that shows bound text is a text of its own, not one to dissolve.
                if s.kind.is_none()
                    && role == "generic"
                    && attr("role").is_none()
                    && s.values.contains_key("text")
                {
                    // Text beside the binding is what the page shows before data arrives.
                    if s.children.iter().all(|c| c.role == "text") {
                        s.children.clear();
                        s.kind = Some("text".into());
                    } else {
                        s.values.shift_remove("text");
                        s.notes.push(Note {
                            kind: LossKind::Bindings,
                            note:
                                "a text binding on an element that also holds content is left out"
                                    .into(),
                        });
                    }
                }
                if role == "columnheader" {
                    lift_header_button(&mut s);
                }
                let slot = attr("data-weft-slot").or((tag == "footer").then_some("footer"));
                if let Some(slot) = slot {
                    s.role = "generic".into();
                    for c in &mut s.children {
                        c.slot = Some(slot.to_owned());
                    }
                }
            }
        }
        Some(s)
    }

    /// `<template data-each>` repeats its content; `<template data-empty>` is the empty slot.
    fn template(&mut self, el: usize, depth: usize, label: Option<usize>) -> Option<Sem> {
        let dom = self.dom;
        if depth > MAX_DEPTH || {
            self.nodes += 1;
            self.nodes > MAX_NODES
        } {
            self.truncated = true;
            return None;
        }
        let mut s = Sem::new("generic", "");
        if let Some(list) = dom.attr(el, "data-each") {
            s.kind = Some("each".into());
            s.id = self.ids.get(&el).cloned();
            if let Some(notes) = self.notes.get(&el) {
                s.notes = notes.clone();
            }
            if let Some(value) = bind_value(list) {
                s.values.insert("in".into(), value);
            }
            if let Some(name) = dom.attr(el, "data-as") {
                s.props.insert("as".into(), name.into());
            }
            s.children = self.children(el, depth, label);
        } else if dom.attr(el, "data-empty").is_some() {
            s.children = self.children(el, depth, label);
            for c in &mut s.children {
                c.slot = Some("empty".into());
            }
        } else {
            return None;
        }
        Some(s)
    }

    fn read_conventions(
        &mut self,
        conventions: &Conventions,
        el: usize,
        tag: &str,
        role: &str,
        label: Option<usize>,
        s: &mut Sem,
    ) {
        let dom = self.dom;
        let attr = |key: &str| dom.attr(el, key);
        // Values a page needs and source does not state: the `#` of a link without a literal
        // href, the modal default of a dialog, and the tab a page shows first.
        if s.props.get("href").is_some_and(|h| h == "#") {
            s.props.shift_remove("href");
        }
        if s.props.get("modal").is_some_and(|m| m == "true") {
            s.props.shift_remove("modal");
        }
        if role == "tab" {
            s.states.shift_remove("selected");
        }
        for (prop, raw) in entries(attr("data-bind").unwrap_or("")) {
            match bind_value(raw) {
                Some(value) => {
                    s.values.insert(prop.to_owned(), value);
                }
                None => s.notes.push(Note {
                    kind: LossKind::Bindings,
                    note: format!(
                        "{prop} is bound to {}, which is not a data path",
                        js_prefix(raw, 80)
                    ),
                }),
            }
        }
        for (event, action) in entries(attr("data-action").unwrap_or("")) {
            if is_action(action) {
                s.on.insert(event.to_owned(), action.to_owned());
            } else {
                s.notes.push(Note {
                    kind: LossKind::Actions,
                    note: format!("{} is not an action name", js_prefix(action, 80)),
                });
            }
        }
        if let Some(kind) = attr("data-weft-kind") {
            s.kind = Some(kind.to_owned());
        }
        for (key, value) in dom.attrs(el) {
            let Some(prop) = key.strip_prefix("data-prop-") else {
                continue;
            };
            match read_value(value, None) {
                Ok(Value::Token(token)) if conventions.paths.contains(&token) => {
                    s.values.insert(prop.to_owned(), Value::Token(token));
                }
                Ok(Value::String(text)) => {
                    s.props.insert(prop.to_owned(), text);
                }
                _ => s.notes.push(Note {
                    kind: LossKind::Props,
                    note: format!("{key}=\"{}\" is not a value", js_prefix(value, 80)),
                }),
            }
        }
        if let Some(align) = attr("data-align") {
            s.props.insert("align".into(), align.into());
        }
        if attr("data-wrap").is_some() {
            s.props.insert("wrap".into(), "true".into());
        }
        if tag == "dialog" {
            let open = if attr("open").is_some() {
                "true"
            } else {
                "false"
            };
            s.props.insert("open".into(), open.into());
        }
        if tag == "fieldset"
            && let Some(legend) = dom.elements(el).find(|&c| dom.name(c) == Some("legend"))
        {
            if s.name.is_empty() {
                s.name = squash(&self.text_of(legend, false));
            }
            self.consumed.insert(legend);
        }
        // A control named by bound text in its label: the label is that binding.
        if CONTROLS.contains(&tag) && s.name.is_empty() && !s.values.contains_key("label") {
            let owner = attr("id")
                .and_then(|id| self.label_for.get(id).copied())
                .or(label);
            if let Some(bound) = owner.and_then(|o| self.bound_text(o, 0)) {
                s.values.insert("label".into(), bound);
            }
        }
    }

    /// The first `text` binding in a subtree.
    fn bound_text(&self, el: usize, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        let own = entries(self.attr(el, "data-bind").unwrap_or(""))
            .find(|(prop, _)| *prop == "text")
            .and_then(|(_, raw)| bind_value(raw));
        own.or_else(|| {
            self.dom
                .elements(el)
                .find_map(|c| self.bound_text(c, depth + 1))
        })
    }

    fn children(&mut self, el: usize, depth: usize, label: Option<usize>) -> Vec<Sem> {
        let dom = self.dom;
        let mut out = Vec::new();
        for &c in dom.children(el) {
            match &dom.nodes[c] {
                HNode::Text(t) => {
                    // Text inside a label is the name of its control, not content of its own.
                    if label.is_none() && !js_trim(t).is_empty() {
                        out.push(Sem::text(t.clone()));
                    }
                }
                HNode::Element { name, .. } if name == "br" => out.push(Sem::text(" ")),
                HNode::Element { .. } => {
                    if let Some(s) = self.element(c, depth + 1, label) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }
}

fn role_of(dom: &Dom, el: usize, tag: &str, source: bool) -> &'static str {
    let attr = |key: &str| dom.attr(el, key);
    for r in split_ws(js_trim(attr("role").unwrap_or(""))) {
        if let Some(known) = ARIA_ROLES.iter().find(|k| **k == r) {
            return known;
        }
    }
    match tag {
        "a" => {
            // In source, a bound href is a link too.
            let bound =
                source && entries(attr("data-bind").unwrap_or("")).any(|(prop, _)| prop == "href");
            if attr("href").is_some() || bound {
                "link"
            } else {
                "generic"
            }
        }
        "input" => {
            let kind = attr("type").unwrap_or("text").to_lowercase();
            lookup(INPUT_ROLES, &kind).unwrap_or("textbox")
        }
        "img" => {
            if attr("alt") == Some("") {
                "presentation"
            } else {
                "img"
            }
        }
        "th" => {
            if attr("scope") == Some("row") {
                "rowheader"
            } else {
                "columnheader"
            }
        }
        "select" => {
            if attr("multiple").is_some() || js_number_from(attr("size").unwrap_or("")) > 1.0 {
                "listbox"
            } else {
                "combobox"
            }
        }
        _ => lookup(IMPLICIT_ROLES, tag).unwrap_or("generic"),
    }
}

/// Declarations of a `style` attribute; a repeated property keeps its last value.
fn parse_style(style: Option<&str>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for decl in style.unwrap_or("").split(';') {
        if let Some(colon) = decl.find(':')
            && colon > 0
        {
            out.insert(
                js_trim(&decl[..colon]).to_lowercase(),
                js_trim(&decl[colon + 1..]).to_owned(),
            );
        }
    }
    out
}

/// `repeat(N,` at the start of `grid-template-columns`: the column count a Weft grid renders.
fn repeat_count(value: &str) -> Option<&str> {
    let rest = js_trim_start(value.strip_prefix("repeat(")?);
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let (digits, rest) = rest.split_at(end);
    (!digits.is_empty() && js_trim_start(rest).starts_with(',')).then_some(digits)
}

fn js_trim_start(s: &str) -> &str {
    s.trim_start_matches(is_js_space)
}

/// Inline style Weft has no prop for: what a stack or grid reads from it is its layout, the rest
/// is a loss rather than silently gone.
fn left_out_style(s: &mut Sem, style: Option<&str>) {
    const LAYOUT: &[&str] = &[
        "display",
        "flex-direction",
        "align-items",
        "flex-wrap",
        "gap",
        "grid-template-columns",
    ];
    let layout = matches!(s.kind.as_deref(), Some("stack" | "grid"));
    let mut left: Vec<String> = parse_style(style)
        .into_iter()
        .filter(|(k, _)| !(layout && LAYOUT.contains(&k.as_str())))
        // How an element flows is the renderer's choice (the JSX generator sets links inline-block).
        .filter(|(k, v)| {
            !(k == "display" && matches!(v.as_str(), "block" | "inline" | "inline-block"))
        })
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();
    if left.is_empty() {
        return;
    }
    // Declarations come from a map; sorted, the note reads the same on every run.
    left.sort();
    s.notes.push(Note {
        kind: LossKind::Layout,
        note: format!("style {} is left out", left.join("; ")),
    });
}

/// An element whose only content is one bound text run shows that binding itself.
fn lift_bound_text(s: &mut Sem) {
    if s.values.contains_key("text") {
        return;
    }
    if let [only] = s.children.as_mut_slice()
        && (only.kind.is_none()
            || (only.kind.as_deref() == Some("text")
                && only.id.is_none()
                && only.values.len() == 1))
        && only.role == "generic"
        && only.children.is_empty()
        && let Some(text) = only.values.shift_remove("text")
    {
        s.values.insert("text".into(), text);
        s.children.clear();
    }
}

/// A sortable column header holds a button; its press is the column's.
fn lift_header_button(s: &mut Sem) {
    if let [only] = s.children.as_mut_slice()
        && only.role == "button"
        && only.kind.is_none()
    {
        let button = std::mem::take(only);
        s.on = button.on;
        s.children = button.children;
        if s.name == button.name {
            s.name.clear();
        }
    }
}

/// The layout a class list names: the generator's `weft-stack`/`weft-row`/`weft-grid`, or the
/// corpus's `stack`/`row`.
fn layout_class(class: Option<&str>) -> Option<&'static str> {
    split_ws(class.unwrap_or(""))
        .into_iter()
        .find_map(|c| match c {
            "weft-grid" => Some("grid"),
            "weft-row" | "row" => Some("row"),
            "weft-stack" | "stack" => Some("stack"),
            _ => None,
        })
}

/// Weft renderer markup for the role-less layout and text kinds (SPEC §9: they add no node of
/// their own), recognised only on elements that carry a Weft id, or, in source, a layout class or
/// style.
fn layout(s: &mut Sem, dom: &Dom, el: usize, conventions: Option<&Conventions>) {
    let style = parse_style(dom.attr(el, "style"));
    let get = |key: &str| style.get(key).map(String::as_str);
    let has_elements = dom.elements(el).next().is_some();
    let class = conventions.and_then(|_| layout_class(dom.attr(el, "class")));
    if get("display") == Some("grid") || class == Some("grid") {
        s.kind = Some("grid".into());
        let columns = repeat_count(get("grid-template-columns").unwrap_or("")).unwrap_or("1");
        s.props.insert("columns".into(), columns.into());
    } else if get("display") == Some("flex") || has_elements || class.is_some() {
        s.kind = Some("stack".into());
        if get("flex-direction") == Some("row") || class == Some("row") {
            s.props.insert("direction".into(), "row".into());
        }
        if let Some(align) = get("align-items").and_then(|a| lookup(ALIGN, a)) {
            s.props.insert("align".into(), align.into());
        }
        if get("flex-wrap") == Some("wrap") {
            s.props.insert("wrap".into(), "true".into());
        }
    } else {
        s.kind = Some("text".into());
    }
    let gap_class = conventions.and_then(|c| {
        split_ws(dom.attr(el, "class").unwrap_or(""))
            .into_iter()
            .filter_map(|class| class.strip_prefix("gap-"))
            .map(|size| format!("space.{size}"))
            .find(|path| c.paths.contains(path))
    });
    let gap_var = conventions.and_then(|c| {
        let var = get("gap")?.strip_prefix("var(")?.strip_suffix(')')?;
        c.vars.get(js_trim(var)).cloned()
    });
    if let Some(token) = gap_var.or(gap_class) {
        s.values.insert("gap".into(), Value::Token(token));
    } else if let Some(gap) = get("gap") {
        s.notes.push(Note {
            kind: if gap.starts_with("var(--weft-") {
                LossKind::Tokens
            } else {
                LossKind::Layout
            },
            note: format!(
                "gap {} cannot be mapped back to a design token",
                js_prefix(gap, 80)
            ),
        });
    }
}

/// Never recoverable from rendered HTML, whatever it holds.
const DOM_LOSSES: &[(LossKind, &str)] = &[
    (
        LossKind::Bindings,
        "values are the resolved values the page shows, not bindings",
    ),
    (
        LossKind::Actions,
        "event handlers and their action names are not in the HTML",
    ),
    (
        LossKind::Tokens,
        "design token references are rendered as CSS and cannot be mapped back",
    ),
    (
        LossKind::Slots,
        "slot membership is not in the HTML; slot content is imported as default content",
    ),
    (
        LossKind::Hidden,
        "elements a renderer leaves out (hidden, closed dialogs) are not in the HTML",
    ),
];

pub(crate) fn find_body(dom: &Dom) -> Option<usize> {
    let mut queue: Vec<usize> = dom.elements(DOCUMENT).collect();
    let mut i = 0;
    while i < queue.len() && i < 64 {
        let el = queue[i];
        match dom.name(el) {
            Some("body") => return Some(el),
            Some("html") => queue.extend(dom.elements(el)),
            _ => {}
        }
        i += 1;
    }
    None
}

/// The page text within the length limit, and the diagnostic when it was cut.
pub(crate) fn bounded<'h>(html: &'h str, diagnostics: &mut Vec<Diagnostic>) -> &'h str {
    if js_length(html) > MAX_HTML_LENGTH {
        limit_reached(
            diagnostics,
            "#",
            &format!("is longer than {MAX_HTML_LENGTH} characters"),
        );
        return js_prefix(html, MAX_HTML_LENGTH);
    }
    html
}

/// Imports rendered HTML. Never fails: what cannot be read is reported in the result.
pub fn from_dom(html: &str, catalog: &Catalog) -> ImportResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let dom = parse_html(bounded(html, &mut diagnostics));
    let built = read_dom(&dom, catalog, None, HashMap::new(), diagnostics);
    let mut result = built.result;
    let mut losses: Vec<_> = DOM_LOSSES
        .iter()
        .map(|&(kind, note)| weft_import::Loss {
            kind,
            path: built.root_path.clone(),
            note: note.into(),
        })
        .collect();
    losses.append(&mut result.losses);
    result.losses = losses;
    result
}

/// The document a parsed page describes, read as rendered HTML or, with `conventions`, as source.
pub(crate) fn read_dom(
    dom: &Dom,
    catalog: &Catalog,
    conventions: Option<&Conventions>,
    notes: HashMap<usize, Vec<Note>>,
    mut diagnostics: Vec<Diagnostic>,
) -> Built {
    let mut ctx = Ctx {
        dom,
        conventions,
        by_html_id: HashMap::new(),
        label_for: HashMap::new(),
        ids: HashMap::new(),
        reserved: Vec::new(),
        notes,
        consumed: HashSet::new(),
        forms: 0,
        nodes: 0,
        truncated: false,
    };
    ctx.prescan();
    let body = find_body(dom).unwrap_or(DOCUMENT);
    let sems = ctx.children(body, 0, None);
    if ctx.truncated {
        limit_reached(
            &mut diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    let source_ids: HashSet<String> = ctx.reserved.iter().cloned().collect();
    let mut built = build_document(
        &sems,
        BuildOptions {
            catalog,
            reserved: ctx.reserved,
            diagnostics,
        },
    );
    if conventions.is_some() {
        let generated = count_nodes(&built.result.document.root, |n| {
            n.id.as_ref().is_none_or(|id| !source_ids.contains(id))
        });
        if generated > 0 {
            built.result.losses.insert(
                0,
                weft_import::Loss {
                    kind: LossKind::Ids,
                    path: built.root_path.clone(),
                    note: format!(
                        "{generated} elements carry no data-weft-id; their ids are generated"
                    ),
                },
            );
        }
    }
    built
}

/// Nodes of a document that match, counted without recursion.
pub(crate) fn count_nodes(root: &Node, matches: impl Fn(&Node) -> bool) -> usize {
    let mut pending = vec![root];
    let mut count = 0;
    while let Some(n) = pending.pop() {
        count += usize::from(matches(n));
        for child in n.children.iter().chain(n.slots.values().flatten()) {
            if let Child::Node(c) = child {
                pending.push(c);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_splits_like_javascript() {
        assert_eq!(split_ws("a  b"), ["a", "b"]);
        assert_eq!(split_ws(" a"), ["", "a"]);
        assert_eq!(split_ws("a "), ["a", ""]);
        assert_eq!(split_ws(""), [""]);
        assert_eq!(split_ws("  "), ["", ""]);
    }

    #[test]
    fn instances_become_static_ids() {
        assert_eq!(instance_id("row[2][0]").as_deref(), Some("row-2-0"));
        assert_eq!(instance_id("a").as_deref(), Some("a"));
        assert_eq!(instance_id("a[]"), None);
        assert_eq!(instance_id("1a"), None);
        assert_eq!(instance_id("a[1]x"), None);
        assert_eq!(instance_id("a.b"), None);
    }

    #[test]
    fn grid_columns_read_the_repeat_count() {
        assert_eq!(repeat_count("repeat( 3 , 1fr)"), Some("3"));
        assert_eq!(repeat_count("repeat(x, 1fr)"), None);
        assert_eq!(repeat_count("1fr 1fr"), None);
    }
}
