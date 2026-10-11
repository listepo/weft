//! Sheets and alerts presented over the screen.
use weft_core::{Node, Value};

use super::super::syntax::{Expr, Stmt, plain_string};
use crate::data::Leaf;

use super::helpers::{Mod, Mods, arg, closure, first_arg, ident, implicit, unwind};
use super::types::{Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn sheet(&mut self, m: &Mod, hidden: Option<&Mod>, path: &str) -> Option<Node> {
        let content = m
            .closures
            .first()
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let mut inner_mods = Mods(vec![]);
        let mut statements = content.clone();
        // `VStack(alignment: .leading) { … }` with the dialog's modifiers.
        if let [Stmt::Expr(e)] = content.as_slice() {
            let (base, mods) = unwind(e);
            if let Expr::Call(c) = base
                && ident(&c.callee) == Some("VStack")
            {
                statements = closure(&c.closures, None)
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                inner_mods = Mods(mods);
            }
        }
        let (mut node, here) = self.element("dialog", &mut inner_mods, "", path)?;
        self.writes(
            &mut node,
            "open",
            arg(&m.args, "isPresented"),
            Leaf::Bool,
            &here,
        );
        if let Some(Expr::Closure(c)) = arg(&m.args, "onDismiss") {
            let body = c.body.clone();
            self.event(&mut node, "close", &body, &here);
        }
        let children = self.nested(|r| r.views(&statements, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        if let Some(inset) = inner_mods.take("safeAreaInset") {
            let body = inset
                .closures
                .first()
                .map(|c| c.body.clone())
                .unwrap_or_default();
            let actions = match body.as_slice() {
                [Stmt::Expr(Expr::Call(c))] if ident(&c.callee) == Some("HStack") => {
                    closure(&c.closures, None)
                        .map(|c| c.body.clone())
                        .unwrap_or_default()
                }
                other => other.to_vec(),
            };
            let views =
                self.nested(|r| r.views(&actions, Place::Nodes, &format!("{here}/slot[actions]")));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert("actions".to_owned(), views);
            }
        }
        if let Some(i) = inner_mods.take("presentationBackgroundInteraction") {
            match implicit(first_arg(&i.args)) {
                Some("disabled") => {
                    node.props.insert("modal".to_owned(), Value::Bool(true));
                }
                Some("enabled") => {
                    node.props.insert("modal".to_owned(), Value::Bool(false));
                }
                _ => {}
            }
        }
        if let Some(h) = hidden {
            inner_mods.0.push(h.clone());
        }
        self.common(&mut node, inner_mods, &here);
        Some(node)
    }

    /// `.alert(title, isPresented:) { actions } message: { … }` is a dialog too.
    pub(super) fn alert(&mut self, m: &Mod, path: &str) -> Option<Node> {
        let title = match first_arg(&m.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let (mut node, here) = self.element("dialog", &mut Mods(vec![]), &title, path)?;
        if let Some(t) = first_arg(&m.args).cloned()
            && let Some(v) = self.value(&t, Leaf::Text, &here)
        {
            node.props.insert("label".to_owned(), v);
        }
        self.writes(
            &mut node,
            "open",
            arg(&m.args, "isPresented"),
            Leaf::Bool,
            &here,
        );
        let actions = closure(&m.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let message = closure(&m.closures, Some("message"))
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let children = self.nested(|r| r.views(&message, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        let views =
            self.nested(|r| r.views(&actions, Place::Nodes, &format!("{here}/slot[actions]")));
        let views = self.elements_only(views, &here);
        if !views.is_empty() {
            node.slots.insert("actions".to_owned(), views);
        }
        Some(node)
    }
}
