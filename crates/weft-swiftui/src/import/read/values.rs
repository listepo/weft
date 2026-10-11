//! Reading expressions into Weft values, data paths and handlers.
use weft_core::{Node, Value};

use super::super::syntax::{Arg, Expr, Part, Stmt, TypeDecl, plain_string};
use super::super::{LossKind, MAX_DEPTH};
use crate::data::Leaf;
use weft_import::literal;

use super::helpers::{
    action_name, convert, first_arg, ident, implicit, is_action, is_id, is_literal, is_path_name,
    type_name,
};
use super::types::{Handler, Reader, Root, VALUE_TYPES};

impl<'a> Reader<'a> {
    pub(super) fn lose(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    /// Which stored properties hold data: an object's members are data paths, a value property
    /// is one, and a theme's members are token paths.
    pub(super) fn classify_roots(&mut self, view: &TypeDecl) {
        let mut objects = 0;
        for p in &view.properties {
            if p.body.is_some() || p.type_name.as_deref().is_some_and(|t| t.contains("->")) {
                continue;
            }
            let initial_type = match &p.initial {
                Some(Expr::Call(c)) => type_name(&c.callee),
                _ => None,
            };
            let declared = p
                .type_name
                .clone()
                .or(initial_type.clone())
                .unwrap_or_default();
            let root = if p.name.ends_with("theme")
                || p.name.ends_with("Theme")
                || declared.ends_with("Theme")
            {
                Root::Theme
            } else if p.initial.as_ref().is_some_and(is_literal)
                || VALUE_TYPES
                    .iter()
                    .any(|t| declared.trim_end_matches('?') == *t)
                || declared.starts_with('[')
                || p.attributes
                    .iter()
                    .any(|a| a == "Binding" || a == "AppStorage" || a == "SceneStorage")
            {
                Root::Value
            } else {
                objects += 1;
                // The first object is the data model; others are named parts of it.
                Root::Object(if objects == 1 {
                    String::new()
                } else {
                    p.name.clone()
                })
            };
            self.roots.insert(p.name.clone(), root);
        }
    }

    /// `path` is the element's path without its id (`…/button`); a loss names the element with
    /// the id generated for it, so it points at an element of the imported document.
    pub(super) fn take_id(
        &mut self,
        explicit: Option<String>,
        kind: &str,
        name: &str,
        path: &str,
    ) -> String {
        let note = match explicit {
            Some(id) if is_id(&id) && !self.ids.used.contains(&id) => {
                self.ids.used.insert(id.clone());
                return id;
            }
            Some(id) => {
                format!("the id {id:?} is not a valid, unique Weft id; a new one is generated")
            }
            None => "the view has no accessibilityIdentifier; the id is generated".to_owned(),
        };
        let id = self.ids.fresh(kind, name);
        self.lose(LossKind::Ids, &format!("{path}#{id}"), note);
        id
    }

    // ---- Values ----

    pub(super) fn string(&mut self, s: &str, path: &str) -> String {
        literal(&mut self.losses, path, s)
    }

    /// The Weft data path an expression reads, if it reads one.
    pub(super) fn path(&self, e: &Expr) -> Option<String> {
        let p = self.path_inner(e, 0)?;
        // `$` alone and a loop's bare item are not paths a prop can bind to the root.
        (p != "$" && p != "$.").then_some(p)
    }

    pub(super) fn path_inner(&self, e: &Expr, depth: usize) -> Option<String> {
        self.path_in(e, depth, self.aliases.len())
    }

    /// `scopes` is how many alias scopes `e` sees: an argument of an inlined view is written in
    /// the caller, so it sees only the scopes outside the view (`DeviceRow(device: device)`).
    pub(super) fn path_in(&self, e: &Expr, depth: usize, scopes: usize) -> Option<String> {
        if depth > MAX_DEPTH * 4 {
            return None;
        }
        match e {
            Expr::Ident { name, .. } => {
                for (i, scope) in self.aliases[..scopes].iter().enumerate().rev() {
                    if let Some(alias) = scope.get(name) {
                        return self.path_in(alias, depth + 1, i);
                    }
                }
                if let Some(l) = self.loops.iter().rev().find(|l| l.ident == *name) {
                    return Some(format!("${}", l.name));
                }
                let bare = name.strip_prefix('$').unwrap_or(name);
                match self.roots.get(bare)? {
                    Root::Object(prefix) if prefix.is_empty() => Some("$".to_owned()),
                    Root::Object(prefix) => Some(format!("$.{prefix}")),
                    Root::Value => is_path_name(bare).then(|| format!("$.{bare}")),
                    Root::Theme => None,
                }
            }
            // `x.isEmpty` is a truth test, not a field; a field of that name is written in backticks.
            Expr::Member {
                name, raw: false, ..
            } if name == "isEmpty" => None,
            Expr::Member { base, name, .. } => {
                if matches!(base.as_ref(), Expr::Ident { name: s, .. } if s == "self") {
                    return self.path_in(
                        &Expr::Ident {
                            name: name.clone(),
                            raw: false,
                        },
                        depth + 1,
                        scopes,
                    );
                }
                let base = self.path_in(base, depth + 1, scopes)?;
                is_path_name(name).then(|| format!("{base}.{name}"))
            }
            Expr::Subscript { base, args } => match args.as_slice() {
                [Arg { label: None, value }] => {
                    if let Some(i) = ident(value)
                        && let Some(l) = self
                            .loops
                            .iter()
                            .rev()
                            .find(|l| l.index.as_deref() == Some(i))
                    {
                        return Some(format!("${}", l.name));
                    }
                    match value {
                        Expr::Num(n) if *n >= 0.0 && n.fract() == 0.0 => {
                            let base = self.path_in(base, depth + 1, scopes)?;
                            Some(format!("{base}.{}", *n as u64))
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn token(&self, e: &Expr) -> Option<String> {
        let mut segments = vec![];
        let mut e = e;
        loop {
            match e {
                Expr::Member { base, name, .. } => {
                    segments.push(name.clone());
                    e = base;
                }
                Expr::Ident { name, .. } => {
                    if self.roots.get(name) != Some(&Root::Theme) || segments.is_empty() {
                        return None;
                    }
                    segments.reverse();
                    return Some(segments.join("."));
                }
                _ => return None,
            }
        }
    }

    pub(super) fn bind(&self, e: &Expr, not: bool) -> Option<Value> {
        Some(Value::Bind {
            bind: self.path(e)?,
            not,
        })
    }

    /// Reads a value used as text, a flag or a number, undoing the conversions `generate` adds.
    pub(super) fn value(&mut self, e: &Expr, leaf: Leaf, path: &str) -> Option<Value> {
        self.value_inner(e, leaf, path, 0)
    }

    pub(super) fn value_inner(
        &mut self,
        e: &Expr,
        leaf: Leaf,
        path: &str,
        depth: usize,
    ) -> Option<Value> {
        if depth > 16 {
            return None;
        }
        let d = depth + 1;
        let literal = match e {
            Expr::Str(parts) => match plain_string(parts) {
                Some(s) => Some(Value::String(self.string(&s, path))),
                None => return self.interpolated(parts, leaf, path),
            },
            Expr::Num(n) => Some(Value::Number(*n)),
            Expr::Neg(inner) => match inner.as_ref() {
                Expr::Num(n) => Some(Value::Number(-n)),
                _ => None,
            },
            Expr::Bool(b) => Some(Value::Bool(*b)),
            _ => None,
        };
        if let Some(v) = literal {
            return Some(convert(v, leaf));
        }
        if let Some(t) = self.token(e) {
            return Some(Value::Token(t));
        }
        if let Some(v) = self.bind(e, false) {
            return Some(v);
        }
        match e {
            Expr::Call(c) => {
                let name = ident(&c.callee).unwrap_or_default();
                let only = match c.args.as_slice() {
                    [a] => Some(&a.value),
                    _ => None,
                };
                match (name, only) {
                    (
                        "weftText" | "weftInt" | "weftDouble" | "weftStep" | "weftColumns"
                        | "weftURL" | "weftOn" | "String" | "Int" | "Double" | "Text",
                        Some(inner),
                    ) => self.value_inner(inner, leaf, path, d),
                    _ => match c.callee.as_ref() {
                        Expr::Implicit(n) if n == "constant" => {
                            only.and_then(|i| self.value_inner(i, leaf, path, d))
                        }
                        _ => None,
                    },
                }
            }
            Expr::Not(inner) => match inner.as_ref() {
                // `!x.isEmpty`: x is set.
                Expr::Member {
                    base,
                    name,
                    raw: false,
                } if name == "isEmpty" => self.bind(base, false),
                Expr::Call(c) if ident(&c.callee) == Some("weftOn") => {
                    let a = first_arg(&c.args)?;
                    self.bind(a, true)
                }
                other => match self.value_inner(other, Leaf::Bool, path, d)? {
                    Value::Bind { bind, not } => Some(Value::Bind { bind, not: !not }),
                    Value::Bool(b) => Some(Value::Bool(!b)),
                    _ => None,
                },
            },
            Expr::Member {
                base,
                name,
                raw: false,
            } if name == "isEmpty" => self.bind(base, true),
            Expr::Binary { left, op, right } if matches!(right.as_ref(), Expr::Num(n) if *n == 0.0) => {
                match op.as_str() {
                    "!=" => self.bind(left, false),
                    "==" => self.bind(left, true),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `"Hello \(name)"`: a lone interpolation is its value; mixed text keeps the literal parts.
    pub(super) fn interpolated(&mut self, parts: &[Part], leaf: Leaf, path: &str) -> Option<Value> {
        if let [Part::Interpolation(e)] = parts {
            return self.value(e, leaf, path);
        }
        let text: String = parts
            .iter()
            .filter_map(|p| match p {
                Part::Text(t) => Some(t.as_str()),
                Part::Interpolation(_) => None,
            })
            .collect();
        self.lose(
            LossKind::Bindings,
            path,
            "interpolated text is kept as its literal parts only",
        );
        let text = self.string(&text, path);
        Some(convert(Value::String(text), leaf))
    }

    pub(super) fn declares_event(&self, kind: &str, event: &str) -> bool {
        self.catalog
            .components
            .get(kind)
            .and_then(|c| c.events.as_ref())
            .is_some_and(|e| e.iter().any(|x| x == event))
    }

    pub(super) fn declares_prop(&self, kind: &str, prop: &str) -> bool {
        self.catalog
            .components
            .get(kind)
            .is_some_and(|c| c.prop(prop).is_some())
    }

    // ---- Handlers ----

    pub(super) fn handler(&mut self, statements: &[Stmt]) -> Handler {
        let mut h = Handler::default();
        for s in statements {
            let Stmt::Expr(Expr::Call(c)) = s else {
                h.other.push("a statement".to_owned());
                continue;
            };
            match ident(&c.callee) {
                Some("send") | Some("perform") => {
                    let case = implicit(c.args.first().map(|a| &a.value));
                    match case {
                        Some(case) if h.action.is_none() => {
                            let action = self
                                .actions
                                .get(case)
                                .cloned()
                                .unwrap_or_else(|| case.to_owned());
                            if is_action(&action) {
                                h.action = Some(action);
                            } else {
                                h.other.push(format!("the action {action:?}"));
                            }
                        }
                        _ => h.other.push("a second action".to_owned()),
                    }
                }
                Some("submit") => {
                    let form = match first_arg(&c.args) {
                        Some(Expr::Str(p)) => plain_string(p),
                        _ => None,
                    };
                    h.submit = Some(form.unwrap_or_default());
                }
                Some("openLink") => h.href = first_arg(&c.args).cloned(),
                _ => {
                    let name = match c.callee.as_ref() {
                        Expr::Ident { name, .. } => Some(name.clone()),
                        Expr::Member { name, .. } => Some(name.clone()),
                        _ => None,
                    };
                    match name.map(|n| action_name(&n)) {
                        Some(a) if h.action.is_none() => {
                            h.other.push("handler code".to_owned());
                            h.action = Some(a);
                        }
                        _ => h.other.push("handler code".to_owned()),
                    }
                }
            }
        }
        h
    }

    /// Adds `on-<event>` from a handler closure, noting what Weft cannot keep.
    pub(super) fn event(
        &mut self,
        node: &mut Node,
        event: &str,
        statements: &[Stmt],
        path: &str,
    ) -> Handler {
        let mut h = self.handler(statements);
        if !h.other.is_empty() {
            let what = h.other.join(", ");
            self.lose(
                LossKind::Actions,
                path,
                format!("{what} in the `{event}` handler is not kept; Weft names an action only"),
            );
            h.other.clear();
        }
        if let Some(action) = h.action.take() {
            if self.declares_event(&node.kind, event) {
                node.on.insert(event.to_owned(), action);
            } else {
                self.lose(
                    LossKind::Actions,
                    path,
                    format!(
                        "`{}` has no `{event}` event; the action {action:?} is dropped",
                        node.kind
                    ),
                );
            }
        }
        h
    }
}
