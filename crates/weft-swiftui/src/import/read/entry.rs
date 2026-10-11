//! The entry point: which view is the screen, and the ids it claims.

use std::collections::{HashMap, HashSet};

use weft_core::{Catalog, Diagnostic, Document, WEFT_VERSION};

use super::super::syntax::{Expr, Source, Stmt, TypeDecl, plain_string};
use super::super::{ImportResult, MAX_DEPTH};
use crate::kinds::is_core_kind;
use crate::swift;
use weft_import::{IdState, Losses, limit_reached};

use super::helpers::{first_arg, ident};
use super::types::Reader;

pub fn read(
    source: &Source,
    catalog: &Catalog,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ImportResult> {
    let views: Vec<&TypeDecl> = source
        .types
        .iter()
        .filter(|t| t.inherits.iter().any(|i| i == "View") && body(t).is_some())
        .collect();
    let main = pick_main(&views)?;
    let custom = catalog
        .components
        .keys()
        .filter(|k| !is_core_kind(k))
        .map(|k| (swift::view_name(k), k.clone()))
        .collect();
    let mut reader = Reader {
        catalog,
        custom,
        actions: HashMap::new(),
        views: HashMap::new(),
        roots: HashMap::new(),
        loops: vec![],
        aliases: vec![],
        inlining: vec![main.name.clone()],
        ids: IdState::default(),
        losses: Losses::default(),
        elements: 0,
        depth: 0,
        truncated: false,
        row_child: false,
    };
    for t in &source.types {
        for (case, raw) in &t.cases {
            reader
                .actions
                .entry(case.clone())
                .or_insert_with(|| raw.clone().unwrap_or_else(|| case.clone()));
        }
    }
    for v in &views {
        if let Some(b) = body(v) {
            reader.views.insert(v.name.clone(), b);
            reserve_ids(b, &mut reader.ids.reserved);
        }
    }
    reader.classify_roots(main);
    let statements = body(main).unwrap_or_default();
    let root = reader.screen(statements);
    if reader.truncated {
        limit_reached(
            diagnostics,
            "#",
            "is larger or deeper than the import limit",
        );
    }
    Some(ImportResult {
        document: Document {
            weft: WEFT_VERSION.to_owned(),
            version: None,
            context: Vec::new(),
            fragments: Default::default(),
            root,
        },
        losses: reader.losses.0,
        diagnostics: vec![],
    })
}

fn body(t: &TypeDecl) -> Option<&[Stmt]> {
    t.properties
        .iter()
        .find(|p| p.name == "body")
        .and_then(|p| p.body.as_deref())
}

/// The view no other view in the file uses: the screen; helper views are inlined into it.
fn pick_main<'t>(views: &[&'t TypeDecl]) -> Option<&'t TypeDecl> {
    let mut used = HashSet::new();
    for v in views {
        if let Some(b) = body(v) {
            for s in b {
                callees_stmt(s, &mut used, 0);
            }
        }
    }
    views
        .iter()
        .find(|v| !used.contains(v.name.as_str()))
        .or_else(|| views.first())
        .copied()
}

fn callees_stmt(s: &Stmt, out: &mut HashSet<String>, depth: usize) {
    match s {
        Stmt::Expr(e) => callees(e, out, depth),
        Stmt::If {
            then, otherwise, ..
        } => {
            for s in then.iter().chain(otherwise) {
                callees_stmt(s, out, depth + 1);
            }
        }
        Stmt::Other(_) => {}
    }
}

fn callees(e: &Expr, out: &mut HashSet<String>, depth: usize) {
    if depth > MAX_DEPTH * 8 {
        return;
    }
    match e {
        Expr::Call(c) => {
            if let Some(name) = ident(&c.callee) {
                out.insert(name.to_owned());
            }
            callees(&c.callee, out, depth + 1);
            for a in &c.args {
                callees(&a.value, out, depth + 1);
            }
            for cl in &c.closures {
                for s in &cl.body {
                    callees_stmt(s, out, depth + 1);
                }
            }
        }
        Expr::Member { base, .. } => callees(base, out, depth + 1),
        Expr::Closure(cl) => {
            for s in &cl.body {
                callees_stmt(s, out, depth + 1);
            }
        }
        _ => {}
    }
}

/// Explicit ids are claimed before any id is generated, so a generated id never takes one.
fn reserve_ids(statements: &[Stmt], out: &mut HashSet<String>) {
    fn walk(e: &Expr, out: &mut HashSet<String>, depth: usize) {
        if depth > MAX_DEPTH * 8 {
            return;
        }
        match e {
            Expr::Call(c) => {
                if let Expr::Member { name, .. } = c.callee.as_ref()
                    && (name == "accessibilityIdentifier" || name == "weftEach")
                    && let Some(Expr::Str(parts)) = first_arg(&c.args)
                    && let Some(id) = plain_string(parts)
                {
                    out.insert(id);
                }
                walk(&c.callee, out, depth + 1);
                for a in &c.args {
                    walk(&a.value, out, depth + 1);
                }
                for cl in &c.closures {
                    for s in &cl.body {
                        stmt(s, out, depth + 1);
                    }
                }
            }
            Expr::Member { base, .. } => walk(base, out, depth + 1),
            Expr::Closure(cl) => {
                for s in &cl.body {
                    stmt(s, out, depth + 1);
                }
            }
            _ => {}
        }
    }
    fn stmt(s: &Stmt, out: &mut HashSet<String>, depth: usize) {
        match s {
            Stmt::Expr(e) => walk(e, out, depth),
            Stmt::If {
                then, otherwise, ..
            } => {
                for s in then.iter().chain(otherwise) {
                    stmt(s, out, depth + 1);
                }
            }
            Stmt::Other(_) => {}
        }
    }
    for s in statements {
        stmt(s, out, 0);
    }
}
