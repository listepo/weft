//! Reader state: what a closure does, where a view is placed, and the reader itself.

use std::collections::HashMap;

use weft_core::Catalog;

use super::super::syntax::{Expr, Stmt};
use weft_import::{IdState, Losses};

/// What a closure does, read as Weft: the action it fires, a form it submits, a link it opens.
#[derive(Default)]
pub(super) struct Handler {
    pub(super) action: Option<String>,
    pub(super) submit: Option<String>,
    pub(super) href: Option<Expr>,
    /// Code that is not a Weft action.
    pub(super) other: Vec<String>,
}

/// Where a view is placed: decides between kinds that share a SwiftUI form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    /// Any element container.
    Nodes,
    List,
    Table,
    Header,
    Row,
    Choice(&'static str),
    Tabs,
    Menu,
}

pub(super) struct Loop {
    pub(super) ident: String,
    pub(super) index: Option<String>,
    pub(super) name: String,
}

#[derive(PartialEq, Eq)]
pub(super) enum Root {
    Object(String),
    Value,
    Theme,
}

pub(super) struct Reader<'a> {
    pub(super) catalog: &'a Catalog,
    /// The kinds of the project's own catalog, by the name of the view the app writes for each.
    pub(super) custom: HashMap<String, String>,
    pub(super) actions: HashMap<String, String>,
    pub(super) views: HashMap<String, &'a [Stmt]>,
    pub(super) roots: HashMap<String, Root>,
    pub(super) loops: Vec<Loop>,
    pub(super) aliases: Vec<HashMap<String, Expr>>,
    pub(super) inlining: Vec<String>,
    pub(super) ids: IdState,
    pub(super) losses: Losses,
    pub(super) elements: usize,
    pub(super) depth: usize,
    pub(super) truncated: bool,
    /// The view being read is a child of an `HStack`, where a full-width frame is `grow`.
    pub(super) row_child: bool,
}

/// The reserved names of the generated view; a loop variable that took a `_` to avoid one
/// gets its Weft name back.
pub(super) const VIEW_MEMBERS: &[&str] = &[
    "body", "model", "theme", "perform", "send", "submit", "openLink", "openURL",
];

pub(super) const LAYOUT_MODIFIERS: &[&str] = &[
    "padding",
    "frame",
    "background",
    "foregroundColor",
    "foregroundStyle",
    "font",
    "fontWeight",
    "bold",
    "italic",
    "cornerRadius",
    "clipShape",
    "shadow",
    "opacity",
    "offset",
    "fixedSize",
    "lineLimit",
    "multilineTextAlignment",
    "listStyle",
    "listRowBackground",
    "scrollContentBackground",
    "navigationTitle",
    "navigationBarTitleDisplayMode",
    "toolbar",
    "tint",
    "controlSize",
    "labelsHidden",
    "border",
    "overlay",
    "imageScale",
    "resizable",
    "scaledToFit",
    "scaledToFill",
    "aspectRatio",
    "textFieldStyle",
    "buttonStyle",
    "pickerStyle",
    "toggleStyle",
    "formStyle",
    "groupBoxStyle",
    "presentationDetents",
    "ignoresSafeArea",
    "contentShape",
];
pub(super) const VALUE_TYPES: &[&str] = &[
    "String", "Bool", "Int", "Double", "Float", "CGFloat", "Date",
];
