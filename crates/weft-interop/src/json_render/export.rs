//! Weft → json-render (SPEC §9, "To json-render"). The catalog of the spec is the Weft catalog, so
//! each element keeps its kind, id and props; only what json-render evaluates itself changes form:
//! bindings become `$state`/`$item` expressions, `hidden` becomes `visible`, events become `on`.

use std::collections::HashSet;

use serde_json::{Map, Value as Json, json};
use weft_core::{Catalog, Child, Content, Document, Node, Value};
use weft_import::{Loss, LossKind, Losses, safe_url};

use crate::ids::{collect_ids, fresh_id};
use crate::number;
use crate::paths::to_pointer;

pub struct Exported {
    pub spec: Json,
    pub losses: Vec<Loss>,
}

struct Ex<'c> {
    catalog: &'c Catalog,
    elements: Map<String, Json>,
    ids: HashSet<String>,
    losses: Losses,
    /// The loop variables around the node being written, outermost first.
    scope: Vec<String>,
}

/// `state` is sample data for the spec's state model; without it the spec has no `state`, since
/// json-render checks every `repeat` against the state a spec carries.
pub fn to_json_render(
    doc: &Document,
    catalog: &Catalog,
    state: Option<&Map<String, Json>>,
) -> Exported {
    let mut ex = Ex {
        catalog,
        elements: Map::new(),
        ids: HashSet::new(),
        losses: Losses::default(),
        scope: Vec::new(),
    };
    collect_ids(&doc.root, &mut ex.ids);
    let root = ex.element(&doc.root, "");
    let mut spec = json!({ "root": root, "elements": ex.elements });
    if let Some(state) = state {
        spec["state"] = Json::Object(state.clone());
    }
    Exported {
        spec,
        losses: ex.losses.0,
    }
}

/// A condition on one path, as `visible` and `$cond` take it.
fn condition(item: bool, at: String, not: bool) -> Json {
    let mut c = Map::new();
    c.insert(if item { "$item" } else { "$state" }.into(), at.into());
    if not {
        c.insert("not".into(), true.into());
    }
    Json::Object(c)
}

impl Ex<'_> {
    /// Where a binding reads, as json-render addresses it: `(true, field)` for the innermost
    /// item, `(false, pointer)` for the state.
    fn pointer(&mut self, bind: &str, path: &str, name: &str) -> Option<(bool, String)> {
        // json-render reads the whole item as the field "", which Weft's pointers do not name.
        if self.scope.last().map(|v| format!("${v}")).as_deref() == Some(bind) {
            return Some((true, String::new()));
        }
        match to_pointer(bind, &self.scope) {
            Ok(p) => Some((!p.starts_with('/'), p)),
            Err(why) => {
                self.losses
                    .push(LossKind::Bindings, path, format!("{name}: {why}"));
                None
            }
        }
    }

    fn value(&mut self, v: &Value, writable: bool, path: &str, name: &str) -> Option<Json> {
        Some(match v {
            Value::String(s) => json!(s),
            Value::Number(n) => number(*n),
            Value::Bool(b) => json!(b),
            Value::Token(t) => json!({ "token": t }),
            Value::Bind { bind, not } => {
                let (item, at) = self.pointer(bind, path, name)?;
                if *not {
                    return Some(
                        json!({ "$cond": condition(item, at, true), "$then": true, "$else": false }),
                    );
                }
                let op = match (item, writable) {
                    (true, true) => "$bindItem",
                    (false, true) => "$bindState",
                    (true, false) => "$item",
                    (false, false) => "$state",
                };
                let mut m = Map::new();
                m.insert(op.into(), at.into());
                Json::Object(m)
            }
        })
    }

    fn list(&mut self, list: &[Child], path: &str) -> Json {
        let mut keys = Vec::new();
        for c in list {
            keys.push(match c {
                Child::Node(n) => self.element(n, path),
                Child::Text(t) => {
                    // json-render children are element keys only, so the run needs an element.
                    self.losses.push(
                        LossKind::Text,
                        path,
                        format!("text \"{t}\" beside elements becomes a text element"),
                    );
                    let key = fresh_id(&mut self.ids, "text");
                    let run = json!({ "type": "text", "props": { "text": t }, "children": [] });
                    self.elements.insert(key.clone(), run);
                    key
                }
            });
        }
        keys.into()
    }

    fn element(&mut self, n: &Node, parent: &str) -> String {
        let key =
            n.id.clone()
                .unwrap_or_else(|| fresh_id(&mut self.ids, &n.kind));
        let path = format!("{parent}/{}#{key}", n.kind);
        // Parents come before their children, so a spec streamed in order renders top down.
        self.elements.insert(key.clone(), Json::Null);
        let def = self.catalog.components.get(&n.kind);
        let (mut props, mut extra) = (Map::new(), Map::new());
        for (name, v) in &n.props {
            match (n.kind.as_str(), name.as_str(), v) {
                ("each", "in", Value::Bind { bind, not: false }) => {
                    if let Some((item, at)) = self.pointer(bind, &path, name) {
                        let at = if item {
                            json!({ "$item": at })
                        } else {
                            at.into()
                        };
                        extra.insert("repeat".into(), json!({ "statePath": at }));
                    }
                }
                (_, "hidden", Value::Bool(b)) => {
                    extra.insert("visible".into(), (!b).into());
                }
                (_, "hidden", Value::Bind { bind, not }) => {
                    if let Some((item, at)) = self.pointer(bind, &path, name) {
                        extra.insert("visible".into(), condition(item, at, !not));
                    }
                }
                _ => {
                    let writable = def
                        .and_then(|d| d.prop(name))
                        .is_some_and(|p| p.writable == Some(true));
                    if let Some(j) = self.value(v, writable, &path, name) {
                        // SPEC §9 Trust: a literal href or src is http, https, mailto or relative.
                        if matches!(name.as_str(), "href" | "src")
                            && let Json::String(s) = &j
                        {
                            match safe_url(s) {
                                Some(url) => {
                                    props.insert(name.clone(), url.into());
                                }
                                None => self.losses.push(
                                    LossKind::Props,
                                    &path,
                                    format!(
                                        "{name} is not http, https, mailto or relative; it is left out"
                                    ),
                                ),
                            }
                        } else {
                            props.insert(name.clone(), j);
                        }
                    }
                }
            }
        }
        let var = match n.props.get("as") {
            Some(Value::String(var)) if n.kind == "each" => Some(var.clone()),
            _ => None,
        };
        let pushed = var.is_some();
        self.scope.extend(var);
        let shows_text = def.is_some_and(|d| matches!(d.content, Content::Text | Content::Mixed));
        let text: Option<Vec<&str>> = n
            .children
            .iter()
            .map(|c| match c {
                Child::Text(t) => Some(t.as_str()),
                Child::Node(_) => None,
            })
            .collect();
        let children = match text {
            Some(runs) if shows_text && !runs.is_empty() => {
                props.insert("text".into(), runs.join(" ").into());
                json!([])
            }
            _ => self.list(&n.children, &path),
        };
        let mut slots = Map::new();
        for (name, list) in &n.slots {
            let keys = self.list(list, &format!("{path}/slot[{name}]"));
            slots.insert(name.clone(), keys);
        }
        let mut on = Map::new();
        for (event, action) in &n.on {
            // What a Weft host receives with an action (SPEC §2.2): the element and the item.
            let mut params = json!({ "id": key });
            if !self.scope.is_empty() {
                params["item"] = json!({ "$item": "" });
            }
            on.insert(event.clone(), json!({ "action": action, "params": params }));
        }
        if pushed {
            self.scope.pop();
        }
        let mut el = json!({ "type": n.kind, "props": props, "children": children });
        if !slots.is_empty() {
            el["slots"] = slots.into();
        }
        if !on.is_empty() {
            el["on"] = on.into();
        }
        for (k, v) in extra {
            el[k.as_str()] = v;
        }
        self.elements.insert(key.clone(), el);
        key
    }
}
