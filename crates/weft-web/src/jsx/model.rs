//! The document model the generator walks: nodes, pieces and the scope a loop binds.

use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Map, Value as Json};
use weft_core::ComponentDef;
use weft_import::{js_trim, squash};

use super::Gen;
use super::tree::{C, Code, Str};

// ---- Scope and pieces ----

#[derive(Clone)]
pub(super) struct Var {
    pub(super) ident: String,
    pub(super) path: Str,
}

#[derive(Clone, Default)]
pub(super) struct Scope {
    pub(super) vars: HashMap<String, Var>,
    pub(super) suffix: Str,
    /// Prepended to every id, so an inline fragment's elements keep the use's instance path.
    pub(super) prefix: Str,
    pub(super) item: Option<Str>,
    pub(super) loops: usize,
}

/// A node of the document as the renderer's expansion reads it.
pub(crate) struct N<'a> {
    pub(super) kind: &'a str,
    pub(super) def: Option<&'a ComponentDef>,
    pub(super) doc_id: &'a str,
    pub(super) id: Str,
    pub(super) props: Option<&'a Map<String, Json>>,
    pub(super) on: Option<&'a Map<String, Json>>,
    pub(super) children: Option<&'a Json>,
    pub(super) slots: Option<&'a Map<String, Json>>,
    pub(super) scope: Rc<Scope>,
    pub(super) depth: usize,
}

impl<'a> N<'a> {
    pub(super) fn raw(&self, name: &str) -> Option<&'a Json> {
        self.props?.get(name)
    }

    pub(super) fn event(&self, name: &str) -> Option<&'a str> {
        self.on?.get(name)?.as_str()
    }

    pub(super) fn slot(&self, name: &str) -> Option<&'a Json> {
        self.slots?.get(name)
    }
}

pub(super) enum Piece<'a> {
    Text(String),
    /// A node and, when it depends on data, the expression that hides it.
    Node(Rc<N<'a>>, Option<String>),
    Each {
        src: String,
        ident: String,
        index: String,
        inner: Vec<Piece<'a>>,
    },
}

/// A piece that is not a loop, as the per-piece callbacks see it.
#[derive(Clone, Copy)]
pub(super) enum Leaf<'p, 'a> {
    Text(&'p str),
    Node(&'p Rc<N<'a>>),
}

/// Which way a parent `stack` lays its children out, so a child with `grow` can take that axis.
#[derive(Clone)]
pub(super) enum Grow {
    Row,
    Column,
    /// A bound direction: a JS expression that equals `"row"` when the stack is a row.
    Bound(String),
}

/// `group` is the enclosing radio-group, `form` the nearest enclosing form. `grow` is set only
/// for the default children of a `stack` (a named slot is not that stack's child, SPEC §2.2).
#[derive(Clone, Default)]
pub(super) struct Ctx<'a> {
    pub(super) group: Option<Rc<N<'a>>>,
    pub(super) form: Option<Rc<N<'a>>>,
    pub(super) grow: Option<Grow>,
}

pub(super) fn is_binding_raw(raw: Option<&Json>) -> bool {
    raw.and_then(Json::as_object)
        .is_some_and(|o| o.get("bind").is_some_and(Json::is_string))
}

/// Strings of a `text`-content kind, joined as the renderer's `contentText` joins them.
pub(super) fn content_text(list: Option<&Json>) -> String {
    let Some(Json::Array(items)) = list else {
        return String::new();
    };
    let parts: Vec<&str> = items
        .iter()
        .filter_map(Json::as_str)
        .map(js_trim)
        .filter(|s| !s.is_empty())
        .collect();
    squash(&parts.join(" "))
}

pub(super) fn filter_pieces<'a>(
    list: &[Piece<'a>],
    keep: &dyn Fn(Leaf<'_, 'a>) -> bool,
) -> Vec<Piece<'a>> {
    let mut out = Vec::new();
    for p in list {
        match p {
            Piece::Text(text) => {
                if keep(Leaf::Text(text)) {
                    out.push(Piece::Text(text.clone()));
                }
            }
            Piece::Node(n, hidden) => {
                if keep(Leaf::Node(n)) {
                    out.push(Piece::Node(n.clone(), hidden.clone()));
                }
            }
            Piece::Each {
                src,
                ident,
                index,
                inner,
            } => {
                let inner = filter_pieces(inner, keep);
                if !inner.is_empty() {
                    out.push(Piece::Each {
                        src: src.clone(),
                        ident: ident.clone(),
                        index: index.clone(),
                        inner,
                    });
                }
            }
        }
    }
    out
}

pub(super) type MapFn<'f, 'a> = dyn FnMut(&mut Gen<'a>, Leaf<'_, 'a>) -> Option<C<'a>> + 'f;
pub(super) type MakeFn<'f, 'a> = dyn FnMut(&mut Gen<'a>, Leaf<'_, 'a>) -> Option<Code<'a>> + 'f;
