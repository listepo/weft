//! The instance tree the accessibility tree is built from, as the reference renderer builds it
//! (`@weft/render-react`, `expand.ts`): `each` expanded with instance ids, `hidden` nodes removed,
//! the `text` prop turned into content, and every node paired with the scope its bindings resolve
//! in; then the readings of props both the renderer and the tree depend on. The data is untrusted:
//! lookups follow own keys only, and nothing here panics.

use std::borrow::Cow;

use serde_json::Value as Json;

use crate::controls::{
    self, Bounds, DateType, Span, date_text, is_float, slider_span, slider_value, stepper_bounds,
    stepper_value,
};
use crate::{
    Catalog, Child, ComponentDef, Content, Node, PropType, Value, is_loop_variable, js_number_from,
    js_trim, resolve_path, squash, text as show, truthy,
};

/// A hostile document can nest deep; past this depth content is not drawn, as in the renderer
/// (`MAX_DEPTH` of `expand.ts`), so the two show the same content.
const MAX_DEPTH: usize = 200;

#[derive(Clone)]
pub(crate) struct Scope<'a> {
    data: &'a Json,
    vars: Vec<(&'a str, &'a Json)>,
    /// Appended to the template id of every node inside `each`, e.g. `[2][0]`.
    suffix: String,
}

pub(crate) struct Inst<'a> {
    pub kind: &'a str,
    /// The template id from the document, and the instance id (`id[index]` inside `each`).
    pub doc_id: &'a str,
    pub id: String,
    pub def: Option<&'a ComponentDef>,
    node: &'a Node,
    pub children: Vec<InstChild<'a>>,
    /// By slot name, sorted, as the renderer visits them.
    pub slots: Vec<(&'a str, Vec<InstChild<'a>>)>,
    scope: Scope<'a>,
    /// Whether the default slot has something other than empty loops to show (SPEC §5.1 `empty`).
    filled: bool,
}

pub(crate) enum InstChild<'a> {
    Node(Inst<'a>),
    Text(String),
}

impl<'a> InstChild<'a> {
    pub fn node(&self) -> Option<&Inst<'a>> {
        match self {
            InstChild::Node(n) => Some(n),
            InstChild::Text(_) => None,
        }
    }
}

/// The drawn root as an instance, or `None` when it is hidden.
pub(crate) fn expand_root<'a>(
    root: &'a Node,
    catalog: &'a Catalog,
    data: &'a Json,
) -> Option<Inst<'a>> {
    let scope = Scope {
        data,
        vars: Vec::new(),
        suffix: String::new(),
    };
    Expander { catalog }.node_of(root, &scope, 0)
}

struct Expander<'a> {
    catalog: &'a Catalog,
}

impl<'a> Expander<'a> {
    /// The root is read as a child like any other: it may be hidden, or a loop.
    fn node_of(&self, node: &'a Node, scope: &Scope<'a>, depth: usize) -> Option<Inst<'a>> {
        let mut out = Vec::new();
        self.child(node, scope, depth, &mut out);
        out.into_iter().find_map(|c| match c {
            InstChild::Node(n) => Some(n),
            InstChild::Text(_) => None,
        })
    }

    fn list(&self, list: &'a [Child], scope: &Scope<'a>, depth: usize) -> Vec<InstChild<'a>> {
        let mut out = Vec::new();
        if depth > MAX_DEPTH {
            return out;
        }
        for child in list {
            match child {
                Child::Text(t) if !js_trim(t).is_empty() => {
                    push(&mut out, InstChild::Text(t.clone()))
                }
                Child::Text(_) => {}
                Child::Node(n) => self.child(n, scope, depth, &mut out),
            }
        }
        out
    }

    fn child(&self, n: &'a Node, scope: &Scope<'a>, depth: usize, out: &mut Vec<InstChild<'a>>) {
        if n.kind == crate::rules::EACH {
            for c in self.each(n, scope, depth) {
                push(out, c);
            }
        } else if !truthy(resolve(n.props.get("hidden"), scope).as_deref()) {
            push(out, InstChild::Node(self.node(n, scope, depth)));
        }
    }

    fn each(&self, n: &'a Node, scope: &Scope<'a>, depth: usize) -> Vec<InstChild<'a>> {
        // Only a plain binding names data a loop can walk; `not` makes it a boolean.
        let Some(Value::Bind { bind, not: false }) = n.props.get("in") else {
            return Vec::new();
        };
        let Some(Json::Array(items)) = resolve_path(scope.data, &scope.vars, bind) else {
            return Vec::new();
        };
        let Some(Value::String(name)) = n.props.get("as") else {
            return Vec::new();
        };
        if !is_loop_variable(name) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let mut inner = scope.clone();
            inner.vars.push((name, item));
            inner.suffix.push_str(&format!("[{i}]"));
            for c in self.list(&n.children, &inner, depth + 1) {
                push(&mut out, c);
            }
        }
        out
    }

    fn node(&self, n: &'a Node, scope: &Scope<'a>, depth: usize) -> Inst<'a> {
        let mut names: Vec<&'a String> = n.slots.keys().collect();
        names.sort();
        let slots = names
            .into_iter()
            .map(|name| (name.as_str(), self.list(&n.slots[name], scope, depth + 1)))
            .collect();
        let def = self.catalog.components.get(&n.kind);
        let mut children = self.list(&n.children, scope, depth + 1);
        // SPEC §5.1: a text-bearing kind takes its text from content or from the `text` prop; as
        // content, both spellings read alike from here on, and content wins when both are given.
        let texts = def.is_some_and(|d| matches!(d.content, Content::Text | Content::Mixed));
        if texts && children.is_empty() {
            let own = show(resolve(n.props.get("text"), scope).as_deref());
            if !js_trim(&own).is_empty() {
                children.push(InstChild::Text(own));
            }
        }
        let doc_id = n.id.as_deref().unwrap_or("");
        Inst {
            kind: &n.kind,
            doc_id,
            id: if doc_id.is_empty() {
                String::new()
            } else {
                format!("{doc_id}{}", scope.suffix)
            },
            def,
            node: n,
            children,
            slots,
            scope: scope.clone(),
            filled: n.children.iter().any(|c| shows(c, scope)),
        }
    }
}

/// Adjacent text is one DOM text node in the renderer, so it is one run here too.
fn push<'a>(out: &mut Vec<InstChild<'a>>, c: InstChild<'a>) {
    if let (InstChild::Text(t), Some(InstChild::Text(last))) = (&c, out.last_mut()) {
        last.push(' ');
        last.push_str(t);
        return;
    }
    out.push(c);
}

/// SPEC §5.1 `empty`: a static element counts even when hidden, a loop only when its array has
/// items. Table columns are the header, not rows, so they never count.
fn shows(child: &Child, scope: &Scope<'_>) -> bool {
    let Child::Node(n) = child else { return false };
    match n.kind.as_str() {
        "column" => false,
        crate::rules::EACH => matches!(
            resolve(n.props.get("in"), scope).as_deref(),
            Some(Json::Array(items)) if !items.is_empty()
        ),
        _ => true,
    }
}

/// A prop value against data and loop variables (SPEC §2.1). A token is styling, never data.
fn resolve<'a>(raw: Option<&'a Value>, scope: &Scope<'a>) -> Option<Cow<'a, Json>> {
    match raw? {
        Value::String(s) => Some(Cow::Owned(Json::String(s.clone()))),
        Value::Number(n) => serde_json::Number::from_f64(*n).map(|n| Cow::Owned(Json::Number(n))),
        Value::Bool(b) => Some(Cow::Owned(Json::Bool(*b))),
        Value::Bind { bind, not } => {
            let hit = resolve_path(scope.data, &scope.vars, bind);
            if *not {
                Some(Cow::Owned(Json::Bool(!truthy(hit))))
            } else {
                hit.map(Cow::Borrowed)
            }
        }
        Value::Token(_) => None,
    }
}

// ---- Readings of props; the renderer's `expand.ts` restates each one. ----

impl<'a> Inst<'a> {
    pub fn prop(&self, name: &str) -> Option<Cow<'a, Json>> {
        let value = resolve(self.node.props.get(name), &self.scope)?;
        let def = self.def.and_then(|d| d.prop(name));
        match (def, value.as_ref()) {
            (Some(def), Json::Number(n)) if def.kind == PropType::Number => {
                // Validation keeps literals in range, so this only moves bound data, which the
                // renderer brings into the declared range rather than drawing what is forbidden.
                let mut v = n.as_f64()?;
                if def.integer == Some(true) {
                    // JavaScript's `Math.round`: halves go up, also below zero.
                    v = (v + 0.5).floor();
                }
                if let Some(min) = def.min {
                    v = v.max(min);
                }
                if let Some(max) = def.max {
                    v = v.min(max);
                }
                serde_json::Number::from_f64(v).map(|n| Cow::Owned(Json::Number(n)))
            }
            _ => Some(value),
        }
    }

    /// A prop as written, when it is literal text: what an extension's `role` is read from.
    pub fn literal(&self, name: &str) -> Option<&'a str> {
        match self.node.props.get(name)? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn text(&self, name: &str) -> String {
        show(self.prop(name).as_deref())
    }

    pub fn flag(&self, name: &str) -> bool {
        truthy(self.prop(name).as_deref())
    }

    pub fn label(&self) -> String {
        js_trim(&self.text("label")).to_owned()
    }

    pub fn state(&self) -> String {
        self.text("state")
    }

    fn declares_slot(&self, name: &str) -> bool {
        self.def.is_some_and(|d| d.slot(name).is_some())
    }

    pub fn slot(&self, name: &str) -> Option<&[InstChild<'a>]> {
        self.slots
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, list)| list.as_slice())
    }

    /// SPEC §5.1: a declared `empty` slot replaces the items when there are none to show or the
    /// state says so.
    pub fn shows_empty(&self) -> bool {
        self.declares_slot("empty")
            && self.slot("empty").is_some()
            && (self.state() == "empty" || !self.filled)
    }

    /// The content in display order (SPEC §4.2): `header` leads, a declared `empty` stands in for
    /// the default content when it is shown and is left out otherwise, and every other slot
    /// follows.
    pub fn ordered(&self) -> Vec<&InstChild<'a>> {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for (slot, list) in &self.slots {
            if *slot == "empty" && self.declares_slot(slot) {
                continue;
            }
            let side = if *slot == "header" {
                &mut before
            } else {
                &mut after
            };
            side.extend(list.iter());
        }
        let body = if self.shows_empty() {
            self.slot("empty").unwrap_or_default()
        } else {
            &self.children
        };
        before.extend(body.iter());
        before.extend(after);
        before
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Inst<'a>> {
        self.children.iter().filter_map(InstChild::node)
    }

    /// An out-of-range level is 2, the level ARIA assumes for a heading without one.
    pub fn heading_level(&self) -> u8 {
        let level = match self.prop("level").as_deref() {
            Some(Json::Number(n)) => n.as_f64(),
            Some(Json::String(s)) => Some(js_number_from(s)),
            _ => None,
        };
        match level {
            Some(l) if l.fract() == 0.0 && (1.0..=6.0).contains(&l) => l as u8,
            _ => 2,
        }
    }

    /// SPEC §5.1: `field` is a textbox refined by `type` as ARIA requires.
    pub fn field_role(&self) -> &'static str {
        match self.text("type").as_str() {
            "number" => "spinbutton",
            "search" => "searchbox",
            _ => "textbox",
        }
    }

    /// A field is invalid when its state says so or when it shows an error message.
    pub fn field_invalid(&self) -> bool {
        self.state() == "invalid" || !self.text("error").is_empty()
    }

    /// What the browser shows: a number input sanitizes anything that is not a valid float to "".
    pub fn field_value(&self) -> String {
        let value = self.text("value");
        if self.text("type") == "number" && !is_float(&value) {
            String::new()
        } else {
            value
        }
    }

    fn num(&self, name: &str) -> Option<f64> {
        match self.prop(name).as_deref() {
            Some(Json::Number(n)) => controls::numeric(n.as_f64(), None),
            Some(Json::String(s)) => controls::numeric(None, Some(s)),
            _ => None,
        }
    }

    pub fn slider_value(&self) -> f64 {
        let span: Span = slider_span(self.num("min"), self.num("max"), self.num("step"));
        slider_value(self.num("value"), span)
    }

    pub fn stepper_value(&self) -> f64 {
        let bounds: Bounds = stepper_bounds(self.num("min"), self.num("max"), self.num("step"));
        stepper_value(self.num("value"), bounds)
    }

    pub fn date_value(&self) -> String {
        date_text(DateType::of(&self.text("type")), &self.text("value"))
    }

    pub fn color_value(&self) -> String {
        controls::color_value(&self.text("value"))
    }

    pub fn options(&self) -> impl Iterator<Item = &Inst<'a>> {
        self.nodes().filter(|c| c.kind == "option")
    }

    /// A single-select always has one option selected; with no match the browser picks the first.
    pub fn selected_option(&self) -> Option<(&Inst<'a>, String)> {
        let value = self.text("value");
        let hit = self
            .options()
            .find(|o| o.text("value") == value)
            .or_else(|| self.options().next())?;
        let label = hit.label();
        let text = if label.is_empty() {
            content_text(&hit.children)
        } else {
            label
        };
        Some((hit, text))
    }

    /// Index of the selected tab: the one whose id `selected` names, else the first.
    pub fn selected_tab(&self, tabs: &[&Inst<'a>]) -> usize {
        let selected = self.text("selected");
        tabs.iter()
            .position(|t| t.doc_id == selected || t.id == selected)
            .unwrap_or(0)
    }

    pub fn tabs(&self) -> Vec<&Inst<'a>> {
        self.nodes().filter(|c| c.kind == "tab").collect()
    }

    pub fn dialog_open(&self) -> bool {
        self.flag("open")
    }

    pub fn radio_checked(&self, group: Option<&Inst<'_>>) -> bool {
        group.is_some_and(|g| {
            let value = g.text("value");
            !value.is_empty() && self.text("value") == value
        })
    }
}

/// The text a subtree gives an accessible name computed from content (accname 2F), joined with
/// spaces because the renderer puts no inline boxes between named parts.
pub(crate) fn content_text<'a: 'b, 'b>(
    list: impl IntoIterator<Item = &'b InstChild<'a>>,
) -> String {
    let mut parts = Vec::new();
    for c in list {
        let t = match c {
            InstChild::Text(t) => t.clone(),
            InstChild::Node(n) => contribution(n),
        };
        let t = js_trim(&t);
        if !t.is_empty() {
            parts.push(t.to_owned());
        }
    }
    squash(&parts.join(" "))
}

fn contribution(n: &Inst<'_>) -> String {
    if n.def.is_some() {
        if n.kind == "dialog" && !n.dialog_open() {
            return String::new();
        }
        // Embedded controls give their value, not their label (accname 2C exception, 2E).
        if n.kind == "field" {
            let parts = [n.field_value(), n.text("error")];
            return parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
        }
        if n.kind == "select" {
            return n.selected_option().map(|(_, t)| t).unwrap_or_default();
        }
    }
    let own = n.label();
    if !own.is_empty() && !matches!(n.kind, "text" | "stack" | "grid") {
        return own;
    }
    if n.def.is_some() {
        if matches!(n.kind, "checkbox" | "switch" | "image" | "model") {
            return String::new();
        }
        if n.kind == "tabs" {
            let tabs = n.tabs();
            let mut parts: Vec<String> = tabs.iter().map(|t| t.label()).collect();
            parts.push(
                tabs.get(n.selected_tab(&tabs))
                    .map(|sel| content_text(&sel.children))
                    .unwrap_or_default(),
            );
            return js_trim(&parts.join(" ")).to_owned();
        }
    }
    content_text(n.ordered())
}
