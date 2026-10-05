//! JSX → the page it renders, written in the corpus HTML conventions (`data-bind`, `data-action`,
//! `<template data-each>` …), so that one reader (`dom::read_dom`) turns pages and components
//! into documents. What the conventions cannot say is noted on the element it concerns.

use std::collections::HashMap;
use std::rc::Rc;

use oxc_ast::ast::{
    Argument, ArrowFunctionBody, BinaryOperator, CallExpression, Expression, JSXAttributeItem,
    JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName, JSXExpression,
    JSXMemberExpressionObject, ObjectPropertyKind, Statement, UnaryOperator,
};
use oxc_span::GetSpan;
use weft_core::js_number;
use weft_import::{LossKind, MAX_NODES, Note, js_prefix, squash};

use super::eval::{Env, Eval, Item, MAX_STEPS, Sv, key_name, renders};
use crate::tree::{DOCUMENT, Dom, HNode};

/// The test of a condition, as far as Weft can say it.
enum Test {
    Static(bool),
    /// Shown when the path is truthy (`not`: falsy).
    Bind {
        path: String,
        not: bool,
    },
    /// Whether a data list is empty (`empty`) or not.
    Length {
        path: String,
        empty: bool,
    },
    Unknown,
}

/// Attributes whose run-time values only serve the generated components' own plumbing (focus,
/// tab wiring, busy state); nothing is lost when they are left out.
const PLUMBING: &[&str] = &[
    "id",
    "role",
    "tabindex",
    "aria-busy",
    "aria-controls",
    "aria-labelledby",
    "aria-selected",
    "aria-current",
    "aria-hidden",
];

/// React and SolidJS attribute spellings → HTML.
const RENAMED: &[(&str, &str)] = &[
    ("className", "class"),
    ("htmlFor", "for"),
    ("defaultValue", "value"),
    ("defaultChecked", "checked"),
];

/// HTML attributes → the Weft prop a binding of them sets.
const BOUND_PROPS: &[(&str, &str)] = &[("aria-label", "label"), ("aria-checked", "checked")];

/// CSS properties that take a bare number; others get `px` the way React adds it.
const UNITLESS: &[&str] = &[
    "flex",
    "flex-grow",
    "flex-shrink",
    "font-weight",
    "line-height",
    "opacity",
    "order",
    "z-index",
];

pub(crate) struct Lowered {
    pub dom: Dom,
    pub notes: HashMap<usize, Vec<Note>>,
    pub truncated: bool,
}

pub(crate) struct Lower<'a> {
    source: &'a str,
    eval: Eval,
    dom: Dom,
    notes: HashMap<usize, Vec<Note>>,
    /// Bindings per element, written as `data-bind` once the element is complete.
    binds: HashMap<usize, Vec<(String, String)>>,
    /// The data path a radio button inside the current group compares against.
    radio: Option<String>,
    /// The data path the tabs inside the current tablist choose their selected tab by.
    chosen: Option<String>,
    truncated: bool,
}

/// The page `root` renders.
pub(crate) fn lower<'a>(source: &'a str, root: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Lowered {
    let mut l = Lower {
        source,
        eval: Eval { steps: 0 },
        dom: Dom {
            nodes: vec![HNode::Element {
                name: String::new(),
                attrs: Vec::new(),
                children: Vec::new(),
            }],
        },
        notes: HashMap::new(),
        binds: HashMap::new(),
        radio: None,
        chosen: None,
        truncated: false,
    };
    let mut top = Vec::new();
    l.expression(root, env, DOCUMENT, &mut top);
    l.set_children(DOCUMENT, top);
    l.finish()
}

fn snippet(source: &str, e: &impl GetSpan) -> String {
    let span = e.span();
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or("");
    let text = squash(text);
    let short = js_prefix(&text, 60);
    if short.len() < text.len() {
        format!("{short}…")
    } else {
        text
    }
}

fn kebab(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// React's rule for JSX text: lines are trimmed where they meet a line break, lines left empty
/// are dropped, and the rest are joined with one space.
pub(crate) fn jsx_text(raw: &str) -> String {
    let lines: Vec<&str> = raw
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let last_filled = lines
        .iter()
        .rposition(|l| l.chars().any(|c| c != ' ' && c != '\t'));
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let line = line.replace('\t', " ");
        let mut t = line.as_str();
        if i != 0 {
            t = t.trim_start_matches(' ');
        }
        if i + 1 != lines.len() {
            t = t.trim_end_matches(' ');
        }
        if !t.is_empty() {
            out.push_str(t);
            if Some(i) != last_filled {
                out.push(' ');
            }
        }
    }
    decode_entities(&out)
}

/// The character references JSX text and attribute strings may use; others are kept as written.
pub(crate) fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let end = tail.find(';').filter(|&e| e <= 10);
        let decoded = end.and_then(|e| {
            let name = &tail[1..e];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                _ => name
                    .strip_prefix("#x")
                    .or_else(|| name.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| name.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, e))
        });
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &tail[e + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

impl<'a> Lower<'a> {
    fn note(&mut self, el: usize, kind: LossKind, note: String) {
        self.notes.entry(el).or_default().push(Note { kind, note });
    }

    fn full(&mut self) -> bool {
        if self.dom.nodes.len() > MAX_NODES * 2 || self.eval.steps > MAX_STEPS {
            self.truncated = true;
        }
        self.truncated
    }

    fn element(&mut self, name: &str, attrs: Vec<(String, String)>) -> usize {
        self.dom.nodes.push(HNode::Element {
            name: name.to_owned(),
            attrs,
            children: Vec::new(),
        });
        self.dom.nodes.len() - 1
    }

    fn text(&mut self, text: String, out: &mut Vec<usize>) {
        // Adjacent runs are one text node, as in a parsed page.
        if let Some(&last) = out.last()
            && let Some(HNode::Text(t)) = self.dom.nodes.get_mut(last)
        {
            t.push_str(&text);
            return;
        }
        self.dom.nodes.push(HNode::Text(text));
        out.push(self.dom.nodes.len() - 1);
    }

    fn set_children(&mut self, el: usize, ids: Vec<usize>) {
        if let Some(HNode::Element { children, .. }) = self.dom.nodes.get_mut(el) {
            *children = ids;
        }
    }

    fn set_attr(&mut self, el: usize, key: &str, value: String) {
        if let Some(HNode::Element { attrs, .. }) = self.dom.nodes.get_mut(el) {
            match attrs.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => attrs.push((key.to_owned(), value)),
            }
        }
    }

    fn bind(&mut self, el: usize, prop: &str, value: String) {
        if el == DOCUMENT {
            return;
        }
        if self
            .binds
            .get(&el)
            .is_some_and(|b| b.iter().any(|(p, _)| p == prop))
        {
            let note = format!("{prop} is bound twice; the first binding is kept");
            self.note(el, LossKind::Bindings, note);
        } else {
            self.binds
                .entry(el)
                .or_default()
                .push((prop.to_owned(), value));
        }
    }

    fn finish(mut self) -> Lowered {
        let binds = std::mem::take(&mut self.binds);
        for (el, list) in binds {
            let text: Vec<String> = list.iter().map(|(p, v)| format!("{p}:{v}")).collect();
            self.set_attr(el, "data-bind", text.join("; "));
        }
        Lowered {
            dom: self.dom,
            notes: self.notes,
            truncated: self.truncated,
        }
    }

    /// Places what an expression renders into `out`, children of `parent`.
    fn expression(
        &mut self,
        e: &'a Expression<'a>,
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        if self.full() {
            return;
        }
        match e.get_inner_expression() {
            Expression::JSXElement(el) => self.jsx_element(el, env, parent, out),
            Expression::JSXFragment(f) => self.children(&f.children, env, parent, out),
            Expression::ConditionalExpression(c)
                if renders(&c.consequent) || renders(&c.alternate) =>
            {
                let test = self.test(&c.test, env);
                let what = snippet(self.source, &c.test);
                self.conditional(
                    test,
                    &what,
                    Some(&c.consequent),
                    Some(&c.alternate),
                    env,
                    parent,
                    out,
                );
            }
            Expression::LogicalExpression(l)
                if l.operator == oxc_ast::ast::LogicalOperator::And && renders(&l.right) =>
            {
                let test = self.test(&l.left, env);
                let what = snippet(self.source, &l.left);
                self.conditional(test, &what, Some(&l.right), None, env, parent, out);
            }
            other => {
                let value = self.eval.eval(other, env);
                self.value(value, other, parent, out);
            }
        }
    }

    /// Places a value: text, a binding of the parent's text, markup, or a repetition.
    fn value(&mut self, value: Sv<'a>, e: &'a Expression<'a>, parent: usize, out: &mut Vec<usize>) {
        match value {
            Sv::Str(_) | Sv::Num(_) => {
                if let Some(t) = value.text() {
                    self.text(t, out);
                }
            }
            Sv::Undef | Sv::Bool(_) => {}
            Sv::Path { path, not } => {
                if parent == DOCUMENT {
                    let note = "the component renders only bound text".to_owned();
                    self.note(DOCUMENT, LossKind::Text, note);
                } else {
                    let path = if not { format!("!{path}") } else { path };
                    self.bind(parent, "text", path);
                }
            }
            Sv::Jsx(x, scope) => self.expression(x, &scope, parent, out),
            Sv::Arr(items) => self.items(&items, e, parent, out),
            _ => {
                let note = format!(
                    "{{{}}} is not a data path or markup Weft can read; it is left out",
                    snippet(self.source, e)
                );
                self.note(parent, LossKind::Bindings, note);
            }
        }
    }

    fn items(
        &mut self,
        items: &[Item<'a>],
        e: &'a Expression<'a>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        for item in items {
            if self.full() {
                return;
            }
            match item {
                Item::One(v) => self.value(v.clone(), e, parent, out),
                // A repetition of nothing renders nothing (generated tables spread one into
                // their header row).
                Item::Repeat(r) if r.items.is_empty() => {}
                Item::Repeat(r) => {
                    let mut attrs = vec![("data-each".to_owned(), r.list.clone())];
                    attrs.push(("data-as".to_owned(), r.var.clone()));
                    let t = self.element("template", attrs);
                    let mut inner = Vec::new();
                    self.items(&r.items, e, t, &mut inner);
                    self.set_children(t, inner);
                    out.push(t);
                }
            }
        }
    }

    /// What a condition tests.
    fn test(&mut self, e: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Test {
        let e = e.get_inner_expression();
        match e {
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                match self.test(&u.argument, env) {
                    Test::Static(b) => Test::Static(!b),
                    Test::Bind { path, not } => Test::Bind { path, not: !not },
                    Test::Length { path, empty } => Test::Length {
                        path,
                        empty: !empty,
                    },
                    Test::Unknown => Test::Unknown,
                }
            }
            Expression::StaticMemberExpression(m) if m.property.name == "length" => {
                match self.eval.eval(&m.object, env) {
                    Sv::Path { path, not: false } => Test::Length { path, empty: false },
                    other => other.truthy().map_or(Test::Unknown, |_| {
                        Test::Static(matches!(other, Sv::Arr(ref a) if !a.is_empty()))
                    }),
                }
            }
            Expression::BinaryExpression(b) => {
                let (operand, literal, flipped) = match (&b.left, &b.right) {
                    (x, Expression::NumericLiteral(n)) => (x, Sv::Num(n.value), false),
                    (Expression::NumericLiteral(n), x) => (x, Sv::Num(n.value), true),
                    (x, Expression::StringLiteral(s)) => (x, Sv::Str(s.value.to_string()), false),
                    (Expression::StringLiteral(s), x) => (x, Sv::Str(s.value.to_string()), true),
                    _ => return self.truthiness(e, env),
                };
                let op = b.operator;
                // `list.length > 0` and its spellings; `text === ""` and `text !== ""`.
                let positive = match (&literal, op, flipped) {
                    (Sv::Num(n), BinaryOperator::GreaterThan, false) if *n == 0.0 => Some(true),
                    (Sv::Num(n), BinaryOperator::LessThan, true) if *n == 0.0 => Some(true),
                    (Sv::Num(n), BinaryOperator::GreaterEqualThan, false) if *n == 1.0 => {
                        Some(true)
                    }
                    (
                        Sv::Num(n),
                        BinaryOperator::StrictInequality | BinaryOperator::Inequality,
                        _,
                    ) if *n == 0.0 => Some(true),
                    (Sv::Num(n), BinaryOperator::StrictEquality | BinaryOperator::Equality, _)
                        if *n == 0.0 =>
                    {
                        Some(false)
                    }
                    (Sv::Num(n), BinaryOperator::LessThan, false) if *n == 1.0 => Some(false),
                    (
                        Sv::Str(s),
                        BinaryOperator::StrictInequality | BinaryOperator::Inequality,
                        _,
                    ) if s.is_empty() => Some(true),
                    (Sv::Str(s), BinaryOperator::StrictEquality | BinaryOperator::Equality, _)
                        if s.is_empty() =>
                    {
                        Some(false)
                    }
                    _ => None,
                };
                let Some(positive) = positive else {
                    return self.truthiness(e, env);
                };
                match self.test(operand, env) {
                    Test::Length { path, empty } => Test::Length {
                        path,
                        empty: empty == positive,
                    },
                    Test::Bind { path, not } => Test::Bind {
                        path,
                        not: not == positive,
                    },
                    Test::Static(b) => Test::Static(b == positive),
                    Test::Unknown => Test::Unknown,
                }
            }
            _ => self.truthiness(e, env),
        }
    }

    fn truthiness(&mut self, e: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Test {
        match self.eval.eval(e, env) {
            Sv::Path { path, not } => Test::Bind { path, not },
            v => v.truthy().map_or(Test::Unknown, Test::Static),
        }
    }

    /// Content shown under a condition.
    #[allow(clippy::too_many_arguments)]
    fn conditional(
        &mut self,
        test: Test,
        what: &str,
        yes: Option<&'a Expression<'a>>,
        no: Option<&'a Expression<'a>>,
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        match test {
            Test::Static(b) => {
                if let Some(x) = if b { yes } else { no } {
                    self.expression(x, env, parent, out);
                }
            }
            Test::Length { path, empty } => {
                let (full, none) = if empty { (no, yes) } else { (yes, no) };
                let mut shown = Vec::new();
                if let Some(x) = full {
                    self.expression(x, env, parent, &mut shown);
                }
                let mut other = Vec::new();
                if let Some(x) = none {
                    self.expression(x, env, parent, &mut other);
                }
                self.length_split(&path, shown, other, what, parent, out);
            }
            Test::Bind { path, not } => {
                for (branch, shown) in [(yes, !not), (no, not)] {
                    let Some(x) = branch else {
                        continue;
                    };
                    let mut placed = Vec::new();
                    self.expression(x, env, parent, &mut placed);
                    let hidden = if shown {
                        format!("!{path}")
                    } else {
                        path.clone()
                    };
                    self.shown_when(&placed, &hidden, what, parent);
                    out.extend(placed);
                }
            }
            Test::Unknown => {
                let note = format!(
                    "what is shown depends on {what}, which is not a data path; every branch is kept"
                );
                self.note(parent, LossKind::Hidden, note);
                for x in [yes, no].into_iter().flatten() {
                    self.expression(x, env, parent, out);
                }
            }
        }
    }

    /// Content for a list with items (`full`) and without (`none`). The `none` content is the
    /// list's empty slot when the list's repetition is beside it or inside `full`; otherwise both
    /// are shown by the list's length.
    fn length_split(
        &mut self,
        path: &str,
        full: Vec<usize>,
        none: Vec<usize>,
        what: &str,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        let repeats = |dom: &Dom, id: usize| dom.attr(id, "data-each") == Some(path);
        let here = out.iter().chain(&full).any(|&id| repeats(&self.dom, id));
        let host = full
            .iter()
            .copied()
            .find(|&id| self.dom.elements(id).any(|c| repeats(&self.dom, c)));
        if none.is_empty() || (!here && host.is_none()) {
            self.shown_when(&full, &format!("!{path}.length"), what, parent);
            out.extend(full);
            self.shown_when(&none, &format!("{path}.length"), what, parent);
            out.extend(none);
            return;
        }
        out.extend(full);
        let t = self.element("template", vec![("data-empty".into(), String::new())]);
        self.set_children(t, none);
        match host.and_then(|h| self.dom.nodes.get_mut(h)) {
            Some(HNode::Element { children, .. }) => children.push(t),
            _ => out.push(t),
        }
    }

    /// Binds what decides whether `placed` shows: `hidden` for elements, `open` (its opposite)
    /// for a dialog, which a condition shows by opening it.
    fn shown_when(&mut self, placed: &[usize], hidden: &str, what: &str, parent: usize) {
        for &id in placed {
            match self.dom.name(id) {
                Some("dialog") => {
                    let open = hidden
                        .strip_prefix('!')
                        .map_or_else(|| format!("!{hidden}"), str::to_owned);
                    self.bind(id, "open", open);
                }
                Some(_) => self.bind(id, "hidden", hidden.to_owned()),
                None => {
                    let note = format!("text shown only when {what} is always shown");
                    self.note(parent, LossKind::Hidden, note);
                }
            }
        }
    }

    fn children(
        &mut self,
        children: &'a [JSXChild<'a>],
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        for child in children {
            if self.full() {
                return;
            }
            match child {
                JSXChild::Text(t) => {
                    let text = jsx_text(t.value.as_str());
                    if !text.is_empty() {
                        self.text(text, out);
                    }
                }
                JSXChild::Element(el) => self.jsx_element(el, env, parent, out),
                JSXChild::Fragment(f) => self.children(&f.children, env, parent, out),
                JSXChild::ExpressionContainer(c) => match &c.expression {
                    JSXExpression::EmptyExpression(_) => {}
                    other => {
                        if let Some(x) = other.as_expression() {
                            self.expression(x, env, parent, out);
                        }
                    }
                },
                JSXChild::Spread(s) => {
                    let note = format!(
                        "spread children {{...{}}} are left out",
                        snippet(self.source, &s.expression)
                    );
                    self.note(parent, LossKind::Structure, note);
                }
            }
        }
    }

    fn jsx_element(
        &mut self,
        el: &'a JSXElement<'a>,
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        let opening = &el.opening_element;
        let name = match &opening.name {
            JSXElementName::Identifier(id) => id.name.as_str().to_owned(),
            JSXElementName::NamespacedName(n) => {
                format!("{}:{}", n.namespace.name.as_str(), n.name.name.as_str())
            }
            JSXElementName::IdentifierReference(id) => {
                return self.component(id.name.as_str(), el, env, parent, out);
            }
            JSXElementName::MemberExpression(m) => {
                let object = match &m.object {
                    JSXMemberExpressionObject::IdentifierReference(id) => id.name.as_str(),
                    _ => "",
                };
                let name = format!("{object}.{}", m.property.name.as_str());
                return self.component(&name, el, env, parent, out);
            }
            JSXElementName::ThisExpression(_) => {
                return self.component("this", el, env, parent, out);
            }
        };
        let node = self.element(&name.to_ascii_lowercase(), Vec::new());
        out.push(node);
        let attrs = self.attributes(node, &name, &opening.attributes, env);
        if let Some(HNode::Element { attrs: slot, .. }) = self.dom.nodes.get_mut(node) {
            *slot = attrs;
        }
        let outer = self.radio.take();
        let outer_chosen = self.chosen.take();
        let mut inner = Vec::new();
        self.children(&el.children, env, node, &mut inner);
        // In JSX, text written beside a bound value is shown with it; Weft binds the whole text.
        let bound = self
            .binds
            .get(&node)
            .is_some_and(|b| b.iter().any(|(p, _)| p == "text"));
        let literal = inner.iter().any(
            |&id| matches!(self.dom.nodes.get(id), Some(HNode::Text(t)) if !t.trim().is_empty()),
        );
        if bound && literal {
            let note = "the text written beside the bound value is left out".to_owned();
            self.note(node, LossKind::Text, note);
        }
        self.set_children(node, inner);
        let radio = self.radio.take();
        let group = self.dom.attr(node, "role") == Some("radiogroup") || name == "fieldset";
        match radio {
            Some(path) if group => {
                if !self
                    .binds
                    .get(&node)
                    .is_some_and(|b| b.iter().any(|(p, _)| p == "value"))
                {
                    self.bind(node, "value", path);
                }
                self.radio = outer;
            }
            radio => self.radio = outer.or(radio),
        }
        let chosen = self.chosen.take();
        match chosen {
            Some(path) if self.dom.attr(node, "role") == Some("tablist") => {
                self.bind(node, "selected", path);
                self.chosen = outer_chosen;
            }
            chosen => self.chosen = outer_chosen.or(chosen),
        }
    }

    /// An element that is not HTML: SolidJS control flow, fragments, or a component of the app.
    fn component(
        &mut self,
        name: &str,
        el: &'a JSXElement<'a>,
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        let attr = |key: &str| {
            el.opening_element.attributes.iter().find_map(|a| match a {
                JSXAttributeItem::Attribute(a) => match (&a.name, &a.value) {
                    (
                        JSXAttributeName::Identifier(n),
                        Some(JSXAttributeValue::ExpressionContainer(c)),
                    ) if n.name == key => c.expression.as_expression(),
                    _ => None,
                },
                JSXAttributeItem::SpreadAttribute(_) => None,
            })
        };
        match name {
            "Fragment" | "React.Fragment" | "Portal" => {
                self.children(&el.children, env, parent, out);
            }
            "For" | "Index" => {
                let list = attr("each").map_or(Sv::Unknown, |x| self.eval.eval(x, env));
                let callback = el.children.iter().find_map(|c| match c {
                    JSXChild::ExpressionContainer(c) => match c.expression.as_expression() {
                        Some(Expression::ArrowFunctionExpression(f)) => Some(f),
                        _ => None,
                    },
                    _ => None,
                });
                match callback {
                    Some(f) => {
                        let value = self.eval.map(list, &f.params, &f.body, false, env);
                        let snippet_of = attr("each");
                        match (value, snippet_of) {
                            (Sv::Arr(items), Some(x)) => self.items(&items, x, parent, out),
                            _ => {
                                let note =
                                    format!("<{name}> over a list Weft cannot read is left out");
                                self.note(parent, LossKind::Repetition, note);
                            }
                        }
                    }
                    None => {
                        let note = format!("<{name}> without a callback is left out");
                        self.note(parent, LossKind::Repetition, note);
                    }
                }
            }
            "Show" => {
                let Some(when) = attr("when") else {
                    self.children(&el.children, env, parent, out);
                    return;
                };
                let test = self.test(when, env);
                let what = snippet(self.source, when);
                // The children are shown when the test holds; they are JSX, not one expression,
                // so they are placed through a fragment of their own.
                let fallback = attr("fallback");
                match test {
                    Test::Static(true) => self.children(&el.children, env, parent, out),
                    Test::Static(false) => {
                        if let Some(x) = fallback {
                            self.expression(x, env, parent, out);
                        }
                    }
                    test => {
                        let mut shown = Vec::new();
                        self.children(&el.children, env, parent, &mut shown);
                        self.condition_placed(test, &what, shown, fallback, env, parent, out);
                    }
                }
            }
            _ => {
                let note = format!(
                    "<{name}> is a component, not an element Weft knows; only its children are kept"
                );
                self.note(parent, LossKind::Kinds, note);
                self.children(&el.children, env, parent, out);
            }
        }
    }

    /// `conditional` for content already placed (SolidJS `<Show>` children).
    #[allow(clippy::too_many_arguments)]
    fn condition_placed(
        &mut self,
        test: Test,
        what: &str,
        shown: Vec<usize>,
        fallback: Option<&'a Expression<'a>>,
        env: &Rc<Env<'a>>,
        parent: usize,
        out: &mut Vec<usize>,
    ) {
        match test {
            Test::Length { path, empty } => {
                let mut other = Vec::new();
                if let Some(x) = fallback {
                    self.expression(x, env, parent, &mut other);
                }
                let (full, none) = if empty {
                    (other, shown)
                } else {
                    (shown, other)
                };
                self.length_split(&path, full, none, what, parent, out);
            }
            Test::Bind { path, not } => {
                let hidden = if not {
                    path.clone()
                } else {
                    format!("!{path}")
                };
                self.shown_when(&shown, &hidden, what, parent);
                out.extend(shown);
                if let Some(x) = fallback {
                    let mut other = Vec::new();
                    self.expression(x, env, parent, &mut other);
                    let hidden = if not { format!("!{path}") } else { path };
                    self.shown_when(&other, &hidden, what, parent);
                    out.extend(other);
                }
            }
            Test::Static(_) | Test::Unknown => {
                let note = format!(
                    "what is shown depends on {what}, which is not a data path; every branch is kept"
                );
                self.note(parent, LossKind::Hidden, note);
                out.extend(shown);
                if let Some(x) = fallback {
                    self.expression(x, env, parent, out);
                }
            }
        }
    }

    /// The HTML attributes of an element; bindings go to `binds`, handlers to `data-action`.
    fn attributes(
        &mut self,
        node: usize,
        tag: &str,
        items: &'a [JSXAttributeItem<'a>],
        env: &Rc<Env<'a>>,
    ) -> Vec<(String, String)> {
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut handlers = Vec::new();
        for item in items {
            let a = match item {
                JSXAttributeItem::Attribute(a) => a,
                JSXAttributeItem::SpreadAttribute(s) => {
                    let note = format!(
                        "spread attributes {{...{}}} are left out",
                        snippet(self.source, &s.argument)
                    );
                    self.note(node, LossKind::Props, note);
                    continue;
                }
            };
            let raw = match &a.name {
                JSXAttributeName::Identifier(n) => n.name.as_str().to_owned(),
                JSXAttributeName::NamespacedName(n) => {
                    format!("{}:{}", n.namespace.name.as_str(), n.name.name.as_str())
                }
            };
            if matches!(raw.as_str(), "key" | "ref" | "children") {
                continue;
            }
            let expression = match &a.value {
                Some(JSXAttributeValue::ExpressionContainer(c)) => c.expression.as_expression(),
                _ => None,
            };
            if raw.len() > 2
                && raw.starts_with("on")
                && raw[2..].starts_with(|c: char| c.is_ascii_uppercase())
            {
                if let Some(x) = expression {
                    handlers.push((raw, x));
                }
                continue;
            }
            let name = RENAMED
                .iter()
                .find(|(r, _)| *r == raw)
                .map_or_else(|| html_name(&raw), |(_, h)| (*h).to_owned());
            let value = match &a.value {
                None => Sv::Bool(true),
                Some(JSXAttributeValue::StringLiteral(s)) => {
                    Sv::Str(decode_entities(s.value.as_str()))
                }
                Some(JSXAttributeValue::ExpressionContainer(_)) => match expression {
                    Some(x) if name == "style" => {
                        if let Some(css) = self.style(node, x, env) {
                            attrs.push((name, css));
                        }
                        continue;
                    }
                    // SolidJS components repeat the select's value on each option for server
                    // rendering; the select's own value is what the document keeps.
                    Some(_) if name == "selected" && tag == "option" => continue,
                    Some(x) if name == "checked" && is_radio(items) => {
                        if let Some(path) = self.radio_path(x, env) {
                            self.radio = Some(path);
                        }
                        continue;
                    }
                    Some(x) => self.eval.eval(x, env),
                    None => Sv::Undef,
                },
                Some(_) => {
                    let note = format!("{raw} holds markup; it is left out");
                    self.note(node, LossKind::Props, note);
                    continue;
                }
            };
            match value {
                Sv::Str(_) | Sv::Num(_) => {
                    if let Some(t) = value.text() {
                        attrs.push((name, t));
                    }
                }
                Sv::Bool(b) if name.starts_with("aria-") || name.starts_with("data-") => {
                    attrs.push((name, b.to_string()));
                }
                Sv::Bool(true) => attrs.push((name, String::new())),
                Sv::Bool(false) | Sv::Undef => {}
                // Which tab is selected is the tablist's binding, not an attribute of the tab.
                Sv::Chosen(path) if name == "aria-selected" => self.chosen = Some(path),
                Sv::Prefix(p) if name == "data-weft-id" => {
                    // `"row[" + index + "]"`: the id of a repeated element.
                    let base = p.strip_suffix('[').unwrap_or(&p).to_owned();
                    attrs.push((name, base));
                }
                Sv::Path { path, not } => {
                    let prop = BOUND_PROPS
                        .iter()
                        .find(|(h, _)| *h == name)
                        .map_or(name.as_str(), |(_, p)| p);
                    let path = if not { format!("!{path}") } else { path };
                    self.bind(node, prop, path);
                }
                _ if PLUMBING.contains(&name.as_str()) => {}
                _ => {
                    let what = expression.map_or_else(String::new, |x| snippet(self.source, x));
                    let note =
                        format!("{raw}={{{what}}} is not a literal or a data path; it is left out");
                    self.note(node, LossKind::Props, note);
                }
            }
        }
        let own = attrs
            .iter()
            .find(|(k, _)| k == "data-weft-id")
            .map(|(_, v)| v.clone());
        let mut actions: Vec<(String, String)> = Vec::new();
        for (raw, x) in handlers {
            let event = match raw.as_str() {
                "onClick" | "onPress" | "onDblClick" | "onDoubleClick" => "press",
                "onSubmit" => "submit",
                "onChange" | "onInput" => "change",
                "onClose" | "onCancel" => "close",
                "onKeyDown" | "onKeyUp" | "onKeyPress" if tag == "dialog" => "close",
                "onKeyDown" | "onKeyUp" | "onKeyPress" => "press",
                _ => "",
            };
            let found = self.handler(x, env, own.as_deref());
            if event.is_empty() {
                if !found.actions.is_empty() {
                    let note = format!("{raw} has no Weft event; its action is left out");
                    self.note(node, LossKind::Actions, note);
                }
                continue;
            }
            for action in found.actions {
                match actions.iter().find(|(e, _)| e == event) {
                    Some((_, a)) if *a == action => {}
                    Some(_) => {
                        let note =
                            format!("{raw} fires more than one action; only the first is kept");
                        self.note(node, LossKind::Actions, note);
                    }
                    None => actions.push((event.to_owned(), action)),
                }
            }
            if !found.understood {
                let note = format!(
                    "{raw}={{{}}} calls no action Weft can read; it is left out",
                    snippet(self.source, x)
                );
                self.note(node, LossKind::Actions, note);
            }
        }
        if !actions.is_empty() {
            let text: Vec<String> = actions.iter().map(|(e, a)| format!("{e}:{a}")).collect();
            attrs.push(("data-action".into(), text.join("; ")));
        }
        attrs
    }

    /// `style={{ … }}` as CSS text.
    fn style(&mut self, node: usize, x: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Option<String> {
        if let Expression::ObjectExpression(o) = x.get_inner_expression() {
            let mut css = Vec::new();
            for p in &o.properties {
                let ObjectPropertyKind::ObjectProperty(p) = p else {
                    continue;
                };
                let Some(key) = key_name(&p.key) else {
                    continue;
                };
                let property = if key.starts_with("--") {
                    key
                } else {
                    kebab(&key)
                };
                match self.eval.eval(&p.value, env) {
                    Sv::Str(s) => css.push(format!("{property}: {s}")),
                    Sv::Num(n) if UNITLESS.contains(&property.as_str()) || n == 0.0 => {
                        css.push(format!("{property}: {}", js_number(n)));
                    }
                    Sv::Num(n) => css.push(format!("{property}: {}px", js_number(n))),
                    Sv::Undef => {}
                    _ => {
                        let note = format!("style {property} is not a literal; it is left out");
                        self.note(node, LossKind::Layout, note);
                    }
                }
            }
            return Some(css.join("; "));
        }
        match self.eval.eval(x, env) {
            Sv::Str(s) => Some(s),
            _ => {
                let note = "style is not a literal; it is left out".to_owned();
                self.note(node, LossKind::Layout, note);
                None
            }
        }
    }

    /// The data path a radio button's `checked` compares with its value
    /// (`data.plan === "pro"`, or the generated `_text(data?.plan) !== "" && "pro" === …`).
    fn radio_path(&mut self, x: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Option<String> {
        let mut pending = vec![x];
        while let Some(x) = pending.pop() {
            match x.get_inner_expression() {
                Expression::LogicalExpression(l) => {
                    pending.push(&l.left);
                    pending.push(&l.right);
                }
                Expression::BinaryExpression(b)
                    if matches!(
                        b.operator,
                        BinaryOperator::StrictEquality | BinaryOperator::Equality
                    ) =>
                {
                    let sides = [self.eval.eval(&b.left, env), self.eval.eval(&b.right, env)];
                    if let [Sv::Path { path, not: false }, Sv::Str(_)]
                    | [Sv::Str(_), Sv::Path { path, not: false }] = sides
                    {
                        return Some(path);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// The actions a handler fires. Generated components call `_act(actions, {id, action})`;
    /// hand-written ones call `actions.todo.add()` or pass `actions.todo.add` itself. A call
    /// whose `id` names another element (a submit button firing its form's action) belongs to
    /// that element.
    fn handler(&mut self, x: &'a Expression<'a>, env: &Rc<Env<'a>>, own: Option<&str>) -> Handler {
        let mut found = Handler::default();
        match self.eval.eval(x, env) {
            Sv::Actions(name) if !name.is_empty() => {
                found.actions.push(name);
                found.understood = true;
                return found;
            }
            Sv::Write => {
                found.understood = true;
                return found;
            }
            _ => {}
        }
        let mut pending: Vec<Walk<'a>> = vec![Walk::Expression(x)];
        let mut seen = 0usize;
        while let Some(w) = pending.pop() {
            seen += 1;
            if seen > 10_000 {
                break;
            }
            match w {
                Walk::Statement(s) => match s {
                    Statement::ExpressionStatement(s) => {
                        pending.push(Walk::Expression(&s.expression))
                    }
                    Statement::ReturnStatement(r) => {
                        if let Some(a) = &r.argument {
                            pending.push(Walk::Expression(a));
                        }
                    }
                    Statement::IfStatement(i) => {
                        pending.push(Walk::Statement(&i.consequent));
                        if let Some(a) = &i.alternate {
                            pending.push(Walk::Statement(a));
                        }
                    }
                    Statement::BlockStatement(b) => {
                        pending.extend(b.body.iter().map(Walk::Statement));
                    }
                    Statement::VariableDeclaration(d) => {
                        for decl in &d.declarations {
                            if let Some(init) = &decl.init {
                                pending.push(Walk::Expression(init));
                            }
                        }
                    }
                    _ => {}
                },
                Walk::Expression(e) => match e.get_inner_expression() {
                    Expression::ArrowFunctionExpression(f) => match &f.body {
                        ArrowFunctionBody::FunctionBody(b) => {
                            pending.extend(b.statements.iter().map(Walk::Statement));
                        }
                        other => {
                            if let Some(x) = other.as_expression() {
                                pending.push(Walk::Expression(x));
                            }
                        }
                    },
                    Expression::FunctionExpression(f) => {
                        if let Some(b) = &f.body {
                            pending.extend(b.statements.iter().map(Walk::Statement));
                        }
                    }
                    Expression::CallExpression(c) => {
                        self.call_in_handler(c, env, own, &mut found);
                        pending.extend(
                            c.arguments
                                .iter()
                                .filter_map(Argument::as_expression)
                                .map(Walk::Expression),
                        );
                    }
                    Expression::ChainExpression(c) => {
                        if let oxc_ast::ast::ChainElement::CallExpression(call) = &c.expression {
                            self.call_in_handler(call, env, own, &mut found);
                        }
                    }
                    Expression::SequenceExpression(s) => {
                        pending.extend(s.expressions.iter().map(Walk::Expression));
                    }
                    Expression::ConditionalExpression(c) => {
                        pending.push(Walk::Expression(&c.consequent));
                        pending.push(Walk::Expression(&c.alternate));
                    }
                    Expression::LogicalExpression(l) => {
                        pending.push(Walk::Expression(&l.left));
                        pending.push(Walk::Expression(&l.right));
                    }
                    Expression::AwaitExpression(a) => pending.push(Walk::Expression(&a.argument)),
                    _ => {}
                },
            }
        }
        found.actions.reverse();
        found
    }

    fn call_in_handler(
        &mut self,
        c: &'a CallExpression<'a>,
        env: &Rc<Env<'a>>,
        own: Option<&str>,
        found: &mut Handler,
    ) {
        let callee = c.callee.get_inner_expression();
        if let Expression::Identifier(id) = callee {
            let name = id.name.as_str();
            if name == "_act" {
                found.understood = true;
                let Some(Expression::ObjectExpression(o)) = c
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .map(Expression::get_inner_expression)
                else {
                    return;
                };
                let mut action = None;
                let mut target = None;
                for p in &o.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = p {
                        match key_name(&p.key).as_deref() {
                            Some("action") => action = self.eval.eval(&p.value, env).text(),
                            Some("id") => {
                                target = match self.eval.eval(&p.value, env) {
                                    Sv::Str(s) => Some(s),
                                    Sv::Prefix(p) => {
                                        Some(p.strip_suffix('[').unwrap_or(&p).to_owned())
                                    }
                                    _ => None,
                                }
                            }
                            _ => {}
                        }
                    }
                }
                let foreign = matches!((own, &target), (Some(o), Some(t)) if o != t);
                if let Some(a) = action.filter(|_| !foreign) {
                    found.actions.push(a);
                }
                return;
            }
            // The generated components' own helpers (`_press`, `_choose`, `_tabKey`) only route
            // events; the actions they fire are in their arguments.
            if name.starts_with('_') {
                found.understood = true;
                return;
            }
        }
        match self.eval.eval(callee, env) {
            Sv::Actions(name) if !name.is_empty() => {
                found.actions.push(name);
                found.understood = true;
            }
            Sv::Write => found.understood = true,
            _ => {
                // `e.preventDefault()` and the like.
                if let Expression::StaticMemberExpression(m) = callee
                    && matches!(
                        m.property.name.as_str(),
                        "preventDefault" | "stopPropagation" | "focus" | "blur"
                    )
                {
                    found.understood = true;
                }
            }
        }
    }
}

#[derive(Default)]
struct Handler {
    actions: Vec<String>,
    /// The handler does something Weft knows (fires an action, writes a binding, routes events).
    understood: bool,
}

enum Walk<'a> {
    Expression(&'a Expression<'a>),
    Statement(&'a Statement<'a>),
}

fn html_name(raw: &str) -> String {
    if raw.starts_with("aria-") || raw.starts_with("data-") || raw.contains(':') {
        raw.to_owned()
    } else {
        raw.to_ascii_lowercase()
    }
}

fn is_radio(items: &[JSXAttributeItem<'_>]) -> bool {
    items.iter().any(|a| match a {
        JSXAttributeItem::Attribute(a) => {
            matches!(&a.name, JSXAttributeName::Identifier(n) if n.name == "type")
                && matches!(&a.value, Some(JSXAttributeValue::StringLiteral(s)) if s.value == "radio")
        }
        JSXAttributeItem::SpreadAttribute(_) => false,
    })
}

#[cfg(test)]
mod tests {
    use super::{decode_entities, jsx_text};

    #[test]
    fn jsx_text_follows_react() {
        assert_eq!(
            jsx_text("\n      Hello\n      world  \n    "),
            "Hello world"
        );
        assert_eq!(jsx_text("  a  "), "  a  ");
        assert_eq!(jsx_text("\n  \n"), "");
        assert_eq!(
            jsx_text("a &amp; b &#65;&#x42; &bogus; &"),
            "a & b AB &bogus; &"
        );
        assert_eq!(decode_entities("&lt;p&gt;"), "<p>");
    }
}
