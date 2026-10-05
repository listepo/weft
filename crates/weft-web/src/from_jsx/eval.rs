//! What an expression of a component stands for, as far as Weft can tell without running it: a
//! literal, a data path, an action, JSX to be placed later, or a list repeated over data. Only the
//! patterns components (generated ones and the corpus conventions) use are understood; anything
//! else is `Unknown` and the caller reports it as a loss.

use std::rc::Rc;

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionBody, ArrowFunctionExpression, BinaryExpression, BinaryOperator,
    BindingPattern, CallExpression, ChainElement, Expression, FormalParameters, LogicalOperator,
    ObjectPropertyKind, PropertyKey, Statement, UnaryOperator,
};
use weft_core::{is_binding, is_loop_variable, js_number};

use crate::js::is_plain_segment;

/// A value as the importer understands it.
#[derive(Clone)]
pub(crate) enum Sv<'a> {
    Str(String),
    Num(f64),
    Bool(bool),
    Undef,
    /// A data path (`$.x`, or `item.x` inside a repetition); `not` negates it.
    Path {
        path: String,
        not: bool,
    },
    /// Text that starts with a literal and goes on with values only known at run time.
    Prefix(String),
    /// The index of a repetition.
    Index,
    /// The index of the tab a data path selects (`Math.max(0, tabs.findIndex(…))`).
    Choice(String),
    /// Whether a tab is the one the path selects (`index === choice`).
    Chosen(String),
    /// The whole props object of a component (`props` in SolidJS).
    Props,
    /// The `actions` prop, or a namespace inside it (`actions.todo`).
    Actions(String),
    /// A two-way write (`actions.set`, `onChange`): the binding already says it.
    Write,
    Arr(Rc<Vec<Item<'a>>>),
    Obj(Rc<Vec<(String, Sv<'a>)>>),
    /// An expression that renders markup, read where it is placed.
    Jsx(&'a Expression<'a>, Rc<Env<'a>>),
    Unknown,
}

#[derive(Clone)]
pub(crate) enum Item<'a> {
    One(Sv<'a>),
    /// `list.map(…)`: the items once per entry of a data list.
    Repeat(Rc<Repeat<'a>>),
}

pub(crate) struct Repeat<'a> {
    pub list: String,
    pub var: String,
    pub items: Vec<Item<'a>>,
}

/// Names in scope: component parameters, loop variables, `const`s.
#[derive(Default)]
pub(crate) struct Env<'a> {
    vars: Vec<(String, Sv<'a>)>,
    parent: Option<Rc<Env<'a>>>,
}

impl<'a> Env<'a> {
    pub fn child(parent: &Rc<Env<'a>>, vars: Vec<(String, Sv<'a>)>) -> Rc<Env<'a>> {
        Rc::new(Env {
            vars,
            parent: Some(parent.clone()),
        })
    }

    /// Iterative: scopes nest as deep as the source does.
    fn get(&self, name: &str) -> Option<Sv<'a>> {
        let mut env = self;
        loop {
            if let Some((_, v)) = env.vars.iter().rev().find(|(n, _)| n == name) {
                return Some(v.clone());
            }
            env = env.parent.as_deref()?;
        }
    }
}

/// Helpers of the generated components that pass their argument through, as far as Weft is
/// concerned: they only coerce or guard it.
const PASS_THROUGH: &[&str] = &[
    "_text", "_on", "_squash", "_list", "_keyed", "_ix", "_float", "String", "Boolean", "Number",
];

/// Calls of a value that leave it a value of the same path (`.trim()`, `.toString()`).
const SAME_VALUE: &[&str] = &["trim", "toString", "valueOf", "toLowerCase", "toUpperCase"];

/// Steps any one import may take; static `map` over static arrays could otherwise multiply.
pub(crate) const MAX_STEPS: usize = 1_000_000;

pub(crate) struct Eval {
    pub steps: usize,
}

impl Sv<'_> {
    pub fn truthy(&self) -> Option<bool> {
        match self {
            Sv::Str(s) => Some(!s.is_empty()),
            Sv::Num(n) => Some(*n != 0.0 && !n.is_nan()),
            Sv::Bool(b) => Some(*b),
            Sv::Undef => Some(false),
            Sv::Arr(_) | Sv::Obj(_) | Sv::Jsx(..) | Sv::Props | Sv::Actions(_) | Sv::Write => {
                Some(true)
            }
            _ => None,
        }
    }

    /// The text a literal renders as.
    pub fn text(&self) -> Option<String> {
        match self {
            Sv::Str(s) => Some(s.clone()),
            Sv::Num(n) => Some(js_number(*n)),
            _ => None,
        }
    }
}

/// The property name of a key that names one statically.
pub(crate) fn key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.as_str().to_owned()),
        PropertyKey::StringLiteral(s) => Some(s.value.as_str().to_owned()),
        _ => None,
    }
}

fn join(path: &str, segment: &str) -> Option<String> {
    is_plain_segment(segment).then(|| format!("{path}.{segment}"))
}

impl Eval {
    fn step(&mut self) -> bool {
        self.steps += 1;
        self.steps <= MAX_STEPS
    }

    pub fn eval<'a>(&mut self, e: &'a Expression<'a>, env: &Rc<Env<'a>>) -> Sv<'a> {
        if !self.step() {
            return Sv::Unknown;
        }
        let e = e.get_inner_expression();
        match e {
            Expression::StringLiteral(s) => Sv::Str(s.value.as_str().to_owned()),
            Expression::NumericLiteral(n) => Sv::Num(n.value),
            Expression::BooleanLiteral(b) => Sv::Bool(b.value),
            Expression::NullLiteral(_) => Sv::Undef,
            Expression::TemplateLiteral(t) => {
                let mut out = String::new();
                for (i, q) in t.quasis.iter().enumerate() {
                    match &q.value.cooked {
                        Some(c) => out.push_str(c.as_str()),
                        None => return Sv::Unknown,
                    }
                    if let Some(x) = t.expressions.get(i) {
                        match self.eval(x, env).text() {
                            Some(s) => out.push_str(&s),
                            None => return if i == 0 { Sv::Prefix(out) } else { Sv::Unknown },
                        }
                    }
                }
                Sv::Str(out)
            }
            Expression::Identifier(id) => match id.name.as_str() {
                "undefined" => Sv::Undef,
                name => env.get(name).unwrap_or(Sv::Unknown),
            },
            Expression::StaticMemberExpression(m) => {
                let object = self.eval(&m.object, env);
                self.member(object, m.property.name.as_str())
            }
            Expression::ComputedMemberExpression(m) => {
                let object = self.eval(&m.object, env);
                match self.eval(&m.expression, env) {
                    Sv::Str(s) => self.member(object, &s),
                    Sv::Num(n) if n >= 0.0 && n.fract() == 0.0 => match object {
                        Sv::Arr(items) => match items.get(n as usize) {
                            Some(Item::One(v)) => v.clone(),
                            _ => Sv::Unknown,
                        },
                        other => self.member(other, &js_number(n)),
                    },
                    _ => Sv::Unknown,
                }
            }
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => self.call(call, env),
                ChainElement::StaticMemberExpression(m) => {
                    let object = self.eval(&m.object, env);
                    self.member(object, m.property.name.as_str())
                }
                ChainElement::ComputedMemberExpression(m) => {
                    let object = self.eval(&m.object, env);
                    match self.eval(&m.expression, env) {
                        Sv::Str(s) => self.member(object, &s),
                        Sv::Num(n) => self.member(object, &js_number(n)),
                        _ => Sv::Unknown,
                    }
                }
                _ => Sv::Unknown,
            },
            Expression::CallExpression(call) => self.call(call, env),
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                match self.eval(&u.argument, env) {
                    Sv::Path { path, not } => Sv::Path { path, not: !not },
                    v => v.truthy().map_or(Sv::Unknown, |t| Sv::Bool(!t)),
                }
            }
            Expression::BinaryExpression(b) => self.binary(b, env),
            Expression::LogicalExpression(l) => {
                let left = self.eval(&l.left, env);
                match (l.operator, left.truthy()) {
                    (LogicalOperator::Or, Some(true)) | (LogicalOperator::And, Some(false)) => left,
                    (LogicalOperator::Or | LogicalOperator::And, Some(_)) => {
                        self.eval(&l.right, env)
                    }
                    (LogicalOperator::Coalesce, _) if matches!(left, Sv::Undef) => {
                        self.eval(&l.right, env)
                    }
                    // A fallback (`data.x || ""`, `data.x ?? 0`) does not change which value is
                    // bound.
                    (LogicalOperator::Or | LogicalOperator::Coalesce, None)
                        if matches!(left, Sv::Path { .. }) =>
                    {
                        left
                    }
                    _ if renders(&l.right) => Sv::Jsx(e, env.clone()),
                    _ => Sv::Unknown,
                }
            }
            Expression::ConditionalExpression(c) => match self.eval(&c.test, env).truthy() {
                Some(true) => self.eval(&c.consequent, env),
                Some(false) => self.eval(&c.alternate, env),
                None if renders(&c.consequent) || renders(&c.alternate) => Sv::Jsx(e, env.clone()),
                None => {
                    // `test ? data.x : ""`: text shown when it is there, which a binding does.
                    let yes = self.eval(&c.consequent, env);
                    let no = self.eval(&c.alternate, env);
                    match (yes, no) {
                        (p @ Sv::Path { .. }, Sv::Str(s)) if s.is_empty() => p,
                        (p @ Sv::Path { .. }, Sv::Undef) => p,
                        _ => Sv::Unknown,
                    }
                }
            },
            Expression::ArrayExpression(a) => {
                let mut items = Vec::new();
                for el in &a.elements {
                    match el {
                        ArrayExpressionElement::SpreadElement(s) => {
                            match self.eval(&s.argument, env) {
                                Sv::Arr(inner) => items.extend(inner.iter().cloned()),
                                _ => return Sv::Unknown,
                            }
                        }
                        ArrayExpressionElement::Elision(_) => items.push(Item::One(Sv::Undef)),
                        other => match other.as_expression() {
                            Some(x) => items.push(Item::One(self.eval(x, env))),
                            None => return Sv::Unknown,
                        },
                    }
                }
                Sv::Arr(Rc::new(items))
            }
            Expression::ObjectExpression(o) => {
                let mut fields = Vec::new();
                for p in &o.properties {
                    match p {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            if let Some(key) = key_name(&p.key) {
                                fields.push((key, self.eval(&p.value, env)));
                            }
                        }
                        ObjectPropertyKind::SpreadProperty(_) => return Sv::Unknown,
                    }
                }
                Sv::Obj(Rc::new(fields))
            }
            Expression::JSXElement(_) | Expression::JSXFragment(_) => Sv::Jsx(e, env.clone()),
            _ => Sv::Unknown,
        }
    }

    fn member<'a>(&mut self, object: Sv<'a>, name: &str) -> Sv<'a> {
        match object {
            Sv::Path { path, not: false } => match join(&path, name) {
                Some(p) => Sv::Path {
                    path: p,
                    not: false,
                },
                None => Sv::Unknown,
            },
            Sv::Props => match name {
                "data" => Sv::Path {
                    path: "$".into(),
                    not: false,
                },
                "actions" => Sv::Actions(String::new()),
                "onChange" => Sv::Write,
                other => join("$", other).map_or(Sv::Unknown, |path| Sv::Path { path, not: false }),
            },
            Sv::Actions(ns) if ns.is_empty() && name == "set" => Sv::Write,
            Sv::Actions(ns) => Sv::Actions(if ns.is_empty() {
                name.to_owned()
            } else {
                format!("{ns}.{name}")
            }),
            Sv::Obj(fields) => fields
                .iter()
                .rev()
                .find(|(k, _)| k == name)
                .map_or(Sv::Undef, |(_, v)| v.clone()),
            Sv::Arr(items) if name == "length" => Sv::Num(items.len() as f64),
            _ => Sv::Unknown,
        }
    }

    fn binary<'a>(&mut self, b: &'a BinaryExpression<'a>, env: &Rc<Env<'a>>) -> Sv<'a> {
        let left = self.eval(&b.left, env);
        let right = self.eval(&b.right, env);
        match b.operator {
            BinaryOperator::Addition => match (&left, &right) {
                (Sv::Num(x), Sv::Num(y)) => Sv::Num(x + y),
                _ => match (left.text(), right.text()) {
                    (Some(x), Some(y)) => Sv::Str(x + &y),
                    (Some(x), None) => Sv::Prefix(x),
                    _ => match left {
                        Sv::Prefix(x) => Sv::Prefix(x),
                        _ => Sv::Unknown,
                    },
                },
            },
            BinaryOperator::StrictEquality | BinaryOperator::Equality => match (&left, &right) {
                (Sv::Num(_) | Sv::Index, Sv::Choice(p)) | (Sv::Choice(p), Sv::Num(_) | Sv::Index) => {
                    Sv::Chosen(p.clone())
                }
                _ => same(&left, &right).map_or(Sv::Unknown, Sv::Bool),
            },
            BinaryOperator::StrictInequality | BinaryOperator::Inequality => {
                same(&left, &right).map_or(Sv::Unknown, |s| Sv::Bool(!s))
            }
            _ => Sv::Unknown,
        }
    }

    fn call<'a>(&mut self, call: &'a CallExpression<'a>, env: &Rc<Env<'a>>) -> Sv<'a> {
        let first = call.arguments.first().and_then(Argument::as_expression);
        let callee = call.callee.get_inner_expression();
        if let Expression::Identifier(id) = callee {
            let name = id.name.as_str();
            if PASS_THROUGH.contains(&name) {
                return first.map_or(Sv::Undef, |x| self.eval(x, env));
            }
            if name == "_url" {
                return match first.map(|x| self.eval(x, env)) {
                    Some(Sv::Str(s)) if s.is_empty() => Sv::Undef,
                    Some(v) => v,
                    None => Sv::Undef,
                };
            }
            if name == "_get" {
                return self.get(call, env);
            }
        }
        // An immediately called function: its constants, then what it returns.
        if call.arguments.is_empty() {
            match callee {
                Expression::ArrowFunctionExpression(f) => {
                    return self.body(&f.body, env);
                }
                Expression::FunctionExpression(f) => {
                    if let Some(body) = &f.body {
                        return self.statements(&body.statements, env);
                    }
                    return Sv::Unknown;
                }
                _ => {}
            }
        }
        let (object, method) = match callee {
            Expression::StaticMemberExpression(m) => (&m.object, m.property.name.as_str()),
            _ => match self.eval(callee, env) {
                Sv::Index => return Sv::Index,
                _ => return Sv::Unknown,
            },
        };
        if SAME_VALUE.contains(&method) {
            return self.eval(object, env);
        }
        if method == "max"
            && matches!(object.get_inner_expression(), Expression::Identifier(m) if m.name == "Math")
        {
            // `Math.max(0, choice)` keeps a selection that matches no tab on the first tab.
            if let [zero, choice] = &call.arguments[..]
                && let (Some(zero), Some(choice)) = (zero.as_expression(), choice.as_expression())
                && matches!(self.eval(zero, env), Sv::Num(n) if n == 0.0)
                && let choice @ Sv::Choice(_) = self.eval(choice, env)
            {
                return choice;
            }
            return Sv::Unknown;
        }
        if method == "findIndex"
            && matches!(self.eval(object, env), Sv::Arr(_))
            && let Some(Expression::ArrowFunctionExpression(f)) =
                first.map(Expression::get_inner_expression)
        {
            return self.tab_choice(f, env).map_or(Sv::Unknown, Sv::Choice);
        }
        if method == "map" || method == "flatMap" {
            let list = self.eval(object, env);
            let Some(Expression::ArrowFunctionExpression(f)) =
                first.map(Expression::get_inner_expression)
            else {
                return Sv::Unknown;
            };
            return self.map(list, &f.params, &f.body, method == "flatMap", env);
        }
        Sv::Unknown
    }

    /// `_get(base, ["players", "0", "name"])`: the path a generated component reads when a
    /// segment cannot follow `?.` (an index, or a name `Object.prototype` also has). Only
    /// segments a binding can spell are read back.
    fn get<'a>(&mut self, call: &'a CallExpression<'a>, env: &Rc<Env<'a>>) -> Sv<'a> {
        let mut args = call.arguments.iter().filter_map(Argument::as_expression);
        let (Some(base), Some(segments), None) = (args.next(), args.next(), args.next()) else {
            return Sv::Unknown;
        };
        let (Sv::Path { mut path, not: false }, Sv::Arr(segments)) =
            (self.eval(base, env), self.eval(segments, env))
        else {
            return Sv::Unknown;
        };
        for segment in segments.iter() {
            match segment {
                Item::One(Sv::Str(s)) if is_binding(&format!("$.{s}")) => {
                    path.push('.');
                    path.push_str(s);
                }
                _ => return Sv::Unknown,
            }
        }
        Sv::Path { path, not: false }
    }

    /// `(x) => x.doc === path || x.id === path`: how generated tabs find the selected tab; the
    /// one data path both sides compare with is the tablist's `selected` binding.
    fn tab_choice<'a>(
        &mut self,
        f: &'a ArrowFunctionExpression<'a>,
        env: &Rc<Env<'a>>,
    ) -> Option<String> {
        let BindingPattern::BindingIdentifier(param) = &f.params.items.first()?.pattern else {
            return None;
        };
        let Expression::LogicalExpression(l) = f.body.as_expression()?.get_inner_expression()
        else {
            return None;
        };
        if l.operator != LogicalOperator::Or {
            return None;
        }
        let by_doc = self.tab_match(&l.left, param.name.as_str(), "doc", env)?;
        let by_id = self.tab_match(&l.right, param.name.as_str(), "id", env)?;
        (by_doc == by_id).then_some(by_doc)
    }

    fn tab_match<'a>(
        &mut self,
        e: &'a Expression<'a>,
        param: &str,
        field: &str,
        env: &Rc<Env<'a>>,
    ) -> Option<String> {
        let Expression::BinaryExpression(b) = e.get_inner_expression() else {
            return None;
        };
        let Expression::StaticMemberExpression(m) = b.left.get_inner_expression() else {
            return None;
        };
        let on_param =
            matches!(m.object.get_inner_expression(), Expression::Identifier(i) if i.name == param);
        if b.operator != BinaryOperator::StrictEquality || !on_param || m.property.name != field {
            return None;
        }
        match self.eval(&b.right, env) {
            Sv::Path { path, not: false } => Some(path),
            _ => None,
        }
    }

    /// `list.map(cb)` or `<For each={list}>{cb}</For>`: unrolled over a static array, a
    /// repetition over a data list.
    pub fn map<'a>(
        &mut self,
        list: Sv<'a>,
        params: &'a FormalParameters<'a>,
        body: &'a ArrowFunctionBody<'a>,
        flat: bool,
        env: &Rc<Env<'a>>,
    ) -> Sv<'a> {
        let item = params.items.first().map(|p| &p.pattern);
        let index = params.items.get(1).map(|p| &p.pattern);
        match list {
            Sv::Arr(entries) => {
                let mut items = Vec::new();
                for (i, entry) in entries.iter().enumerate() {
                    let Item::One(v) = entry else {
                        return Sv::Unknown;
                    };
                    let mut vars = Vec::new();
                    bind_pattern(item, v.clone(), &mut vars);
                    bind_pattern(index, Sv::Num(i as f64), &mut vars);
                    let scope = Env::child(env, vars);
                    let out = self.body(body, &scope);
                    push_result(&mut items, out, flat);
                }
                Sv::Arr(Rc::new(items))
            }
            Sv::Path { path, not: false } => {
                let mut vars = Vec::new();
                let var = loop_var(item, &mut vars);
                bind_pattern(index, Sv::Index, &mut vars);
                let scope = Env::child(env, vars);
                let out = self.body(body, &scope);
                let mut items = Vec::new();
                push_result(&mut items, out, flat);
                Sv::Arr(Rc::new(vec![Item::Repeat(Rc::new(Repeat {
                    list: path,
                    var,
                    items,
                }))]))
            }
            _ => Sv::Unknown,
        }
    }

    pub fn body<'a>(&mut self, body: &'a ArrowFunctionBody<'a>, env: &Rc<Env<'a>>) -> Sv<'a> {
        match body {
            ArrowFunctionBody::FunctionBody(b) => self.statements(&b.statements, env),
            other => match other.as_expression() {
                Some(x) => self.eval(x, env),
                None => Sv::Unknown,
            },
        }
    }

    /// The value a function body returns: its `const`s are in scope for the last `return`.
    pub fn statements<'a>(&mut self, statements: &'a [Statement<'a>], env: &Rc<Env<'a>>) -> Sv<'a> {
        let mut scope = env.clone();
        let mut result = Sv::Undef;
        for s in statements {
            match s {
                Statement::VariableDeclaration(d) => {
                    let mut vars = Vec::new();
                    for decl in &d.declarations {
                        let value = decl
                            .init
                            .as_ref()
                            .map_or(Sv::Undef, |x| self.eval(x, &scope));
                        bind_pattern(Some(&decl.id), value, &mut vars);
                    }
                    scope = Env::child(&scope, vars);
                }
                Statement::ReturnStatement(r) => {
                    result = r
                        .argument
                        .as_ref()
                        .map_or(Sv::Undef, |x| self.eval(x, &scope));
                }
                _ => {}
            }
        }
        result
    }
}

fn push_result<'a>(items: &mut Vec<Item<'a>>, out: Sv<'a>, flat: bool) {
    match out {
        Sv::Arr(inner) if flat => items.extend(inner.iter().cloned()),
        other => items.push(Item::One(other)),
    }
}

/// Whether two values are equal, when both are known.
fn same(a: &Sv<'_>, b: &Sv<'_>) -> Option<bool> {
    match (a, b) {
        (Sv::Str(x), Sv::Str(y)) => Some(x == y),
        (Sv::Num(x), Sv::Num(y)) => Some(x == y),
        (Sv::Bool(x), Sv::Bool(y)) => Some(x == y),
        (Sv::Undef, Sv::Undef) => Some(true),
        (Sv::Str(_) | Sv::Num(_) | Sv::Bool(_), Sv::Undef)
        | (Sv::Undef, Sv::Str(_) | Sv::Num(_) | Sv::Bool(_)) => Some(false),
        _ => None,
    }
}

/// Whether an expression renders markup in one of its branches.
pub(crate) fn renders(e: &Expression<'_>) -> bool {
    let mut pending = vec![e];
    let mut seen = 0;
    while let Some(e) = pending.pop() {
        seen += 1;
        if seen > 64 {
            return false;
        }
        match e.get_inner_expression() {
            Expression::JSXElement(_) | Expression::JSXFragment(_) => return true,
            Expression::ConditionalExpression(c) => {
                pending.push(&c.consequent);
                pending.push(&c.alternate);
            }
            Expression::LogicalExpression(l) => pending.push(&l.right),
            _ => {}
        }
    }
    false
}

/// The loop variable a callback's first parameter names. A destructured item gets the name
/// `item`, and each of its names is bound to a path under it.
fn loop_var<'a>(
    pattern: Option<&'a BindingPattern<'a>>,
    vars: &mut Vec<(String, Sv<'a>)>,
) -> String {
    match pattern {
        Some(BindingPattern::BindingIdentifier(id)) if is_loop_variable(id.name.as_str()) => {
            // Weft spells a loop variable's paths with a `$`: `{$todo.title}`.
            let name = id.name.as_str().to_owned();
            vars.push((
                name.clone(),
                Sv::Path {
                    path: format!("${name}"),
                    not: false,
                },
            ));
            name
        }
        Some(BindingPattern::ArrayPattern(a)) => {
            // `_ix(list).flatMap(([item, index]) => …)` in generated SolidJS.
            let first = a.elements.first().and_then(Option::as_ref);
            let var = loop_var(first, vars);
            bind_pattern(a.elements.get(1).and_then(Option::as_ref), Sv::Index, vars);
            var
        }
        Some(p @ BindingPattern::ObjectPattern(_)) => {
            bind_pattern(
                Some(p),
                Sv::Path {
                    path: "$item".into(),
                    not: false,
                },
                vars,
            );
            "item".into()
        }
        _ => "item".into(),
    }
}

/// Binds the names of a pattern to the parts of a value.
pub(crate) fn bind_pattern<'a>(
    pattern: Option<&'a BindingPattern<'a>>,
    value: Sv<'a>,
    vars: &mut Vec<(String, Sv<'a>)>,
) {
    let mut pending = vec![(pattern, value)];
    let mut e = Eval { steps: 0 };
    while let Some((pattern, value)) = pending.pop() {
        match pattern {
            Some(BindingPattern::BindingIdentifier(id)) => {
                vars.push((id.name.as_str().to_owned(), value));
            }
            Some(BindingPattern::ObjectPattern(o)) => {
                for p in &o.properties {
                    let v = match key_name(&p.key) {
                        Some(k) => e.member(value.clone(), &k),
                        None => Sv::Unknown,
                    };
                    pending.push((Some(&p.value), v));
                }
            }
            Some(BindingPattern::ArrayPattern(a)) => {
                for (i, el) in a.elements.iter().enumerate() {
                    let v = match &value {
                        Sv::Arr(items) => match items.get(i) {
                            Some(Item::One(v)) => v.clone(),
                            _ => Sv::Unknown,
                        },
                        _ => Sv::Unknown,
                    };
                    pending.push((el.as_ref(), v));
                }
            }
            Some(BindingPattern::AssignmentPattern(a)) => pending.push((Some(&a.left), value)),
            None => {}
        }
    }
}
