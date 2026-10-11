//! Layout containers: stacks, grids and sections.
use weft_core::{Child, Node, Value};

use super::super::LossKind;
use super::super::syntax::{Call, Expr, Stmt, plain_string};
use crate::data::Leaf;

use super::helpers::{Mod, Mods, arg, closure, first_arg, ident, implicit, spacer, strip_spacers};
use super::types::{Place, Reader};

impl<'a> Reader<'a> {
    pub(super) fn container(
        &mut self,
        kind: &str,
        call: &Call,
        mut mods: Mods,
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        if kind == "table"
            && call.args.iter().any(|a| {
                !(a.label.as_deref() == Some("alignment")
                    && implicit(Some(&a.value)) == Some("leading"))
            })
        {
            self.lose(
                LossKind::Layout,
                &here,
                "the grid's alignment and spacing are not kept",
            );
        }
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, place, &here));
        self.empty_slot(&mut node, &mut mods, &here);
        self.finish(node, mods, &here)
    }

    /// `.overlay { if cond { … } }` on a list or table is its `empty` slot.
    pub(super) fn empty_slot(&mut self, node: &mut Node, mods: &mut Mods, path: &str) {
        let Some(m) = mods.take("overlay") else {
            return;
        };
        let body = m
            .closures
            .first()
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let content = match body.as_slice() {
            [
                Stmt::If {
                    then, otherwise, ..
                },
            ] if otherwise.is_empty() => then.clone(),
            other => other.to_vec(),
        };
        let views =
            self.nested(|r| r.views(&content, Place::Nodes, &format!("{path}/slot[empty]")));
        let views = self.elements_only(views, path);
        if !views.is_empty() {
            node.slots.insert("empty".to_owned(), views);
        }
    }

    /// Slots and element-only containers hold no text runs: a run becomes a `text` element.
    pub(super) fn elements_only(&mut self, children: Vec<Child>, path: &str) -> Vec<Child> {
        children
            .into_iter()
            .filter_map(|c| match c {
                Child::Text(t) => {
                    let (mut node, _) = self.element("text", &mut Mods(vec![]), &t, path)?;
                    node.children.push(Child::Text(t));
                    Some(Child::Node(Box::new(node)))
                }
                node => Some(node),
            })
            .collect()
    }

    pub(super) fn stack(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some((mut node, here)) = self.element("stack", &mut mods, "", path) else {
            return vec![];
        };
        let row = name != "VStack";
        if row {
            node.props
                .insert("direction".to_owned(), Value::String("row".to_owned()));
        }
        let mut body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let unspaced = matches!(arg(&call.args, "spacing"), Some(Expr::Num(n)) if *n == 0.0);
        let justify = if row {
            self.justify(&mut body, unspaced)
        } else {
            None
        };
        let between = matches!(justify, Some(("space-between", _)));
        if let Some((j, gap)) = justify {
            node.props
                .insert("justify".to_owned(), Value::String(j.to_owned()));
            if let Some(g) = gap {
                node.props.insert("gap".to_owned(), Value::Token(g));
            }
        }
        for a in &call.args {
            match (a.label.as_deref(), &a.value) {
                // The gap of a `space-between` row is its spacers' `minLength`.
                (Some("spacing"), _) if between => {}
                (Some("alignment"), Expr::Implicit(v)) => {
                    let align = match (row, v.as_str()) {
                        (false, "leading") | (true, "top") => Some("start"),
                        (false, "trailing") | (true, "bottom") => Some("end"),
                        (_, "center") => Some("center"),
                        _ => None,
                    };
                    match align {
                        Some(a) => {
                            node.props
                                .insert("align".to_owned(), Value::String(a.to_owned()));
                        }
                        None => self.lose(
                            LossKind::Layout,
                            &here,
                            format!("alignment `.{v}` is not kept"),
                        ),
                    }
                }
                (Some("spacing"), e) => match self.token(e) {
                    Some(t) => {
                        node.props.insert("gap".to_owned(), Value::Token(t));
                    }
                    None => self.lose(
                        LossKind::Tokens,
                        &here,
                        "spacing that is not a theme token is not kept",
                    ),
                },
                _ => self.lose(LossKind::Layout, &here, "a stack argument is not kept"),
            }
        }
        let outer = std::mem::replace(&mut self.row_child, row);
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        self.row_child = outer;
        node.children = self.elements_only(children, &here);
        self.boxed(&mut node, &mut mods);
        self.material(&mut node, &mut mods);
        self.finish(node, mods, &here)
    }

    /// The spacers `generate` draws a row's `justify` with, taken out of its body:
    /// `Spacer(minLength: 0)` first (`end`) or at both ends (`center`); or, in an
    /// `HStack(spacing: 0)`, one spacer between every two children and before the items of a
    /// `ForEach`, the first item's behind an `if` (`space-between`), its `minLength` the gap. Any
    /// other spacer stays in the body, where it is a `layout` loss.
    pub(super) fn justify(
        &self,
        body: &mut Vec<Stmt>,
        unspaced: bool,
    ) -> Option<(&'static str, Option<String>)> {
        let is_spacer = |s: &Stmt| spacer(s).is_some();
        let edge =
            |s: Option<&Stmt>| matches!(s.and_then(spacer), Some(Some(Expr::Num(n))) if *n == 0.0);
        let count = body.iter().filter(|s| is_spacer(s)).count();
        if !unspaced {
            if count == 2 && edge(body.first()) && edge(body.last()) {
                body.pop();
                body.remove(0);
                return Some(("center", None));
            }
            if count == 1 && edge(body.first()) {
                body.remove(0);
                return Some(("end", None));
            }
            return None;
        }
        if body.first().is_some_and(is_spacer)
            || body.last().is_some_and(is_spacer)
            || body
                .windows(2)
                .any(|w| is_spacer(&w[0]) && is_spacer(&w[1]))
        {
            return None;
        }
        let mut stripped = body.clone();
        let mut found = vec![];
        strip_spacers(&mut stripped, false, &mut found);
        let first = found.first()?.clone();
        if found.iter().any(|f| *f != first) {
            return None;
        }
        let gap = match &first {
            Some(e) => Some(self.token(e)?),
            None => None,
        };
        *body = stripped;
        Some(("space-between", gap))
    }

    /// A stack's or grid's `.padding(theme.<path>)`, and the pair of frames of its `max-width`
    /// (`.frame(maxWidth: .infinity, alignment: …)` inside `.frame(maxWidth: theme.<path>)`). Any
    /// other padding or frame stays for `ignored` to report.
    pub(super) fn boxed(&self, node: &mut Node, mods: &mut Mods) {
        let token = |m: &Mod, name: &str, label: Option<&str>| match m.args.as_slice() {
            [a] if m.name == name && a.label.as_deref() == label => self.token(&a.value),
            _ => None,
        };
        let padding = mods
            .0
            .iter()
            .enumerate()
            .find_map(|(i, m)| Some((i, token(m, "padding", None)?)));
        if let Some((i, t)) = padding {
            mods.0.remove(i);
            node.props.insert("padding".to_owned(), Value::Token(t));
        }
        let fill = mods.0.iter().position(|m| {
            m.name == "frame"
                && matches!(m.args.as_slice(), [w, a]
                    if w.label.as_deref() == Some("maxWidth")
                        && implicit(Some(&w.value)) == Some("infinity")
                        && a.label.as_deref() == Some("alignment")
                        && matches!(a.value, Expr::Implicit(_)))
        });
        let cap = mods
            .0
            .iter()
            .enumerate()
            .find_map(|(i, m)| Some((i, token(m, "frame", Some("maxWidth"))?)));
        if let (Some(f), Some((c, t))) = (fill, cap)
            && f < c
        {
            mods.0.remove(c);
            mods.0.remove(f);
            node.props.insert("max-width".to_owned(), Value::Token(t));
        }
    }

    pub(super) fn grid(
        &mut self,
        name: &str,
        call: &Call,
        mut mods: Mods,
        path: &str,
    ) -> Vec<Child> {
        let Some((mut node, here)) = self.element("grid", &mut mods, "", path) else {
            return vec![];
        };
        let columns = arg(&call.args, "columns").or_else(|| arg(&call.args, "rows"));
        let count = match columns {
            // `WeftColumns(columns: n, minWidth: w, spacing: s)`, the grid that reflows.
            Some(n) if name == "WeftColumns" => {
                match arg(&call.args, "minWidth").and_then(|e| self.token(e)) {
                    Some(t) => {
                        node.props
                            .insert("min-column-width".to_owned(), Value::Token(t));
                    }
                    None => self.lose(
                        LossKind::Layout,
                        &here,
                        "a column width that is not a theme token is not kept",
                    ),
                }
                self.value(n, Leaf::Int, &here)
            }
            // `Array(repeating: GridItem(…, spacing: s), count: n)`
            Some(Expr::Call(c)) if ident(&c.callee) == Some("Array") => {
                if let Some(Expr::Call(item)) = arg(&c.args, "repeating")
                    && let Some(s) = arg(&item.args, "spacing")
                    && let Some(t) = self.token(s)
                {
                    node.props.insert("gap".to_owned(), Value::Token(t));
                }
                arg(&c.args, "count").and_then(|n| self.value(n, Leaf::Int, &here))
            }
            Some(Expr::Array(items)) => Some(Value::Number(items.len() as f64)),
            _ => None,
        };
        match count {
            Some(v) => {
                node.props.insert("columns".to_owned(), v);
            }
            None => self.lose(
                LossKind::Layout,
                &here,
                "the grid's column count is not read",
            ),
        }
        if let Some(s) = arg(&call.args, "spacing")
            && let Some(t) = self.token(s)
        {
            node.props.insert("gap".to_owned(), Value::Token(t));
        }
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        self.boxed(&mut node, &mut mods);
        self.material(&mut node, &mut mods);
        self.finish(node, mods, &here)
    }

    pub(super) fn section(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let title = first_arg(&call.args).cloned();
        let name = match &title {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element("section", &mut mods, &name, path) else {
            return vec![];
        };
        if let Some(t) = title
            && let Some(v) = self.value(&t, Leaf::Text, &here)
        {
            node.props.insert("label".to_owned(), v);
        }
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        let header = closure(&call.closures, Some("header"))
            .map(|c| c.body.clone())
            .or_else(|| arg(&call.args, "header").map(|e| vec![Stmt::Expr(e.clone())]));
        if let Some(h) = header {
            let views = self.nested(|r| r.views(&h, Place::Nodes, &format!("{here}/slot[header]")));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert("header".to_owned(), views);
            }
        }
        let footer = closure(&call.closures, Some("footer"))
            .map(|c| c.body.clone())
            .or_else(|| arg(&call.args, "footer").map(|e| vec![Stmt::Expr(e.clone())]));
        if let Some(f) = footer {
            self.lose(
                LossKind::Slots,
                &here,
                "a section footer has no slot in Weft; its content is appended",
            );
            let views = self.nested(|r| r.views(&f, Place::Nodes, &here));
            let views = self.elements_only(views, &here);
            node.children.extend(views);
        }
        self.finish(node, mods, &here)
    }
}
