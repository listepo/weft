//! Weft → A2UI v0.9 (SPEC §9, "To A2UI"). A document becomes a `createSurface` and an
//! `updateComponents` message of the basic catalog: the tree flattens into a component list
//! linked by ids, bindings become JSON Pointers, and what the basic catalog cannot say is a loss.
//! The stand-ins are the ones of `corpus/README.md`.

use std::collections::HashSet;

use serde_json::{Map, Value as Json, json};
use weft_core::{Child, Document, Node, Value};
use weft_import::{Loss, LossKind, Losses};

use crate::ids::{collect_ids, fresh_id};
use crate::number;
use crate::paths::to_pointer;

pub const CATALOG_ID: &str = "https://a2ui.org/specification/v0_9/catalogs/basic/catalog.json";
const VERSION: &str = "v0.9";

pub struct Exported {
    pub messages: Vec<Json>,
    pub losses: Vec<Loss>,
}

/// A component to write: its basic-catalog name, its fields, and sibling components that must
/// follow it in the parent's child list.
struct Built(&'static str, Json, Vec<String>);

struct Ex {
    comps: Vec<Json>,
    ids: HashSet<String>,
    losses: Losses,
    /// The loop variables around the node being written, outermost first.
    scope: Vec<String>,
    /// Submit buttons that received their form's action.
    submits: usize,
    /// The slots a component took, as `<node path>/<slot>`, so that the others can be reported.
    taken: HashSet<String>,
}

pub fn to_a2ui(doc: &Document) -> Exported {
    let mut ex = Ex {
        comps: vec![Json::Null],
        ids: HashSet::from(["root".to_owned()]),
        losses: Losses::default(),
        scope: Vec::new(),
        submits: 0,
        taken: HashSet::new(),
    };
    collect_ids(&doc.root, &mut ex.ids);
    let surface = doc.root.id.clone().unwrap_or_else(|| "screen".into());
    let path = format!("/screen#{surface}");
    let mut parts = Parts::of(&doc.root, &mut ex, &path);
    let mut fields = json!({ "children": ex.kids(&doc.root.children, &path, None) });
    ex.finish(&mut fields, &mut parts, &path);
    ex.fill(0, "root", "Column", fields);
    ex.comps.retain(|c| !c.is_null());
    let messages = vec![
        json!({ "version": VERSION, "createSurface": { "surfaceId": surface, "catalogId": CATALOG_ID } }),
        json!({ "version": VERSION, "updateComponents": { "surfaceId": surface, "components": ex.comps } }),
    ];
    Exported {
        messages,
        losses: ex.losses.0,
    }
}

/// Whether `url` has a scheme, as the `uri` format of `openUrl` requires.
fn absolute(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    })
}

/// An A2UI `Button` always has an action; this one does nothing but name the press.
fn event(name: &str) -> Json {
    json!({ "event": { "name": name } })
}

impl Ex {
    fn lose(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    fn fresh(&mut self, base: &str) -> String {
        fresh_id(&mut self.ids, base)
    }

    fn fill(&mut self, at: usize, id: &str, component: &str, fields: Json) {
        let mut c = Map::new();
        c.insert("id".into(), id.into());
        c.insert("component".into(), component.into());
        if let Json::Object(f) = fields {
            c.extend(f);
        }
        self.comps[at] = Json::Object(c);
    }

    /// A component of its own, after the ones written so far.
    fn add(&mut self, id: &str, component: &str, fields: Json) -> String {
        self.comps.push(Json::Null);
        self.fill(self.comps.len() - 1, id, component, fields);
        id.to_owned()
    }

    fn text(&mut self, base: &str, text: Json, variant: Option<&str>) -> String {
        let id = self.fresh(base);
        let mut fields = json!({ "text": text });
        if let Some(v) = variant {
            fields["variant"] = v.into();
        }
        self.add(&id, "Text", fields)
    }

    fn column(&mut self, ids: Vec<String>) -> String {
        let id = self.fresh("column");
        self.add(&id, "Column", json!({ "children": ids }))
    }

    /// A single component for a list of them: the only one, or a column holding all.
    fn one(&mut self, mut ids: Vec<String>) -> String {
        match ids.len() {
            1 => ids.remove(0),
            _ => self.column(ids),
        }
    }

    fn dynamic(&mut self, v: &Value, path: &str, boolean: bool) -> Option<Json> {
        match v {
            Value::String(s) => Some(json!(s)),
            Value::Number(n) => Some(number(*n)),
            Value::Bool(b) => Some(json!(b)),
            Value::Token(t) => {
                self.lose(
                    LossKind::Tokens,
                    path,
                    format!("token {t} has no A2UI value"),
                );
                None
            }
            Value::Bind { bind, not } => match to_pointer(bind, &self.scope) {
                Ok(p) if !*not => Some(json!({ "path": p })),
                Ok(p) if boolean => Some(
                    json!({ "call": "not", "args": { "value": { "path": p } }, "returnType": "boolean" }),
                ),
                Ok(_) => {
                    self.lose(
                        LossKind::Bindings,
                        path,
                        "a negated binding has no A2UI form outside a boolean",
                    );
                    None
                }
                Err(why) => {
                    self.lose(LossKind::Bindings, path, why);
                    None
                }
            },
        }
    }

    /// A prop written as a dynamic value, removed from `p`.
    fn take(
        &mut self,
        p: &mut weft_core::Map<Value>,
        name: &str,
        path: &str,
        boolean: bool,
    ) -> Option<Json> {
        let v = p.shift_remove(name)?;
        self.dynamic(&v, path, boolean)
    }

    /// What a text kind shows: its `text` prop, else its text children.
    fn shown(&mut self, n: &Node, p: &mut weft_core::Map<Value>, path: &str) -> Json {
        if let Some(j) = self.take(p, "text", path, false) {
            return j;
        }
        let parts: Vec<&str> = n
            .children
            .iter()
            .filter_map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect();
        json!(parts.join(" "))
    }

    /// What was not written: every prop and event left over is a loss.
    fn leftovers(&mut self, p: &weft_core::Map<Value>, on: &weft_core::Map<String>, path: &str) {
        for (k, v) in p {
            let (kind, why) = match (k.as_str(), v) {
                ("hidden", _) => (
                    LossKind::Hidden,
                    "a bound `hidden` has no A2UI form; the element is always shown",
                ),
                (_, Value::Token(_)) => (LossKind::Tokens, "design tokens have no A2UI form"),
                (_, Value::Bind { .. }) => {
                    (LossKind::Bindings, "this binding has no A2UI property")
                }
                ("columns" | "wrap", _) => (LossKind::Layout, "this layout has no A2UI form"),
                _ => (LossKind::Props, "this prop has no A2UI property"),
            };
            self.lose(kind, path, format!("{k}: {why}"));
        }
        for (event, action) in on {
            self.lose(
                LossKind::Actions,
                path,
                format!("on-{event}=\"{action}\" has no A2UI event"),
            );
        }
    }

    fn ids(&mut self, list: &[Child], path: &str, form: Option<&str>) -> Vec<String> {
        let mut ids = Vec::new();
        for c in list {
            match c {
                Child::Text(t) => ids.push(self.text("text", json!(t), None)),
                Child::Node(n) => ids.extend(self.node(n, path, form)),
            }
        }
        ids
    }

    /// A child list: the template of a lone `<each>`, else the components in order.
    fn kids(&mut self, list: &[Child], path: &str, form: Option<&str>) -> Json {
        if let [Child::Node(n)] = list
            && n.kind == "each"
        {
            return self.template(n, path, form).unwrap_or_else(|| json!([]));
        }
        json!(self.ids(list, path, form))
    }

    fn template(&mut self, e: &Node, path: &str, form: Option<&str>) -> Option<Json> {
        let path = format!("{path}/each#{}", e.id.as_deref().unwrap_or_default());
        let Some(Value::Bind { bind, not: false }) = e.props.get("in") else {
            self.lose(
                LossKind::Repetition,
                &path,
                "the repeated list is not a data path",
            );
            return None;
        };
        let list = match to_pointer(bind, &self.scope) {
            Ok(p) => p,
            Err(why) => {
                self.lose(LossKind::Repetition, &path, why);
                return None;
            }
        };
        let var = match e.props.get("as") {
            Some(Value::String(s)) => s.clone(),
            _ => "item".into(),
        };
        self.scope.push(var);
        let ids = self.ids(&e.children, &path, form);
        self.scope.pop();
        let template = self.one(ids);
        Some(json!({ "componentId": template, "path": list }))
    }

    fn node(&mut self, n: &Node, path: &str, form: Option<&str>) -> Vec<String> {
        if n.kind == "each" {
            let Some(t) = self.template(n, path, form) else {
                return Vec::new();
            };
            let column = self.fresh("column");
            return vec![self.add(&column, "Column", json!({ "children": t }))];
        }
        let id = match &n.id {
            Some(id) if id != "root" => id.clone(),
            _ => self.fresh(&n.kind),
        };
        let path = format!("{path}/{}#{id}", n.kind);
        if let Some(Value::Bool(true)) = n.props.get("hidden") {
            self.lose(LossKind::Hidden, &path, "a hidden element is not written");
            return Vec::new();
        }
        let mut parts = Parts::of(n, self, &path);
        let at = self.comps.len();
        self.comps.push(Json::Null);
        match self.build(n, &id, &mut parts, &path, form) {
            Err(ids) => {
                self.leftovers(&parts.p, &parts.on, &path);
                ids
            }
            Ok(Built(component, mut fields, after)) => {
                for name in n.slots.keys() {
                    if !self.taken.contains(&format!("{path}/{name}")) {
                        self.lose(
                            LossKind::Slots,
                            &path,
                            format!("the {name} slot has no place in A2UI; it is left out"),
                        );
                    }
                }
                self.finish(&mut fields, &mut parts, &path);
                self.fill(at, &id, component, fields);
                std::iter::once(id).chain(after).collect()
            }
        }
    }

    /// The accessible name no property carried, and the losses of what was left.
    fn finish(&mut self, fields: &mut Json, parts: &mut Parts, path: &str) {
        if let Some(l) = parts.label.take() {
            fields["accessibility"] = json!({ "label": l });
        }
        self.leftovers(&parts.p, &parts.on, path);
    }
}

/// The props, events and label of a node, taken one by one as its component uses them.
struct Parts {
    p: Props,
    on: weft_core::Map<String>,
    label: Option<Json>,
}

type Props = weft_core::Map<Value>;

impl Parts {
    fn of(n: &Node, ex: &mut Ex, path: &str) -> Parts {
        let mut p = n.props.clone();
        // `hidden: false` shows the element, as every element is shown.
        if matches!(p.get("hidden"), Some(Value::Bool(_))) {
            p.shift_remove("hidden");
        }
        let label = p
            .shift_remove("label")
            .and_then(|l| ex.dynamic(&l, path, false));
        Parts {
            p,
            on: n.on.clone(),
            label,
        }
    }
}

impl Ex {
    fn slot(&mut self, n: &Node, name: &str, path: &str, form: Option<&str>) -> Vec<String> {
        self.taken.insert(format!("{path}/{name}"));
        n.slots
            .get(name)
            .map(|l| self.ids(l, path, form))
            .unwrap_or_default()
    }

    /// A string-valued property: a number is written as the text it shows.
    fn stringy(&mut self, v: Option<Value>, path: &str) -> Option<Json> {
        match v {
            Some(Value::Number(n)) => Some(json!(weft_core::js_number(n))),
            Some(v) => self.dynamic(&v, path, false),
            None => None,
        }
    }

    /// `checks` that fail while the control is disabled: a button is enabled when its checks pass.
    fn checks(&mut self, v: Value, path: &str) -> Option<Json> {
        let condition = match v {
            Value::Bool(true) => json!(false),
            Value::Bool(false) => return None,
            Value::Bind { bind, not } => match to_pointer(&bind, &self.scope) {
                Ok(p) if not => json!({ "call": "required", "args": { "value": { "path": p } } }),
                Ok(p) => json!({ "call": "not", "args": { "value": { "path": p } } }),
                Err(why) => {
                    self.lose(LossKind::Bindings, path, why);
                    return None;
                }
            },
            _ => return None,
        };
        Some(json!([{ "condition": condition, "message": "Not available" }]))
    }

    fn check(call: &str, message: &str, value: Option<&Json>) -> Option<Json> {
        Some(
            json!({ "condition": { "call": call, "args": { "value": value? } }, "message": message }),
        )
    }

    fn button(
        &mut self,
        n: &Node,
        id: &str,
        parts: &mut Parts,
        path: &str,
        form: Option<&str>,
    ) -> Built {
        let p = &mut parts.p;
        let text = self.shown(n, p, path);
        let child = self.text(&format!("{id}-label"), text, None);
        let button = n.kind == "button";
        let mut action = parts.on.shift_remove("press").map(|a| event(&a));
        let submit = matches!(p.shift_remove("submit"), Some(Value::Bool(true)));
        if let (None, true, Some(f)) = (&action, submit, form) {
            action = Some(event(f));
            self.submits += 1;
        }
        if !button && action.is_none() {
            match p.shift_remove("href") {
                Some(Value::String(url)) if absolute(&url) => {
                    action = Some(
                        json!({ "functionCall": { "call": "openUrl", "args": { "url": url }, "returnType": "void" } }),
                    );
                }
                Some(Value::String(_)) => self.lose(
                    LossKind::Props,
                    path,
                    "openUrl takes an absolute URI; a relative href is left out",
                ),
                Some(_) => self.lose(
                    LossKind::Bindings,
                    path,
                    "openUrl takes a literal URL; a bound href is left out",
                ),
                None => {}
            }
        }
        if action.is_none() {
            self.lose(
                LossKind::Actions,
                path,
                "an A2UI button needs an action; the id names the press",
            );
        }
        let variant = match p.get("variant") {
            _ if !button => "borderless",
            Some(Value::String(v)) if v == "primary" => "primary",
            _ => "default",
        };
        if button && matches!(p.get("variant"), Some(Value::String(v)) if v != "danger") {
            p.shift_remove("variant");
        }
        let mut fields = json!({ "child": child, "variant": variant, "action": action.unwrap_or_else(|| event(id)) });
        if let Some(checks) = p
            .shift_remove("disabled")
            .and_then(|d| self.checks(d, path))
        {
            fields["checks"] = checks;
        }
        Built("Button", fields, Vec::new())
    }

    fn choice(&mut self, n: &Node, parts: &mut Parts, path: &str) -> Built {
        let mut options = Vec::new();
        for c in &n.children {
            let Child::Node(o) = c else { continue };
            if !matches!(o.kind.as_str(), "radio" | "option" | "segment") {
                self.lose(
                    LossKind::Repetition,
                    path,
                    "options that repeat over data have no A2UI form; they are left out",
                );
                continue;
            }
            let mut op = o.props.clone();
            let label = self.shown(o, &mut op, path);
            let value = match op.shift_remove("value") {
                Some(Value::String(s)) => s,
                _ => String::new(),
            };
            let opath = format!("{path}/{}#{}", o.kind, o.id.as_deref().unwrap_or_default());
            self.leftovers(&op, &o.on, &opath);
            options.push(json!({ "label": label, "value": value }));
        }
        let value = match self.take(&mut parts.p, "value", path, false) {
            Some(Json::String(s)) => json!([s]),
            Some(bound) => bound,
            None => json!([]),
        };
        let mut fields =
            json!({ "variant": "mutuallyExclusive", "options": options, "value": value });
        if let Some(l) = parts.label.take() {
            fields["label"] = l;
        }
        match n.kind.as_str() {
            "segmented-control" => fields["displayStyle"] = "chips".into(),
            "select" | "combobox" => fields["filterable"] = true.into(),
            _ => {}
        }
        Built("ChoicePicker", fields, Vec::new())
    }

    fn table(&mut self, n: &Node, id: &str, path: &str, form: Option<&str>) -> Built {
        let (columns, rows): (Vec<&Child>, Vec<&Child>) = n
            .children
            .iter()
            .partition(|c| matches!(c, Child::Node(c) if c.kind == "column"));
        let mut header = Vec::new();
        for c in columns.iter().filter_map(|c| c.as_node()) {
            let mut cp = c.props.clone();
            let (cid, cpath) = (
                c.id.clone().unwrap_or_default(),
                format!("{path}/column#{}", c.id.as_deref().unwrap_or_default()),
            );
            let text = self.shown(c, &mut cp, &cpath);
            let mut con = c.on.clone();
            header.push(match con.shift_remove("press") {
                Some(a) => {
                    let label = self.text(&format!("{cid}-label"), text, None);
                    self.add(
                        &cid,
                        "Button",
                        json!({ "child": label, "variant": "borderless", "action": event(&a) }),
                    )
                }
                None => self.text(&cid, text, None),
            });
            self.leftovers(&cp, &con, &cpath);
        }
        let mut kids = Vec::new();
        if !header.is_empty() {
            let row = self.fresh(&format!("{id}-header"));
            kids.push(self.add(&row, "Row", json!({ "children": header })));
        }
        let rows: Vec<Child> = rows.into_iter().cloned().collect();
        let list = self.fresh(&format!("{id}-rows"));
        let children = self.kids(&rows, path, form);
        kids.push(self.add(&list, "List", json!({ "children": children })));
        kids.extend(self.slot(n, "empty", path, form));
        Built("Column", json!({ "children": kids }), Vec::new())
    }

    fn build(
        &mut self,
        n: &Node,
        id: &str,
        parts: &mut Parts,
        path: &str,
        form: Option<&str>,
    ) -> Result<Built, Vec<String>> {
        let kind = n.kind.as_str();
        let p = &mut parts.p;
        Ok(match kind {
            "stack" | "grid" => {
                let row =
                    matches!(p.shift_remove("direction"), Some(Value::String(d)) if d == "row");
                let mut f = json!({ "children": self.kids(&n.children, path, form) });
                if let Some(Value::String(a)) = p.shift_remove("align") {
                    f["align"] = a.into();
                }
                Built(if row { "Row" } else { "Column" }, f, Vec::new())
            }
            "row" => Built(
                "Row",
                json!({ "children": self.kids(&n.children, path, form) }),
                Vec::new(),
            ),
            "section" | "alert" => {
                let mut ids = self.slot(n, "header", path, form);
                if let Some(t) = self.take(p, "text", path, false) {
                    ids.push(self.text(&format!("{id}-text"), t, None));
                }
                ids.extend(self.ids(&n.children, path, form));
                Built("Card", json!({ "child": self.one(ids) }), Vec::new())
            }
            "form" => {
                let action = parts.on.shift_remove("submit");
                let before = self.submits;
                let mut ids = self.ids(&n.children, path, action.as_deref());
                ids.extend(self.slot(n, "footer", path, action.as_deref()));
                if action.is_some() && self.submits == before {
                    self.lose(
                        LossKind::Actions,
                        path,
                        "no submit button carries the form's action",
                    );
                }
                Built("Column", json!({ "children": ids }), Vec::new())
            }
            "heading" => {
                let level = match p.shift_remove("level") {
                    Some(Value::Number(l)) => l.clamp(1.0, 6.0) as u8,
                    _ => 1,
                };
                if level > 5 {
                    self.lose(
                        LossKind::Props,
                        path,
                        "A2UI has headings h1 to h5; h5 stands in for level 6",
                    );
                }
                let text = self.shown(n, p, path);
                Built(
                    "Text",
                    json!({ "text": text, "variant": format!("h{}", level.min(5)) }),
                    Vec::new(),
                )
            }
            "text" => {
                let muted = matches!(p.get("tone"), Some(Value::String(t)) if t == "muted");
                if muted || matches!(p.get("tone"), Some(Value::String(t)) if t == "default") {
                    p.shift_remove("tone");
                }
                let mut f = json!({ "text": self.shown(n, p, path) });
                if muted {
                    f["variant"] = "caption".into();
                }
                Built("Text", f, Vec::new())
            }
            "image" => {
                let url = self
                    .take(p, "src", path, false)
                    .unwrap_or_else(|| json!(""));
                let mut f = json!({ "url": url });
                if let Some(l) = parts.label.take() {
                    f["description"] = l;
                }
                Built("Image", f, Vec::new())
            }
            "button" | "link" | "menu-item" => self.button(n, id, parts, path, form),
            "field" | "stepper" | "color-picker" => {
                let ty = match p.shift_remove("type") {
                    Some(Value::String(t)) => t,
                    _ => "text".to_owned(),
                };
                let variant = match (kind, ty.as_str()) {
                    ("stepper", _) | (_, "number") => "number",
                    (_, "password") => "obscured",
                    (_, "multiline") => "longText",
                    _ => "shortText",
                };
                if kind == "field"
                    && !["text", "email", "password", "number", "multiline"].contains(&ty.as_str())
                {
                    self.lose(
                        LossKind::Props,
                        path,
                        format!("type {ty} has no A2UI form; a plain text field stands in"),
                    );
                }
                if kind != "field" {
                    self.lose(
                        LossKind::Kinds,
                        path,
                        format!("{kind} has no A2UI component; a text field stands in"),
                    );
                }
                let value = p.shift_remove("value");
                let value = self.stringy(value, path);
                let mut f = json!({ "label": parts.label.take().unwrap_or_else(|| json!("")), "variant": variant });
                let mut checks = Vec::new();
                if matches!(p.get("required"), Some(Value::Bool(true))) {
                    p.shift_remove("required");
                    match Self::check("required", "Required", value.as_ref()) {
                        Some(c) => checks.push(c),
                        None => self.lose(
                            LossKind::Props,
                            path,
                            "required needs a value to check; it is left out",
                        ),
                    }
                }
                // A2UI has no email variant of a text field: its `email` check is the closest.
                if ty == "email" {
                    match Self::check("email", "Not an email address", value.as_ref()) {
                        Some(c) => checks.push(c),
                        None => self.lose(
                            LossKind::Props,
                            path,
                            "type email needs a value to check; it is left out",
                        ),
                    }
                }
                if !checks.is_empty() {
                    f["checks"] = checks.into();
                }
                if let Some(v) = value {
                    f["value"] = v;
                }
                Built("TextField", f, Vec::new())
            }
            "checkbox" | "switch" => {
                if kind == "switch" {
                    self.lose(
                        LossKind::Kinds,
                        path,
                        "switch has no A2UI component; a checkbox stands in",
                    );
                }
                let value = self
                    .take(p, "checked", path, true)
                    .unwrap_or_else(|| json!(false));
                Built(
                    "CheckBox",
                    json!({ "label": parts.label.take().unwrap_or_else(|| json!("")), "value": value }),
                    Vec::new(),
                )
            }
            "radio-group" | "segmented-control" | "select" | "combobox" => {
                self.choice(n, parts, path)
            }
            "slider" => {
                let mut bound = |name: &str, default: f64| match p.shift_remove(name) {
                    Some(Value::Number(v)) => number(v),
                    _ => number(default),
                };
                let (min, max) = (bound("min", 0.0), bound("max", 100.0));
                let value = self
                    .take(p, "value", path, false)
                    .unwrap_or_else(|| min.clone());
                let mut f = json!({ "min": min, "max": max, "value": value });
                if let Some(l) = parts.label.take() {
                    f["label"] = l;
                }
                Built("Slider", f, Vec::new())
            }
            "date-picker" => {
                let ty = match p.shift_remove("type") {
                    Some(Value::String(t)) => t,
                    _ => "date".into(),
                };
                let value = p.shift_remove("value");
                let value = self.stringy(value, path).unwrap_or_else(|| json!(""));
                let mut f = json!({ "value": value, "enableDate": ty != "time", "enableTime": ty != "date" });
                for bound in ["min", "max"] {
                    if let Some(v) = self.take(p, bound, path, false) {
                        f[bound] = v;
                    }
                }
                if let Some(l) = parts.label.take() {
                    f["label"] = l;
                }
                Built("DateTimeInput", f, Vec::new())
            }
            "list" | "menu" => {
                let after = self.slot(n, "empty", path, form);
                Built(
                    "List",
                    json!({ "children": self.kids(&n.children, path, form) }),
                    after,
                )
            }
            "table" => self.table(n, id, path, form),
            "tabs" => {
                let mut tabs = Vec::new();
                for t in n
                    .children
                    .iter()
                    .filter_map(Child::as_node)
                    .filter(|t| t.kind == "tab")
                {
                    let tpath = format!("{path}/tab#{}", t.id.as_deref().unwrap_or_default());
                    let mut tp = Parts::of(t, self, &tpath);
                    let title = tp.label.take().unwrap_or_else(|| json!(""));
                    let ids = self.ids(&t.children, &tpath, form);
                    tabs.push(json!({ "title": title, "child": self.one(ids) }));
                    self.leftovers(&tp.p, &tp.on, &tpath);
                }
                if tabs.is_empty() {
                    self.lose(LossKind::Structure, path, "A2UI tabs need at least one tab");
                    return Err(Vec::new());
                }
                Built("Tabs", json!({ "tabs": tabs }), Vec::new())
            }
            "dialog" => {
                let mut ids = self.ids(&n.children, path, form);
                ids.extend(self.slot(n, "actions", path, form));
                let content = self.one(ids);
                self.lose(
                    LossKind::Structure,
                    path,
                    "A2UI opens a Modal from a trigger; a generated button opens this one",
                );
                let title = parts.label.clone().unwrap_or_else(|| json!("Open"));
                let text = self.text(&format!("{id}-trigger-label"), title, None);
                let trigger_id = self.fresh(&format!("{id}-trigger"));
                let action = event(&format!("{id}.open"));
                let trigger = self.add(
                    &trigger_id,
                    "Button",
                    json!({ "child": text, "action": action }),
                );
                Built(
                    "Modal",
                    json!({ "trigger": trigger, "content": content }),
                    Vec::new(),
                )
            }
            "item" | "cell" => {
                let mut ids = self
                    .take(p, "text", path, false)
                    .map(|t| self.text(&format!("{id}-text"), t, None))
                    .into_iter()
                    .collect::<Vec<_>>();
                ids.extend(self.ids(&n.children, path, form));
                return Err(vec![self.one(ids)]);
            }
            _ => {
                self.lose(
                    LossKind::Kinds,
                    path,
                    format!("{kind} has no A2UI component; its content is kept"),
                );
                let mut ids = self.ids(&n.children, path, form);
                ids.extend(n.slots.values().flat_map(|l| self.ids(l, path, form)));
                return Err(ids);
            }
        })
    }
}
