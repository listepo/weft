//! Rendered HTML → Weft (SPEC §9, "From a running UI"): pages from a Weft renderer, whose
//! `data-weft-id` attributes give the ids back, or any semantic HTML. The HTML is untrusted: it is
//! only parsed, never run, and size, depth and node count are bounded.

use std::collections::{HashMap, HashSet};

use weft_core::{ARIA_ROLES, Catalog, Diagnostic, is_id};
use weft_import::{
    BuildOptions, ImportResult, LossKind, MAX_DEPTH, MAX_NODES, Note, Scalar, Sem, build_document,
    is_js_space, js_length, js_number_from, js_prefix, js_trim, limit_reached, squash,
};

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

struct Ctx<'d> {
    dom: &'d Dom,
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
                    self.notes.insert(el, notes);
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
        let role = role_of(dom, el, tag);
        // Hidden content is not part of the UI, except inactive tab panels, which hold a tab's
        // content.
        if attr("hidden").is_some() && role != "tabpanel" {
            return None;
        }
        if tag == "input" && attr("type").map(str::to_lowercase).as_deref() == Some("hidden") {
            return None;
        }
        if (tag == "img" && role == "presentation") || tag == "caption" {
            return None;
        }

        let mut s = Sem::new(role, self.acc_name(el, tag, role, label));
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

        if id.is_some()
            && role == "generic"
            && attr("role").is_none()
            && matches!(tag, "div" | "span")
        {
            layout(&mut s, dom, el);
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
        }
        Some(s)
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

fn role_of(dom: &Dom, el: usize, tag: &str) -> &'static str {
    let attr = |key: &str| dom.attr(el, key);
    for r in split_ws(js_trim(attr("role").unwrap_or(""))) {
        if let Some(known) = ARIA_ROLES.iter().find(|k| **k == r) {
            return known;
        }
    }
    match tag {
        "a" => {
            if attr("href").is_some() {
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

/// Weft renderer markup for the role-less layout and text kinds (SPEC §9: they add no node of
/// their own), recognised only on elements that carry a Weft id.
fn layout(s: &mut Sem, dom: &Dom, el: usize) {
    let style = parse_style(dom.attr(el, "style"));
    let get = |key: &str| style.get(key).map(String::as_str);
    let has_elements = dom.elements(el).next().is_some();
    if get("display") == Some("grid") {
        s.kind = Some("grid".into());
        let columns = repeat_count(get("grid-template-columns").unwrap_or("")).unwrap_or("1");
        s.props.insert("columns".into(), columns.into());
    } else if get("display") == Some("flex") || has_elements {
        s.kind = Some("stack".into());
        if get("flex-direction") == Some("row") {
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
    if let Some(gap) = get("gap") {
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

fn find_body(dom: &Dom) -> Option<usize> {
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

/// Imports rendered HTML. Never fails: what cannot be read is reported in the result.
pub fn from_dom(html: &str, catalog: &Catalog) -> ImportResult {
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut text = html;
    if js_length(text) > MAX_HTML_LENGTH {
        text = js_prefix(text, MAX_HTML_LENGTH);
        limit_reached(
            &mut diagnostics,
            "#",
            &format!("is longer than {MAX_HTML_LENGTH} characters"),
        );
    }
    let dom = parse_html(text);
    let mut ctx = Ctx {
        dom: &dom,
        by_html_id: HashMap::new(),
        label_for: HashMap::new(),
        ids: HashMap::new(),
        reserved: Vec::new(),
        notes: HashMap::new(),
        consumed: HashSet::new(),
        forms: 0,
        nodes: 0,
        truncated: false,
    };
    ctx.prescan();
    let body = find_body(&dom).unwrap_or(DOCUMENT);
    let sems = ctx.children(body, 0, None);
    if ctx.truncated {
        limit_reached(
            &mut diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    let built = build_document(
        &sems,
        BuildOptions {
            catalog,
            reserved: ctx.reserved,
            diagnostics,
        },
    );
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
