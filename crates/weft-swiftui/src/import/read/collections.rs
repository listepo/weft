//! Lists, tabs, menus and custom views.
use weft_core::{Child, Content, PropType, Value};

use super::super::LossKind;
use super::super::syntax::{Call, Closure, Expr, Stmt};
use crate::data::Leaf;
use crate::swift;

use super::helpers::{Mods, arg, closure, first_arg, ident, implicit, type_name, unwind};
use super::types::{Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn list(&mut self, call: &Call, mods: Mods, path: &str) -> Vec<Child> {
        // `List(items) { item in … }` is a list over an `<each>`.
        if let Some(data) = first_arg(&call.args).cloned() {
            let each = Call {
                callee: Box::new(Expr::Ident {
                    name: "ForEach".to_owned(),
                    raw: false,
                }),
                args: call.args.clone(),
                closures: call.closures.clone(),
            };
            let _ = data;
            let wrapper = Call {
                callee: call.callee.clone(),
                args: vec![],
                closures: vec![Closure {
                    label: None,
                    params: vec![],
                    body: vec![Stmt::Expr(Expr::Call(each))],
                }],
            };
            return self.container("list", &wrapper, mods, Place::List, path);
        }
        self.container("list", call, mods, Place::List, path)
    }

    pub(super) fn tabs(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("tabs", &mut mods, "", path) else {
            return vec![];
        };
        self.writes(
            &mut node,
            "selected",
            arg(&call.args, "selection"),
            Leaf::Text,
            &here,
        );
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Tabs, &here));
        self.finish(node, mods, &here)
    }

    pub(super) fn tab(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some(item) = mods.take("tabItem") else {
            if name == "ForEach" {
                return self.each(call, mods, Place::Tabs, path);
            }
            self.lose(
                LossKind::Structure,
                path,
                format!("`{name}` without `.tabItem` is not a tab and is dropped"),
            );
            return vec![];
        };
        let Some((mut node, here)) = self.element("tab", &mut mods, "", path) else {
            return vec![];
        };
        let label = item.closures.first().and_then(|c| match c.body.as_slice() {
            [Stmt::Expr(e)] => {
                let (base, _) = unwind(e);
                match base {
                    Expr::Call(t) if matches!(ident(&t.callee), Some("Text" | "Label")) => {
                        first_arg(&t.args).cloned()
                    }
                    _ => None,
                }
            }
            _ => None,
        });
        if let Some(e) = label
            && let Some(v) = self.value(&e, Leaf::Text, &here)
        {
            node.props.insert("label".to_owned(), v);
        }
        // `.tag(id)` selects the tab by its id; nothing else to keep.
        mods.take("tag");
        if name == "VStack" {
            if call.args.iter().any(|a| {
                !(a.label.as_deref() == Some("alignment")
                    && implicit(Some(&a.value)) == Some("leading"))
            }) {
                self.lose(
                    LossKind::Layout,
                    &here,
                    "the tab's stack layout is not kept",
                );
            }
            let body = closure(&call.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default();
            let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
            node.children = self.elements_only(children, &here);
        } else {
            let children = self.nested(|r| r.element_view(name, call, Mods(vec![]), &here));
            node.children = self.elements_only(children, &here);
        }
        self.finish(node, mods, &here)
    }

    pub(super) fn menu(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("menu", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        let items = if first_arg(&call.args).is_some()
            || closure(&call.closures, Some("label")).is_some()
        {
            closure(&call.closures, None)
        } else {
            closure(&call.closures, Some("content"))
        };
        let body = items.map(|c| c.body.clone()).unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Menu, &here));
        self.finish(node, mods, &here)
    }

    pub(super) fn mixed(
        &mut self,
        kind: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        if !call.args.is_empty() {
            self.lose(
                LossKind::Layout,
                &here,
                "the stack's alignment and spacing are not kept",
            );
        }
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        // A lone `Text(verbatim:)` or `Text(expr)` without modifiers is the element's own text.
        if let [Stmt::Expr(Expr::Call(c))] = body.as_slice()
            && ident(&c.callee) == Some("Text")
            && (arg(&c.args, "verbatim").is_some()
                || matches!(first_arg(&c.args), Some(e) if !matches!(e, Expr::Str(_))))
        {
            self.text_content(&mut node, c, &here);
        } else {
            node.children = self.nested(|r| r.mixed_children(&body, &here));
        }
        if kind == "alert"
            && let Some(m) = mods.take("backgroundStyle")
        {
            let color = match first_arg(&m.args) {
                Some(Expr::Call(c)) => match c.callee.as_ref() {
                    Expr::Member { base, .. } => type_name(base),
                    _ => None,
                },
                Some(other) => type_name(other),
                None => None,
            };
            let tone = match color.as_deref() {
                Some("Color.blue") => Some("info"),
                Some("Color.green") => Some("success"),
                Some("Color.orange") => Some("warning"),
                Some("Color.red") => Some("danger"),
                _ => None,
            };
            match tone {
                Some(t) => {
                    node.props
                        .insert("tone".to_owned(), Value::String(t.to_owned()));
                }
                None => self.lose(
                    LossKind::Layout,
                    &here,
                    "a background with no Weft tone is not kept",
                ),
            }
        }
        self.finish(node, mods, &here)
    }

    /// The call of a view the app writes for a kind of its own catalog: labelled arguments are
    /// props and `on…` handlers, builders are the content and the slots (README, "Custom
    /// components"). The first trailing builder is the first the view takes.
    pub(super) fn custom_view(
        &mut self,
        kind: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some(def) = self.catalog.components.get(kind) else {
            return vec![];
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        let mut builders: Vec<String> = vec![];
        if def.content != Content::None {
            builders.push("content".to_owned());
        }
        builders.extend(
            def.slots
                .iter()
                .flat_map(|s| s.keys())
                .map(|n| swift::label(n)),
        );
        let mut bodies: Vec<(String, Vec<Stmt>)> = vec![];
        for a in &call.args {
            let Some(label) = a.label.as_deref() else {
                self.lose(
                    LossKind::Props,
                    &here,
                    format!(
                        "an argument of `{}` without a label is not kept",
                        swift::view_name(kind)
                    ),
                );
                continue;
            };
            let prop = def
                .props
                .iter()
                .flatten()
                .find(|(name, _)| swift::label(name) == label || name.as_str() == label);
            if let Some((name, prop)) = prop {
                let value = match prop.kind {
                    PropType::Token => self.token(&a.value).map(Value::Token),
                    PropType::Number => self.value(&a.value, Leaf::Int, &here),
                    PropType::Boolean => self.value(&a.value, Leaf::Bool, &here),
                    PropType::String | PropType::Enum => self.value(&a.value, Leaf::Text, &here),
                };
                match value {
                    Some(v) => {
                        node.props.insert(name.clone(), v);
                    }
                    None => self.lose(
                        if prop.kind == PropType::Token {
                            LossKind::Tokens
                        } else {
                            LossKind::Bindings
                        },
                        &here,
                        format!("the value of `{name}` is not a literal, a data path or a token"),
                    ),
                }
                continue;
            }
            if label == "state" && def.states.is_some() {
                if let Some(v) = self.value(&a.value, Leaf::Text, &here) {
                    node.props.insert("state".to_owned(), v);
                }
                continue;
            }
            let event = def
                .events
                .iter()
                .flatten()
                .find(|e| swift::event_label(e) == label)
                .cloned();
            match (event, &a.value) {
                (Some(event), Expr::Closure(c)) => {
                    self.event(&mut node, &event, &c.body.clone(), &here);
                }
                (None, Expr::Closure(c)) if builders.iter().any(|b| b == label) => {
                    bodies.push((label.to_owned(), c.body.clone()));
                }
                _ => self.lose(
                    LossKind::Props,
                    &here,
                    format!(
                        "`{}` has no `{label}` argument in Weft",
                        swift::view_name(kind)
                    ),
                ),
            }
        }
        for (i, c) in call.closures.iter().enumerate() {
            let label = match (&c.label, i) {
                (Some(l), _) => Some(l.clone()),
                (None, 0) => builders
                    .iter()
                    .find(|b| !bodies.iter().any(|(l, _)| l == *b))
                    .cloned(),
                (None, _) => None,
            };
            match label.filter(|l| builders.contains(l)) {
                Some(l) => bodies.push((l, c.body.clone())),
                None => self.lose(
                    LossKind::Slots,
                    &here,
                    format!(
                        "a view builder of `{}` has no slot in Weft",
                        swift::view_name(kind)
                    ),
                ),
            }
        }
        let slots: Vec<String> = def.slots.iter().flat_map(|s| s.keys()).cloned().collect();
        for (label, body) in bodies {
            if label == "content" && def.content != Content::None {
                let children = self.nested(|r| r.mixed_children(&body, &here));
                node.children.extend(children);
                continue;
            }
            let Some(name) = slots.iter().find(|n| swift::label(n) == label) else {
                continue;
            };
            let slot_path = format!("{here}/slot[{name}]");
            let views = self.nested(|r| r.views(&body, Place::Nodes, &slot_path));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert(name.clone(), views);
            }
        }
        // The canonical form orders slots and events by name.
        node.slots.sort_keys();
        node.on.sort_keys();
        self.finish(node, mods, &here)
    }
}
