//! Source fragments of a generated component: an element tree printed as JSX, and expressions
//! whose layout depends on where they are printed.

use std::rc::Rc;

use super::N;

/// A string built at run time: literal parts and JavaScript expressions, joined with `+`.
#[derive(Clone, Debug)]
pub(crate) enum Part {
    S(String),
    Js(String),
}

pub(crate) type Str = Vec<Part>;

pub(crate) fn s(text: impl Into<String>) -> Part {
    Part::S(text.into())
}

pub(crate) fn js(code: impl Into<String>) -> Part {
    Part::Js(code.into())
}

/// The expression that builds `parts` at run time.
pub(crate) fn str_js(parts: &[Part]) -> String {
    let mut merged: Vec<Part> = Vec::new();
    for p in parts {
        match (p, merged.last_mut()) {
            (Part::S(text), Some(Part::S(last))) => last.push_str(text),
            _ => merged.push(p.clone()),
        }
    }
    if merged.is_empty() {
        return "\"\"".to_owned();
    }
    merged
        .iter()
        .map(|p| match p {
            Part::S(text) => crate::js::quote(text),
            Part::Js(code) => code.clone(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

/// Whether every part is known at compile time, and the text if so.
pub(crate) fn str_literal(parts: &[Part]) -> Option<String> {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::S(text) => out.push_str(text),
            Part::Js(_) => return None,
        }
    }
    Some(out)
}

/// A child of an element: an element, a text run, or an expression.
pub(crate) enum C<'a> {
    J(J<'a>),
    Text(String),
    Code(Code<'a>),
}

/// An element; the empty tag is a fragment.
pub(crate) struct J<'a> {
    pub tag: String,
    pub attrs: Vec<Attr<'a>>,
    pub kids: Vec<C<'a>>,
}

pub(crate) struct Attr<'a> {
    pub name: String,
    pub value: AttrV<'a>,
}

pub(crate) enum AttrV<'a> {
    /// A string, quoted when its characters are inert.
    S(String),
    Js(String),
    /// A boolean attribute that is present.
    True,
    /// An element or expression, printed where the attribute stands (SolidJS `fallback`).
    Expr(Box<C<'a>>),
}

pub(crate) fn attr<'a>(name: &str, value: AttrV<'a>) -> Attr<'a> {
    Attr {
        name: name.to_owned(),
        value,
    }
}

pub(crate) fn attr_s<'a>(name: &str, value: impl Into<String>) -> Attr<'a> {
    attr(name, AttrV::S(value.into()))
}

pub(crate) fn attr_js<'a>(name: &str, code: impl Into<String>) -> Attr<'a> {
    attr(name, AttrV::Js(code.into()))
}

pub(crate) fn el<'a>(tag: &str, attrs: Vec<Attr<'a>>, kids: Vec<C<'a>>) -> J<'a> {
    J {
        tag: tag.to_owned(),
        attrs,
        kids,
    }
}

/// An expression whose text depends on the indentation it is printed at.
#[derive(Default)]
pub(crate) struct Code<'a>(pub Vec<Seg<'a>>);

pub(crate) enum Seg<'a> {
    Lit(String),
    /// An element or expression in expression position.
    Expr(Box<C<'a>>),
    Code(Code<'a>),
    /// `[` one item per line `]`.
    Array(Vec<Code<'a>>),
    /// `(() => { lines; return result; })()`, for elements that need values computed once.
    Block(Vec<Code<'a>>, Box<C<'a>>),
    /// The accessible name of a node, read when printed (the TypeScript generator read it lazily,
    /// which decides whether its helpers count as used when the code is never printed).
    Label(Rc<N<'a>>),
    /// The resolved `value` prop of a node, read when printed.
    Value(Rc<N<'a>>),
}

impl<'a> Code<'a> {
    pub(crate) fn lit(text: impl Into<String>) -> Self {
        Code(vec![Seg::Lit(text.into())])
    }

    pub(crate) fn of(segs: Vec<Seg<'a>>) -> Self {
        Code(segs)
    }
}

pub(crate) fn lit<'a>(text: impl Into<String>) -> Seg<'a> {
    Seg::Lit(text.into())
}

pub(crate) fn expr<'a>(c: C<'a>) -> Seg<'a> {
    Seg::Expr(Box::new(c))
}
