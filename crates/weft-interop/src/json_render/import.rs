//! json-render → Weft (SPEC §9, "From json-render"). The elements reachable from `root` become the
//! role tree `weft-import` builds documents from, so ids, required props, content rules and the
//! loss table work as for every other importer. The spec is only read: no expression in it is
//! evaluated, and its size, depth and element count are bounded (`W602`).

use std::collections::HashSet;

use serde_json::{Map, Value as Json};
use weft_core::{
    Catalog, Code, ComponentDef, Content, Diagnostic, Value, is_action, is_id, is_loop_variable,
    is_token, parse_json, universal_prop,
};
use weft_import::{
    BuildOptions, ImportResult, LossKind, Losses, MAX_DEPTH, MAX_NODES, Note, Sem, build_document,
    empty_result, limit_reached, literal,
};

use crate::paths::from_pointer;
use crate::{sem, too_long};

/// A prop value as json-render states it: a literal, a binding or token, or nothing Weft can read.
enum Dyn {
    Lit(String),
    Val(Value),
    None,
}

struct Im<'a> {
    catalog: &'a Catalog,
    elements: &'a Map<String, Json>,
    /// Elements already placed; Weft ids are unique, so an element stands in one place only.
    used: HashSet<String>,
    /// The loop variables of the repetitions around the element being read.
    scope: Vec<String>,
    /// Losses of the element being read, handed to the element it becomes.
    notes: Vec<Note>,
    nodes: usize,
    truncated: bool,
}

pub fn from_json_render(text: &str, catalog: &Catalog) -> ImportResult {
    if let Some(result) = too_long(text) {
        return result;
    }
    let mut diagnostics = Vec::new();
    let spec = parse_json(text).unwrap_or_default();
    let (root, elements) = (spec["root"].as_str(), spec["elements"].as_object());
    let (Some(root), Some(elements)) = (
        root,
        elements.filter(|e| root.is_some_and(|r| e.contains_key(r))),
    ) else {
        let what = "The input is not a json-render spec with a root element.";
        let expected = "a JSON object whose `root` names an entry of its `elements` map";
        diagnostics.push(Diagnostic::new(Code::W601, "#", what, expected));
        return empty_result(diagnostics);
    };
    let mut im = Im {
        catalog,
        elements,
        used: HashSet::new(),
        scope: Vec::new(),
        notes: Vec::new(),
        nodes: 0,
        truncated: false,
    };
    if !spec["state"].is_null() {
        im.note(
            LossKind::Values,
            "the state model is not part of a document",
        );
    }
    let mut top = im.conv(root, 0);
    if top.is_empty() {
        top.push(sem("screen", ""));
    }
    top[0].notes.append(&mut im.notes);
    if im.truncated {
        let what = "is larger or deeper than the import limit";
        limit_reached(&mut diagnostics, "#", what);
    }
    let reserved = elements.keys().filter(|k| is_id(k)).cloned().collect();
    build_document(
        &top,
        BuildOptions {
            catalog,
            reserved,
            diagnostics,
        },
    )
    .result
}

/// A condition on one path, `{ "$state" | "$item": path, "not"?: true }`: the path, whether it
/// reads the item, and whether it is negated. Any comparison makes it something Weft cannot say.
fn condition(c: &Json) -> Option<(&str, bool, bool)> {
    let o = c.as_object()?;
    let not = match o.get("not") {
        None => false,
        Some(Json::Bool(true)) => true,
        Some(_) => return None,
    };
    if o.len() != 1 + usize::from(not) {
        return None;
    }
    if let Some(p) = o.get("$state").and_then(Json::as_str) {
        return Some((p, false, not));
    }
    o.get("$item")
        .and_then(Json::as_str)
        .map(|p| (p, true, not))
}

fn shows_text(def: &ComponentDef) -> bool {
    matches!(def.content, Content::Text | Content::Mixed)
}

impl<'a> Im<'a> {
    fn note(&mut self, kind: LossKind, note: impl Into<String>) {
        self.notes.push(Note {
            kind,
            note: note.into(),
        });
    }

    fn bind(&mut self, pointer: &str, item: bool, not: bool) -> Dyn {
        let bind = if item {
            from_pointer(pointer, &self.scope)
        } else {
            // A `$state` path is absolute whatever repetition it stands in.
            pointer
                .starts_with('/')
                .then(|| from_pointer(pointer, &[]))
                .flatten()
        };
        match bind {
            Some(bind) => Dyn::Val(Value::Bind { bind, not }),
            None => {
                self.note(
                    LossKind::Bindings,
                    format!("{pointer} is not a Weft data path here; it is left out"),
                );
                Dyn::None
            }
        }
    }

    fn dynamic(&mut self, v: &Json) -> Dyn {
        let o = match v {
            Json::String(t) => return Dyn::Lit(t.clone()),
            Json::Number(n) => return Dyn::Lit(n.to_string()),
            Json::Bool(b) => return Dyn::Lit(b.to_string()),
            Json::Null => return Dyn::None,
            Json::Object(o) => o,
            Json::Array(_) => {
                self.note(LossKind::Values, "a list value has no Weft form");
                return Dyn::None;
            }
        };
        if o.len() == 1 {
            if let Some(t) = o.get("token").and_then(Json::as_str) {
                if is_token(t) {
                    return Dyn::Val(Value::Token(t.to_owned()));
                }
                self.note(LossKind::Tokens, format!("{t} is not a token path"));
                return Dyn::None;
            }
            for (op, item) in [
                ("$state", false),
                ("$bindState", false),
                ("$item", true),
                ("$bindItem", true),
            ] {
                if let Some(p) = o.get(op).and_then(Json::as_str) {
                    return self.bind(p, item, false);
                }
            }
        }
        // The one `$cond` the exporter writes: a negated binding.
        if o.len() == 3
            && o.get("$then") == Some(&Json::Bool(true))
            && o.get("$else") == Some(&Json::Bool(false))
            && let Some((p, item, true)) = o.get("$cond").and_then(condition)
        {
            return self.bind(p, item, true);
        }
        let keys: Vec<&str> = o.keys().map(String::as_str).collect();
        let note = format!("the expression {{{}}} has no Weft form", keys.join(", "));
        self.note(LossKind::Values, note);
        Dyn::None
    }

    fn props(&mut self, s: &mut Sem, kind: &str, def: &ComponentDef, props: &Json) {
        for (name, v) in props.as_object().into_iter().flatten() {
            let declared = def.prop(name).is_some() || universal_prop(name).is_some();
            if (kind == "screen" && name == "weft") || name == "id" {
                continue;
            }
            if !declared {
                self.note(LossKind::Props, format!("<{kind}> has no prop {name}"));
                continue;
            }
            match (name.as_str(), self.dynamic(v)) {
                (_, Dyn::None) => {}
                // Set as a value: as a name, the builder would drop a label equal to the content.
                ("label", Dyn::Lit(t)) => {
                    let mut lost = Losses::default();
                    let label = literal(&mut lost, "", &t);
                    self.notes.extend(lost.0.into_iter().map(|l| Note {
                        kind: l.kind,
                        note: l.note,
                    }));
                    s.values.insert(name.clone(), Value::String(label));
                }
                ("text", Dyn::Lit(t)) if shows_text(def) => s.children.push(Sem::text(t)),
                ("hidden", Dyn::Lit(t)) => {
                    s.values.insert(name.clone(), Value::Bool(t == "true"));
                }
                (_, Dyn::Lit(t)) => {
                    s.props.insert(name.clone(), t);
                }
                (_, Dyn::Val(v)) => {
                    s.values.insert(name.clone(), v);
                }
            }
        }
    }

    fn visible(&mut self, s: &mut Sem, v: &Json) {
        let hidden = match (v, condition(v)) {
            (Json::Null, _) => return,
            (Json::Bool(b), _) => Dyn::Val(Value::Bool(!b)),
            (_, Some((p, item, not))) => self.bind(p, item, !not),
            _ => {
                let note = "this visible condition has no Weft form; the element is always shown";
                self.note(LossKind::Hidden, note);
                return;
            }
        };
        if let Dyn::Val(v) = hidden {
            s.values.insert("hidden".into(), v);
        }
    }

    fn events(&mut self, s: &mut Sem, on: &Json) {
        for (event, binding) in on.as_object().into_iter().flatten() {
            let (first, more) = match binding {
                Json::Array(list) => (list.first(), list.len() > 1),
                b => (Some(b), false),
            };
            if more {
                self.note(
                    LossKind::Actions,
                    format!("only the first action of {event} is kept"),
                );
            }
            let Some(b) = first else { continue };
            match b["action"].as_str() {
                Some(action) if is_action(action) => {
                    s.on.insert(event.clone(), action.to_owned());
                }
                _ => {
                    let note = format!("the action of {event} is not a Weft action name");
                    self.note(LossKind::Actions, note);
                    continue;
                }
            }
            let params = b["params"].as_object().into_iter().flatten();
            if params.map(|(k, _)| k).any(|k| k != "id" && k != "item") {
                self.note(LossKind::Actions, "action params have no Weft form");
            }
            for k in ["confirm", "onSuccess", "onError", "preventDefault"] {
                if !b[k].is_null() {
                    self.note(LossKind::Actions, format!("{k} has no Weft form"));
                }
            }
        }
    }

    fn keys(&mut self, list: &Json, depth: usize, slot: Option<&str>) -> Vec<Sem> {
        let mut out = Vec::new();
        for key in list
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
        {
            for mut s in self.conv(key, depth + 1) {
                s.slot = slot.map(str::to_owned);
                out.push(s);
            }
        }
        out
    }

    fn conv(&mut self, key: &str, depth: usize) -> Vec<Sem> {
        self.nodes += 1;
        if self.nodes > MAX_NODES || depth > MAX_DEPTH {
            self.truncated = true;
            return Vec::new();
        }
        let elements = self.elements;
        let Some(el) = elements.get(key) else {
            self.note(
                LossKind::Structure,
                format!("element {key} is used but not defined"),
            );
            return Vec::new();
        };
        if !self.used.insert(key.to_owned()) {
            let note = format!("element {key} is used again; only its first place is kept");
            self.note(LossKind::Structure, note);
            return Vec::new();
        }
        let mark = self.notes.len();
        let mut out = self.element(el, key, depth);
        // The notes of an element belong to what it became; a dropped one passes them up.
        if let Some(first) = out.first_mut() {
            first.notes.extend(self.notes.drain(mark..));
        }
        out
    }

    /// The Weft id of the element under `key`; any other key leaves the builder to generate one.
    fn id(&mut self, key: &str) -> Option<String> {
        if is_id(key) {
            return Some(key.to_owned());
        }
        let note = format!("{key:?} is not a Weft id; a generated one stands in");
        self.note(LossKind::Ids, note);
        None
    }

    /// The `<each>` of a `repeat`, before its children are read; the scope then holds its variable.
    fn repetition(&mut self, el: &Json) -> Sem {
        let repeat = &el["repeat"];
        let mut each = sem("each", "");
        let in_ = match &repeat["statePath"] {
            Json::String(p) => self.bind(p, false, false),
            Json::Object(o) if o.len() == 1 => match o.get("$item").and_then(Json::as_str) {
                Some(p) => self.bind(p, true, false),
                None => Dyn::None,
            },
            _ => Dyn::None,
        };
        match in_ {
            Dyn::Val(v) => {
                each.values.insert("in".into(), v);
            }
            _ => self.note(
                LossKind::Repetition,
                "the repeated list is not a Weft data path",
            ),
        }
        if !repeat["key"].is_null() {
            self.note(LossKind::Repetition, "repeat.key has no Weft form");
        }
        let wanted = el["props"]["as"].as_str().filter(|v| {
            el["type"] == "each" && is_loop_variable(v) && !self.scope.iter().any(|s| s == v)
        });
        if wanted.is_none() && el["type"] == "each" && el["props"]["as"].is_string() {
            let note = "the loop variable is not a free loop variable name; another stands in";
            self.note(LossKind::Repetition, note);
        }
        let var = match wanted {
            Some(v) => v.to_owned(),
            // Inner variables may not shadow outer ones (SPEC §4.3), so the default is numbered.
            None => (1..)
                .map(|n| {
                    if n == 1 {
                        "item".to_owned()
                    } else {
                        format!("item{n}")
                    }
                })
                .find(|v| !self.scope.contains(v))
                .unwrap_or_default(),
        };
        each.props.insert("as".into(), var.clone());
        self.scope.push(var);
        each
    }

    fn element(&mut self, el: &'a Json, key: &str, depth: usize) -> Vec<Sem> {
        let ty = el["type"].as_str().unwrap_or_default();
        let mut each = (!el["repeat"].is_null()).then(|| self.repetition(el));
        let mut children = self.keys(&el["children"], depth, None);
        if let Some(e) = each.as_mut() {
            self.scope.pop();
            e.children = std::mem::take(&mut children);
        }
        let mut slotted = Vec::new();
        for (name, keys) in el["slots"].as_object().into_iter().flatten() {
            let slot = (name != "default").then_some(name.as_str());
            slotted.extend(self.keys(keys, depth, slot));
        }
        let def = self.catalog.components.get(ty).filter(|_| ty != "each");
        let Some(def) = def else {
            if ty != "each" {
                let note =
                    format!("the type {ty:?} is not a kind of the catalog; its content is kept");
                self.note(LossKind::Kinds, note);
            } else if let Some(e) = each.as_mut() {
                // The element is the repetition itself, so its id and condition are the `<each>`'s.
                e.id = self.id(key);
                self.visible(e, &el["visible"]);
            } else {
                let note = "an each element without repeat; its content is kept";
                self.note(LossKind::Repetition, note);
            }
            if ty == "each" && !(el["on"].is_null() && el["watch"].is_null()) {
                self.note(LossKind::Actions, "<each> has no events");
            }
            return each.into_iter().chain(children).chain(slotted).collect();
        };
        let mut s = sem(ty, "");
        s.id = self.id(key);
        self.props(&mut s, ty, def, &el["props"]);
        self.visible(&mut s, &el["visible"]);
        self.events(&mut s, &el["on"]);
        if !el["watch"].is_null() {
            self.note(LossKind::Actions, "watch has no Weft form");
        }
        s.children
            .extend(each.into_iter().chain(children).chain(slotted));
        vec![s]
    }
}
