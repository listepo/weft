//! `ForEach` loops and the views that wrap a list of children.
use weft_core::{Child, Node, Value};

use super::super::LossKind;
use super::super::syntax::{Call, Expr, Stmt, plain_string};

use super::helpers::{Mods, arg, closure, element_path, first_arg, ident, loop_name};
use super::types::{Loop, Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn mixed_children(&mut self, body: &[Stmt], path: &str) -> Vec<Child> {
        let mut out = vec![];
        for s in body {
            if let Stmt::Expr(Expr::Call(c)) = s
                && ident(&c.callee) == Some("Text")
                && c.args.len() == 1
                && let Some(Expr::Str(parts)) = first_arg(&c.args)
                && let Some(text) = plain_string(parts)
            {
                if !text.is_empty() {
                    let text = self.string(&text, path);
                    out.push(Child::Text(text));
                }
                continue;
            }
            out.extend(self.views(std::slice::from_ref(s), Place::Nodes, path));
        }
        out
    }

    /// A view where only `kind` may stand gets one around it.
    pub(super) fn wrapped(
        &mut self,
        kind: &str,
        name: &str,
        call: &Call,
        mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        if name == "ForEach" {
            let place = match kind {
                "item" => Place::List,
                "cell" => Place::Row,
                _ => Place::Table,
            };
            return self.each(call, mods, place, path);
        }
        let inner = if kind == "row" {
            Place::Row
        } else {
            Place::Nodes
        };
        // The content is read first so that a wrapper is only made around something.
        let provisional = format!("{path}/{kind}");
        let children =
            self.nested(|r| r.base_view(name, call, &mut Mods(mods.0), inner, &provisional));
        if children.is_empty() {
            return vec![];
        }
        let Some((mut node, _)) = self.element(kind, &mut Mods(vec![]), "", path) else {
            return vec![];
        };
        node.children = self.elements_only(children, path);
        vec![Child::Node(Box::new(node))]
    }

    pub(super) fn grid_row(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        // The row of column headers has no id of its own.
        if !mods.has("accessibilityIdentifier") {
            self.ignored(mods, path);
            return self.views(&body, Place::Header, path);
        }
        let Some((mut node, here)) = self.element("row", &mut mods, "", path) else {
            return vec![];
        };
        node.children = self.nested(|r| r.views(&body, Place::Row, &here));
        self.finish(node, mods, &here)
    }

    pub(super) fn each(
        &mut self,
        call: &Call,
        mut mods: Mods,
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let data = first_arg(&call.args).or_else(|| arg(&call.args, "data"));
        let Some(body) = call.closures.first() else {
            return vec![];
        };
        // `Array(x.enumerated())` with `{ index, item in }`, `x.indices` with `{ index in }`,
        // or `x` with `{ item in }`.
        let mut list = data.cloned();
        let mut enumerated = false;
        let mut indices = false;
        if let Some(Expr::Call(c)) = &list
            && ident(&c.callee) == Some("Array")
            && let Some(Expr::Call(inner)) = first_arg(&c.args)
            && let Expr::Member { base, name, .. } = inner.callee.as_ref()
            && name == "enumerated"
        {
            list = Some(base.as_ref().clone());
            enumerated = true;
        }
        if let Some(Expr::Member { base, name, .. }) = &list
            && name == "indices"
        {
            list = Some(base.as_ref().clone());
            indices = true;
        }
        let path_in = list.as_ref().and_then(|l| self.path(l));
        let (index, item) = match (enumerated, indices, body.params.as_slice()) {
            (true, _, [i, v]) => (Some(i.clone()), v.clone()),
            (false, true, [i]) => (Some(i.clone()), String::new()),
            (false, false, [v]) => (None, v.clone()),
            _ => (None, "$0".to_owned()),
        };
        let Some(bind) = path_in else {
            self.lose(
                LossKind::Repetition,
                path,
                "a `ForEach` over data that is not a data path is imported once, as static content",
            );
            let explicit = mods.take("weftEach");
            let _ = explicit;
            self.ignored(mods, path);
            return self.views(&body.body, place, path);
        };
        if !self.enter() {
            return vec![];
        }
        let explicit = mods
            .take("weftEach")
            .and_then(|m| match first_arg(&m.args) {
                Some(Expr::Str(p)) => plain_string(p),
                _ => None,
            });
        let mut name = loop_name(&item);
        while self.loops.iter().any(|l| l.name == name) {
            name.push('2');
        }
        let mut node = Node::new("each");
        node.id = Some(self.take_id(explicit, "each", &name, &format!("{path}/each")));
        let here = element_path(path, &node);
        node.props
            .insert("in".to_owned(), Value::Bind { bind, not: false });
        node.props
            .insert("as".to_owned(), Value::String(name.clone()));
        self.loops.push(Loop {
            ident: item,
            index,
            name,
        });
        let children = self.nested(|r| r.views(&body.body, place, &here));
        self.loops.pop();
        node.children = self.elements_only(children, &here);
        self.ignored(mods, &here);
        if node.children.is_empty() {
            return vec![];
        }
        vec![Child::Node(Box::new(node))]
    }
}
