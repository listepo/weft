//! Role → kind. Everything the catalog can answer comes from its own `role` fields; the tables
//! here hold only what the catalog cannot express.
//!
//! Structural inversions are applied where the tree is assembled (`build.rs`):
//! - tablist + following tabpanels → tabs with one tab per tab, holding its matched panel;
//! - a table row of only columnheaders → the table's column children;
//! - a checked radio or selected option → the enclosing component's writable `value`;
//! - the selected tab → `tabs.selected`.

use std::collections::HashMap;

use weft_core::{Catalog, ComponentDef, Content};

/// A role a renderer derives from a kind plus a prop (SPEC §5.1 notes), or that implies a prop
/// value because the role is only exposed in that state.
pub struct Refinement {
    pub role: &'static str,
    pub kind: &'static str,
    pub props: &'static [(&'static str, &'static str)],
}

pub const ROLE_REFINEMENTS: &[Refinement] = &[
    Refinement {
        role: "spinbutton",
        kind: "field",
        props: &[("type", "number")],
    },
    Refinement {
        role: "searchbox",
        kind: "field",
        props: &[("type", "search")],
    },
    Refinement {
        role: "paragraph",
        kind: "text",
        props: &[],
    },
    // A dialog is in the accessibility tree only while it is shown.
    Refinement {
        role: "dialog",
        kind: "dialog",
        props: &[("open", "true")],
    },
];

/// Roles that add no element of their own: ARIA's generic and presentational roles, and the header
/// and body row groups a renderer emits for a table (SPEC §5.1 notes).
pub const DISSOLVED_ROLES: &[&str] = &["generic", "none", "presentation", "rowgroup"];

pub fn dissolved(role: &str) -> bool {
    DISSOLVED_ROLES.contains(&role)
}

/// Roles more than one kind has: the kind a bare element of that role is. The others are told
/// apart by what the element says (`<input type=date>`) or by the kind it sits in.
pub const ROLE_DEFAULTS: &[(&str, &str)] = &[("textbox", "field"), ("combobox", "select")];

pub struct Resolved<'c> {
    pub kind: String,
    pub def: &'c ComponentDef,
    pub preset: &'static [(&'static str, &'static str)],
}

pub struct KindIndex<'c> {
    pub catalog: &'c Catalog,
    /// The kinds of each role, in catalog order.
    pub by_role: HashMap<&'c str, Vec<&'c str>>,
    /// The role-less text kind that wraps loose text runs where only elements may go.
    pub text: Option<&'c str>,
}

impl<'c> KindIndex<'c> {
    pub fn new(catalog: &'c Catalog) -> Self {
        let mut by_role: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut text = None;
        for (kind, def) in &catalog.components {
            if def.role == "none" {
                if def.content == Content::Text && text.is_none() {
                    text = Some(kind.as_str());
                }
                continue;
            }
            by_role
                .entry(def.role.as_str())
                .or_default()
                .push(kind.as_str());
        }
        KindIndex {
            catalog,
            by_role,
            text,
        }
    }

    pub fn component(&self, kind: &str) -> Option<&'c ComponentDef> {
        self.catalog.components.get(kind)
    }

    /// The kind a bare element of `role` is.
    pub fn kind_of(&self, role: &str) -> Option<&'c str> {
        let kinds = self.by_role.get(role)?;
        ROLE_DEFAULTS
            .iter()
            .find(|(r, _)| *r == role)
            .and_then(|(_, kind)| kinds.iter().find(|k| *k == kind))
            .or_else(|| kinds.first())
            .copied()
    }

    /// Among the kinds of `role`, the one that belongs inside `parent` (a radio in a segmented
    /// control is a segment), or else the bare kind.
    fn kind_in(&self, role: &str, parent: &str) -> Option<&'c str> {
        self.by_role
            .get(role)?
            .iter()
            .find(|kind| {
                self.component(kind)
                    .and_then(|def| def.allowed_parents.as_ref())
                    .is_some_and(|parents| parents.iter().any(|p| p == parent))
            })
            .copied()
            .or_else(|| self.kind_of(role))
    }

    pub fn resolve(&self, role: &str, named: Option<&str>, parent: &str) -> Option<Resolved<'c>> {
        if let Some(named) = named {
            return self.component(named).map(|def| Resolved {
                kind: named.to_owned(),
                def,
                preset: &[],
            });
        }
        if let Some(r) = ROLE_REFINEMENTS.iter().find(|r| r.role == role)
            && let Some(def) = self.component(r.kind)
        {
            return Some(Resolved {
                kind: r.kind.to_owned(),
                def,
                preset: r.props,
            });
        }
        let kind = self.kind_in(role, parent)?;
        self.component(kind).map(|def| Resolved {
            kind: kind.to_owned(),
            def,
            preset: &[],
        })
    }
}
