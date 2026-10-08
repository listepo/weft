//! A2UI v0.9 → Weft (SPEC §9, "From A2UI"). The messages of one surface become the role tree
//! `weft-import` builds documents from, so ids, required props, content rules and the loss table
//! work as for every other importer. The input is only read: nothing in it is run, and its size,
//! depth and component count are bounded (`W602`).

use std::collections::HashMap;

use serde_json::Value as Json;
use weft_core::{Catalog, Code, Diagnostic, Value, is_action, is_id, parse_json};
use weft_import::{
    BuildOptions, ImportResult, LossKind, MAX_DEPTH, MAX_NODES, Note, Sem, build_document,
    empty_result, limit_reached,
};

use crate::paths::from_pointer;
use crate::{sem, too_long};

/// A value as A2UI states it: a literal, a path into the data model, or nothing Weft can read.
enum Dyn {
    Lit(String),
    Bind(String),
    None,
}

struct Im {
    comps: HashMap<String, Json>,
    /// The loop variables of the templates around the component being read.
    scope: Vec<String>,
    /// Losses of the component being read, handed to the element it becomes.
    notes: Vec<Note>,
    nodes: usize,
    truncated: bool,
}

/// Messages from a JSON array, one object, or JSON Lines (the form A2UI streams in).
fn messages(text: &str) -> Option<Vec<Json>> {
    match parse_json(text) {
        Ok(Json::Array(list)) => Some(list),
        Ok(object @ Json::Object(_)) => Some(vec![object]),
        _ => text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| parse_json(l).ok())
            .collect(),
    }
}

pub fn from_a2ui(text: &str, catalog: &Catalog) -> ImportResult {
    if let Some(result) = too_long(text) {
        return result;
    }
    let mut diagnostics = Vec::new();
    let Some(messages) = messages(text) else {
        let (what, expected) = (
            "The input is not JSON.",
            "A2UI v0.9 messages: a JSON array, an object or JSON Lines",
        );
        diagnostics.push(Diagnostic::new(Code::W601, "#", what, expected));
        return empty_result(diagnostics);
    };
    let mut im = Im {
        comps: HashMap::new(),
        scope: Vec::new(),
        notes: Vec::new(),
        nodes: 0,
        truncated: false,
    };
    let (mut surface, mut data) = (None::<String>, 0);
    for m in &messages {
        let id = ["createSurface", "updateComponents", "updateDataModel"]
            .iter()
            .find_map(|k| m[k]["surfaceId"].as_str());
        match (&surface, id) {
            (None, Some(id)) => surface = Some(id.to_owned()),
            (Some(first), Some(id)) if first != id => {
                im.note(
                    LossKind::Structure,
                    format!("surface {id} is left out: only the first surface is imported"),
                );
                continue;
            }
            _ => {}
        }
        if m["createSurface"]["theme"].is_object() {
            im.note(LossKind::Tokens, "the surface theme has no Weft form");
        }
        if m["updateDataModel"].is_object() {
            data += 1;
        }
        for c in m["updateComponents"]["components"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(id) = c["id"].as_str() {
                im.comps.insert(id.to_owned(), c.clone());
            }
        }
    }
    if data > 0 {
        im.note(
            LossKind::Values,
            format!("the data of {data} updateDataModel message(s) is not part of a document"),
        );
    }
    if !im.comps.contains_key("root") {
        let what = "The surface has no component with the id `root`.";
        diagnostics.push(Diagnostic::new(
            Code::W601,
            "#",
            what,
            "an `updateComponents` message that defines `root`",
        ));
        return empty_result(diagnostics);
    }
    let mut top = sem("screen", "");
    top.id = surface.filter(|s| is_id(s));
    let mut children = im.conv("root", 0);
    // A plain column at the root is the screen itself, as the exporter writes it.
    if let [only] = children.as_slice()
        && im.comps["root"]["component"] == "Column"
        && only.kind.as_deref() == Some("stack")
        && only.props.is_empty()
        && only.values.is_empty()
    {
        top.name = only.name.clone();
        top.notes.extend(only.notes.clone());
        children = only.children.clone();
    }
    top.children = children;
    top.notes.append(&mut im.notes);
    if im.truncated {
        limit_reached(
            &mut diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    let reserved = im.comps.keys().filter(|k| is_id(k)).cloned().collect();
    build_document(
        &[top],
        BuildOptions {
            catalog,
            reserved,
            diagnostics,
        },
    )
    .result
}

impl Im {
    fn note(&mut self, kind: LossKind, note: impl Into<String>) {
        self.notes.push(Note {
            kind,
            note: note.into(),
        });
    }

    fn dynamic(&mut self, v: &Json) -> Dyn {
        match v {
            Json::String(s) => Dyn::Lit(s.clone()),
            Json::Number(n) => Dyn::Lit(n.to_string()),
            Json::Bool(b) => Dyn::Lit(b.to_string()),
            Json::Object(o) if o.contains_key("path") => {
                match o["path"]
                    .as_str()
                    .and_then(|p| from_pointer(p, &self.scope))
                {
                    Some(bind) => Dyn::Bind(bind),
                    None => {
                        self.note(
                            LossKind::Bindings,
                            "a data path that is not a Weft binding is left out",
                        );
                        Dyn::None
                    }
                }
            }
            Json::Null => Dyn::None,
            _ => {
                self.note(
                    LossKind::Values,
                    "a function call has no Weft form; its value is left out",
                );
                Dyn::None
            }
        }
    }

    /// The text of an element that shows text.
    fn show(&mut self, s: &mut Sem, v: &Json) {
        match self.dynamic(v) {
            Dyn::Lit(t) => s.name = t,
            Dyn::Bind(b) => {
                s.values.insert(
                    "text".into(),
                    Value::Bind {
                        bind: b,
                        not: false,
                    },
                );
            }
            Dyn::None => {}
        }
    }

    /// A caption or a value of a prop, as a literal or a binding.
    fn put(&mut self, s: &mut Sem, prop: &str, v: &Json) {
        match self.dynamic(v) {
            Dyn::Lit(t) => {
                s.props.insert(prop.into(), t);
            }
            Dyn::Bind(b) => {
                s.values.insert(
                    prop.into(),
                    Value::Bind {
                        bind: b,
                        not: false,
                    },
                );
            }
            Dyn::None => {}
        }
    }

    fn label(&mut self, s: &mut Sem, v: &Json) {
        match self.dynamic(v) {
            Dyn::Lit(t) => s.name = t,
            Dyn::Bind(b) => {
                s.values.insert(
                    "label".into(),
                    Value::Bind {
                        bind: b,
                        not: false,
                    },
                );
            }
            Dyn::None => {}
        }
    }

    /// The text of a `Text` component, which is what a button, a tab or an option shows.
    fn text_of(&mut self, s: &mut Sem, id: Option<&str>) {
        let comp = id
            .and_then(|i| self.comps.get(i))
            .cloned()
            .unwrap_or_default();
        if comp["component"] == "Text" {
            self.show(s, &comp["text"]);
        } else {
            self.note(
                LossKind::Structure,
                "content other than text is left out of a button",
            );
        }
    }

    fn action(&mut self, s: &mut Sem, action: &Json) {
        if let Some(name) = action["event"]["name"].as_str() {
            if is_action(name) {
                s.on.insert("press".into(), name.to_owned());
            } else {
                self.note(
                    LossKind::Actions,
                    format!("event {name} is not a Weft action name; it is left out"),
                );
            }
            if action["event"]["context"].is_object() {
                self.note(
                    LossKind::Actions,
                    "the context of an event has no Weft form",
                );
            }
        } else if action["functionCall"]["call"] == "openUrl" {
            if let Some(url) = action["functionCall"]["args"]["url"].as_str() {
                s.props.insert("href".into(), url.to_owned());
            }
        } else if action.is_object() {
            self.note(LossKind::Actions, "a local function call has no Weft form");
        }
    }

    /// What `checks` say: a button is disabled while its first check fails, a field is required
    /// when one check is `required`; the rest has no Weft form.
    fn checks(&mut self, s: &mut Sem, checks: &Json) {
        let mut used = false;
        for check in checks.as_array().into_iter().flatten() {
            let cond = &check["condition"];
            let (call, value) = (cond["call"].as_str(), &cond["args"]["value"]["path"]);
            let bind = value.as_str().and_then(|p| from_pointer(p, &self.scope));
            match (call, bind, s.kind.as_deref()) {
                (Some("required"), _, Some("field")) => {
                    s.props.insert("required".into(), "true".into());
                }
                (Some("email"), _, Some("field")) => {
                    s.props.insert("type".into(), "email".into());
                }
                (Some(call @ ("required" | "not")), Some(bind), Some("button" | "link"))
                    if !used =>
                {
                    used = true;
                    s.values.insert(
                        "disabled".into(),
                        Value::Bind {
                            bind,
                            not: call == "required",
                        },
                    );
                }
                (None, _, Some("button" | "link")) if *cond == Json::Bool(false) && !used => {
                    used = true;
                    s.values.insert("disabled".into(), Value::Bool(true));
                }
                _ => self.note(LossKind::Props, "a check has no Weft form; it is left out"),
            }
        }
    }

    fn children(&mut self, list: &Json, depth: usize, wrap: Option<&str>) -> Vec<Sem> {
        let wrapped = |sems: Vec<Sem>| match wrap {
            Some(kind) => {
                let mut w = sem(kind, "");
                w.children = sems;
                vec![w]
            }
            None => sems,
        };
        let mut out = Vec::new();
        if let Json::Array(ids) = list {
            for id in ids.iter().filter_map(Json::as_str) {
                let group = self.conv(id, depth + 1);
                out.extend(wrapped(group));
            }
        } else if let (Some(template), Some(path)) =
            (list["componentId"].as_str(), list["path"].as_str())
        {
            let var = if self.scope.is_empty() {
                "item".to_owned()
            } else {
                format!("item{}", self.scope.len() + 1)
            };
            let mut each = sem("each", "");
            match from_pointer(path, &self.scope) {
                Some(bind) => {
                    each.values
                        .insert("in".into(), Value::Bind { bind, not: false });
                }
                None => self.note(
                    LossKind::Repetition,
                    "the repeated list is not a Weft data path",
                ),
            }
            each.props.insert("as".into(), var.clone());
            self.scope.push(var);
            let body = self.conv(template, depth + 1);
            self.scope.pop();
            each.children = wrapped(body);
            out.push(each);
        }
        out
    }

    fn conv(&mut self, id: &str, depth: usize) -> Vec<Sem> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > MAX_DEPTH {
            self.truncated = true;
            return Vec::new();
        }
        let Some(c) = self.comps.get(id).cloned() else {
            self.note(
                LossKind::Structure,
                format!("component {id} is used but not defined"),
            );
            return Vec::new();
        };
        let mark = self.notes.len();
        let mut out = self.component(&c, depth);
        // The notes of a component belong to the element it became; a dropped one passes them up.
        if let Some(first) = out.first_mut() {
            first.notes.extend(self.notes.drain(mark..));
            if first.id.is_none() && is_id(id) {
                first.id = Some(id.to_owned());
            }
        }
        out
    }

    fn component(&mut self, c: &Json, depth: usize) -> Vec<Sem> {
        let name = c["component"].as_str().unwrap_or_default();
        if !c["weight"].is_null() {
            self.note(LossKind::Layout, "weight has no Weft form");
        }
        let described = c["accessibility"]["description"].is_null();
        if !described {
            self.note(
                LossKind::Props,
                "an accessibility description has no Weft form",
            );
        }
        let mut s = match name {
            "Column" | "Row" => {
                let mut s = sem("stack", "");
                if name == "Row" {
                    s.props.insert("direction".into(), "row".into());
                }
                if let Some(a) = c["align"]
                    .as_str()
                    .filter(|a| ["start", "center", "end", "stretch"].contains(a))
                {
                    s.props.insert("align".into(), a.to_owned());
                }
                if c["justify"].as_str().is_some_and(|j| j != "start") {
                    self.note(LossKind::Layout, "justify has no Weft form");
                }
                s.children = self.children(&c["children"], depth, None);
                s
            }
            "List" => {
                let mut s = sem("list", "");
                if c["direction"] == "horizontal" {
                    self.note(LossKind::Layout, "a horizontal list has no Weft form");
                }
                s.children = self.children(&c["children"], depth, Some("item"));
                s
            }
            "Card" => {
                let labelled = c["accessibility"]["label"].is_string();
                if !labelled {
                    self.note(
                        LossKind::Kinds,
                        "a Card has no Weft kind; a stack stands in",
                    );
                }
                let mut s = sem(if labelled { "section" } else { "stack" }, "");
                s.children = c["child"]
                    .as_str()
                    .map(|i| self.conv(i, depth + 1))
                    .unwrap_or_default();
                s
            }
            "Text" => {
                let level = c["variant"].as_str().and_then(|v| v.strip_prefix('h'));
                let mut s = sem(if level.is_some() { "heading" } else { "text" }, "");
                if let Some(level) = level {
                    s.props.insert("level".into(), level.to_owned());
                } else if c["variant"] == "caption" {
                    s.props.insert("tone".into(), "muted".into());
                }
                self.show(&mut s, &c["text"]);
                s
            }
            "Image" => {
                let mut s = sem("image", "");
                self.label(&mut s, &c["description"]);
                self.put(&mut s, "src", &c["url"]);
                if !c["fit"].is_null() || !c["variant"].is_null() {
                    self.note(LossKind::Props, "fit and variant have no Weft form");
                }
                s
            }
            "Button" => {
                let borderless = c["variant"] == "borderless";
                let mut s = sem(if borderless { "link" } else { "button" }, "");
                if c["variant"] == "primary" {
                    s.props.insert("variant".into(), "primary".into());
                }
                self.text_of(&mut s, c["child"].as_str());
                self.action(&mut s, &c["action"]);
                self.checks(&mut s, &c["checks"]);
                s
            }
            "TextField" => {
                let mut s = sem("field", "");
                self.label(&mut s, &c["label"]);
                let ty = match c["variant"].as_str() {
                    Some("obscured") => Some("password"),
                    Some("longText") => Some("multiline"),
                    Some("number") => Some("number"),
                    _ => None,
                };
                if let Some(ty) = ty {
                    s.props.insert("type".into(), ty.into());
                }
                self.put(&mut s, "value", &c["value"]);
                self.checks(&mut s, &c["checks"]);
                if !c["validationRegexp"].is_null() {
                    self.note(LossKind::Props, "validationRegexp has no Weft form");
                }
                s
            }
            "CheckBox" => {
                let mut s = sem("checkbox", "");
                self.label(&mut s, &c["label"]);
                self.put(&mut s, "checked", &c["value"]);
                self.checks(&mut s, &c["checks"]);
                s
            }
            "ChoicePicker" => self.choice(c),
            "Slider" => {
                let mut s = sem("slider", "");
                self.label(&mut s, &c["label"]);
                for bound in ["min", "max"] {
                    self.put(&mut s, bound, &c[bound]);
                }
                self.put(&mut s, "value", &c["value"]);
                self.checks(&mut s, &c["checks"]);
                s
            }
            "DateTimeInput" => {
                let mut s = sem("date-picker", "");
                self.label(&mut s, &c["label"]);
                match (c["enableDate"] == true, c["enableTime"] == true) {
                    (true, true) => s.props.insert("type".into(), "datetime".into()),
                    (false, true) => s.props.insert("type".into(), "time".into()),
                    _ => None,
                };
                for prop in ["value", "min", "max"] {
                    self.put(&mut s, prop, &c[prop]);
                }
                self.checks(&mut s, &c["checks"]);
                s
            }
            "Tabs" => {
                let mut s = sem("tabs", "");
                for t in c["tabs"].as_array().into_iter().flatten() {
                    let mut tab = sem("tab", "");
                    self.label(&mut tab, &t["title"]);
                    tab.children = t["child"]
                        .as_str()
                        .map(|i| self.conv(i, depth + 1))
                        .unwrap_or_default();
                    s.children.push(tab);
                }
                s
            }
            "Modal" => {
                // The trigger shows in place and its content is a dialog that is closed until
                // opened. The button the exporter generates (event `<id>.open`) is that opening
                // and not content, so it is not read back.
                let modal = c["id"].as_str().unwrap_or_default();
                let trigger = c["trigger"].as_str().unwrap_or_default();
                let generated = self.comps.get(trigger).is_some_and(|t| {
                    t["action"]["event"]["name"]
                        .as_str()
                        .is_some_and(|n| n == format!("{modal}.open"))
                });
                let mut out = if generated {
                    Vec::new()
                } else {
                    self.conv(trigger, depth + 1)
                };
                let mut dialog = sem("dialog", "");
                dialog.props.insert("modal".into(), "true".into());
                dialog.id = generated.then(|| modal.to_owned());
                dialog.children = c["content"]
                    .as_str()
                    .map(|i| self.conv(i, depth + 1))
                    .unwrap_or_default();
                self.label_from_accessibility(c, Some(&mut dialog));
                out.push(dialog);
                return out;
            }
            other => {
                self.note(
                    LossKind::Kinds,
                    format!("the A2UI component {other} has no Weft kind; it is left out"),
                );
                return Vec::new();
            }
        };
        self.label_from_accessibility(c, Some(&mut s));
        vec![s]
    }

    /// The accessible name A2UI keeps apart from the component's own caption.
    fn label_from_accessibility(&mut self, c: &Json, s: Option<&mut Sem>) {
        if let (Some(s), Some(label)) = (s, c["accessibility"]["label"].as_str())
            && s.name.is_empty()
            && !s.values.contains_key("label")
        {
            s.name = label.to_owned();
        }
    }

    fn choice(&mut self, c: &Json) -> Sem {
        let (kind, option) = match (c["filterable"] == true, c["displayStyle"] == "chips") {
            (true, _) => ("select", "option"),
            (_, true) => ("segmented-control", "segment"),
            _ => ("radio-group", "radio"),
        };
        let mut s = sem(kind, "");
        if c["variant"] == "multipleSelection" {
            self.note(
                LossKind::Props,
                "multiple selection is read as a single choice",
            );
        }
        self.label(&mut s, &c["label"]);
        for o in c["options"].as_array().into_iter().flatten() {
            let mut opt = sem(option, "");
            self.show(&mut opt, &o["label"]);
            opt.props.insert(
                "value".into(),
                o["value"].as_str().unwrap_or_default().to_owned(),
            );
            s.children.push(opt);
        }
        match &c["value"] {
            Json::Array(list) => {
                if let Some(first) = list.first().and_then(Json::as_str) {
                    s.props.insert("value".into(), first.to_owned());
                }
                if list.len() > 1 {
                    self.note(LossKind::Values, "only the first selected value is kept");
                }
            }
            other => self.put(&mut s, "value", other),
        }
        self.checks(&mut s, &c["checks"]);
        s
    }
}
