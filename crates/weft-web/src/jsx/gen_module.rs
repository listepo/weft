use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Map, Value as Json};

use super::Gen;
use super::helpers::{TS_KEY, TS_TYPES, component_name};
use super::model::{Ctx, N, Scope, Var};
use super::runtime::RUNTIME;
use super::tree::{C, Code, attr_js, el, js, s};
use crate::js::{compare_utf16, quote};

impl<'a> Gen<'a> {
    // ---- The module ----

    pub(super) fn root_body(&mut self, c: C<'a>, hidden: Option<String>) -> String {
        if self.solid() {
            // A component body runs once; only JSX tracks the data it reads, so an expression at
            // the root is put inside a fragment.
            let c = match hidden {
                Some(h) => self.cond(&format!("!{h}"), c, None),
                None => c,
            };
            let c = match c {
                C::Code(_) => C::J(el("", vec![], vec![c])),
                other => other,
            };
            return self.expr_of(&c, "  ");
        }
        match hidden {
            None => self.expr_of(&c, "  "),
            Some(h) => format!("{h} ? null : {}", self.expr_of(&c, "  ")),
        }
    }

    /// One function per inline fragment, in name order, printed before the screen component.
    pub(super) fn inline_functions(&mut self, document: &'a Json) -> String {
        let Some(map) = document.get("fragments").and_then(Json::as_object) else {
            return String::new();
        };
        let mut names: Vec<&String> = map.keys().collect();
        names.sort_by(|a, b| compare_utf16(a, b));
        let mut out = String::new();
        for name in names {
            let Some(node) = map.get(name).and_then(Json::as_object) else {
                continue;
            };
            out.push_str(&self.inline_function(name, node));
            out.push('\n');
        }
        out
    }

    pub(super) fn inline_function(&mut self, name: &str, node: &'a Map<String, Json>) -> String {
        let params_base = if self.solid() {
            "props.params"
        } else {
            "params"
        };
        let id_base = if self.solid() { "props.id" } else { "id" };
        let mut vars = HashMap::new();
        let children = node.get("children").and_then(Json::as_array);
        let mut body_at = 0;
        if let Some(items) = children {
            for child in items {
                let Some(obj) = child.as_object() else { break };
                if obj.get("kind").and_then(Json::as_str) != Some("param") {
                    break;
                }
                body_at += 1;
                let props = obj.get("props");
                let Some(pname) = props.and_then(|p| p.get("name")).and_then(Json::as_str) else {
                    continue;
                };
                if props.and_then(|p| p.get("type")).and_then(Json::as_str) == Some("slot") {
                    continue;
                }
                vars.insert(
                    pname.to_owned(),
                    Var {
                        ident: format!("{params_base}.{pname}"),
                        path: Vec::new(),
                    },
                );
            }
        }
        let scope = Rc::new(Scope {
            vars,
            prefix: vec![js(id_base), s("/")],
            ..Scope::default()
        });
        let items = children.map(|list| &list[body_at..]).unwrap_or(&[]);
        self.in_inline = true;
        let pieces = self.pieces_of(items, &scope, 0);
        let kids = self.kids(&pieces, &Ctx::default(), None);
        self.in_inline = false;
        let rendered = match kids.len() {
            0 => C::Code(Code::lit("null")),
            1 => kids
                .into_iter()
                .next()
                .unwrap_or_else(|| C::Code(Code::lit("null"))),
            _ => C::J(el("", vec![], kids)),
        };
        let body = self.expr_of(&rendered, "  ");
        let fname = component_name(name);
        format!(
            "function {fname}({}) {{\n  return {body};\n}}\n",
            self.inline_head()
        )
    }

    pub(super) fn inline_head(&self) -> String {
        let typed = "WeftProps & { params?: any; slots?: any; id?: string }";
        if self.solid() {
            if self.ts {
                format!("props: {typed}")
            } else {
                "props".to_owned()
            }
        } else if self.ts {
            format!("{{ data, actions, onChange, params, slots, id }}: {typed}")
        } else {
            "{ data, actions, onChange, params, slots, id }".to_owned()
        }
    }

    /// A `<use>` of an inline fragment, as a call of the component `inline_function` prints.
    pub(super) fn inline_use(&mut self, n: &Rc<N<'a>>, ctx: &Ctx<'a>) -> Option<C<'a>> {
        if n.kind != "use" {
            return None;
        }
        let name = n.raw("fragment")?.as_str()?;
        if !self.inline_names.contains(name) {
            return None;
        }
        let fname = component_name(name);
        let id = self.use_id_expr(n);
        let params = self.params_object(n);
        let slots = self.slots_object(n, ctx);
        if self.lit() {
            let data = self.data_ident();
            let actions = self.actions_ident();
            let on_change = self.on_change_ident();
            return Some(C::Code(Code::lit(format!(
                "{fname}({{ data: {data}, actions: {actions}, onChange: {on_change}, id: {id}, params: {params}, slots: {slots} }})"
            ))));
        }
        let attrs = vec![
            attr_js("data", self.data_ident()),
            attr_js("actions", self.actions_ident()),
            attr_js("onChange", self.on_change_ident()),
            attr_js("id", id),
            attr_js("params", params),
            attr_js("slots", slots),
        ];
        Some(C::J(el(&fname, attrs, vec![])))
    }

    pub(super) fn use_id_expr(&self, n: &N<'a>) -> String {
        if self.in_inline {
            let base = if self.solid() { "props.id" } else { "id" };
            format!("{base} + {}", quote(&format!("/{}", n.doc_id)))
        } else {
            quote(n.doc_id)
        }
    }

    pub(super) fn params_object(&mut self, n: &N<'a>) -> String {
        let mut parts = Vec::new();
        if let Some(props) = n.props {
            let mut keys: Vec<&String> =
                props.keys().filter(|k| k.as_str() != "fragment").collect();
            keys.sort_by(|a, b| compare_utf16(a, b));
            for key in keys {
                let (value, _) = self.resolve(props.get(key), &n.scope);
                parts.push(format!("{key}: {}", value.js));
            }
        }
        if let Some(on) = n.on {
            let mut events: Vec<&String> = on.keys().collect();
            events.sort_by(|a, b| compare_utf16(a, b));
            for event in events {
                let action = on.get(event).and_then(Json::as_str).unwrap_or("");
                parts.push(format!("{event}: {}", quote(action)));
            }
        }
        format!("{{ {} }}", parts.join(", "))
    }

    pub(super) fn slots_object(&mut self, n: &N<'a>, ctx: &Ctx<'a>) -> String {
        let Some(slots) = n.slots else {
            return "{}".to_owned();
        };
        let mut names: Vec<&String> = slots.keys().collect();
        names.sort_by(|a, b| compare_utf16(a, b));
        let mut parts = Vec::new();
        for name in names {
            let pieces = self.pieces(slots.get(name), &n.scope, n.depth + 1);
            let kids = self.kids(&pieces, ctx, None);
            let rendered = match kids.len() {
                0 => C::Code(Code::lit("null")),
                1 => kids
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| C::Code(Code::lit("null"))),
                _ => C::J(el("", vec![], kids)),
            };
            parts.push(format!("{name}: {}", self.expr_of(&rendered, "")));
        }
        format!("{{ {} }}", parts.join(", "))
    }

    pub(super) fn module(&self, name: &str, body: &str, functions: &str) -> String {
        if self.lit() {
            return self.lit_module(name, body, functions);
        }
        let mut out = String::new();
        if self.solid() {
            if !self.solid.is_empty() {
                let names: Vec<&str> = self.solid.iter().copied().collect();
                out.push_str(&format!(
                    "import {{ {} }} from \"solid-js\";\n",
                    names.join(", ")
                ));
            }
            if !self.solid_web.is_empty() {
                let names: Vec<&str> = self.solid_web.iter().copied().collect();
                out.push_str(&format!(
                    "import {{ {} }} from \"solid-js/web\";\n",
                    names.join(", ")
                ));
            }
            if !out.is_empty() {
                out.push('\n');
            }
        } else if self.fragment {
            if self.ts && self.used.contains("_keyed") {
                out.push_str("import { Fragment, type ReactNode } from \"react\";\n\n");
            } else {
                out.push_str("import { Fragment } from \"react\";\n\n");
            }
        }
        if self.ts {
            out.push_str(TS_TYPES);
            if self.used.contains("_press") || self.used.contains("_tabKey") {
                out.push_str(TS_KEY);
            }
            out.push('\n');
        }
        for h in RUNTIME.iter().filter(|h| self.used.contains(h.name)) {
            out.push_str(if self.ts { h.ts } else { h.js });
            out.push_str("\n\n");
        }
        if !functions.is_empty() {
            out.push_str(functions);
            if !functions.ends_with('\n') {
                out.push('\n');
            }
        }
        let params = match (self.solid(), self.ts) {
            (true, true) => "props: WeftProps",
            (true, false) => "props",
            (false, true) => "{ data, actions, onChange }: WeftProps",
            (false, false) => "{ data, actions, onChange }",
        };
        out.push_str(&format!(
            "export default function {name}({params}) {{\n  return {body};\n}}\n"
        ));
        out
    }
}
