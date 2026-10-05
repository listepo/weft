//! The expression tree of a SwiftUI view → Weft elements. Each SwiftUI form `generate` prints is
//! recognised exactly; other views and modifiers map to the nearest kind or become losses.

use std::collections::{HashMap, HashSet};

use weft_core::{
    Catalog, Child, Content, Diagnostic, Document, Node, PropType, Value, WEFT_VERSION,
};

use super::syntax::{Arg, Call, Closure, Expr, Part, Source, Stmt, TypeDecl, plain_string};
use super::{ImportResult, LossKind, MAX_DEPTH, MAX_NODES};
use crate::data::{Leaf, prop_leaf};
use crate::kinds::is_core_kind;
use crate::swift;
use weft_import::{IdState, Losses, limit_reached, literal};

pub fn element_path(parent: &str, node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{parent}/{}#{id}", node.kind),
        None => format!("{parent}/{}", node.kind),
    }
}

/// `[A-Za-z][A-Za-z0-9_-]*` (SPEC §2.2).
pub fn is_id(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// `[a-z][A-Za-z0-9]*(\.[a-z][A-Za-z0-9]*)*` (SPEC §2.2).
fn is_action(s: &str) -> bool {
    s.split('.').all(|seg| {
        let mut chars = seg.chars();
        chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_alphanumeric())
    })
}

fn is_path_name(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A modifier applied to a view: `.name(args) { closures }`.
#[derive(Clone)]
struct Mod {
    name: String,
    args: Vec<Arg>,
    closures: Vec<Closure>,
}

struct Mods(Vec<Mod>);

impl Mods {
    fn take(&mut self, name: &str) -> Option<Mod> {
        let i = self.0.iter().position(|m| m.name == name)?;
        Some(self.0.remove(i))
    }

    fn take_all(&mut self, name: &str) -> Vec<Mod> {
        let (taken, kept) = std::mem::take(&mut self.0)
            .into_iter()
            .partition(|m| m.name == name);
        self.0 = kept;
        taken
    }

    fn has(&self, name: &str) -> bool {
        self.0.iter().any(|m| m.name == name)
    }

    /// Takes `.name(.value)` when its first argument is that implicit member.
    fn take_flag(&mut self, name: &str, value: &str) -> bool {
        let i = self.0.iter().position(|m| {
            m.name == name
                && matches!(m.args.first().map(|a| &a.value), Some(Expr::Implicit(v)) if v == value)
        });
        match i {
            Some(i) => {
                self.0.remove(i);
                true
            }
            None => false,
        }
    }
}

fn first_arg(args: &[Arg]) -> Option<&Expr> {
    args.iter().find(|a| a.label.is_none()).map(|a| &a.value)
}

fn arg<'e>(args: &'e [Arg], label: &str) -> Option<&'e Expr> {
    args.iter()
        .find(|a| a.label.as_deref() == Some(label))
        .map(|a| &a.value)
}

fn closure<'c>(closures: &'c [Closure], label: Option<&str>) -> Option<&'c Closure> {
    closures.iter().find(|c| c.label.as_deref() == label)
}

fn implicit(e: Option<&Expr>) -> Option<&str> {
    match e {
        Some(Expr::Implicit(name)) => Some(name),
        _ => None,
    }
}

fn ident(e: &Expr) -> Option<&str> {
    match e {
        Expr::Ident { name, .. } => Some(name),
        _ => None,
    }
}

/// `Color.clear` → "Color.clear"; `Text` → "Text".
fn type_name(e: &Expr) -> Option<String> {
    match e {
        Expr::Ident { name, .. } => Some(name.clone()),
        Expr::Member { base, name, .. } => Some(format!("{}.{name}", type_name(base)?)),
        _ => None,
    }
}

/// Splits `base.m1().m2()` into the base view and its modifiers, innermost first.
fn unwind(expr: &Expr) -> (&Expr, Vec<Mod>) {
    let mut mods = vec![];
    let mut e = expr;
    while let Expr::Call(Call {
        callee,
        args,
        closures,
    }) = e
    {
        let Expr::Member { base, name, .. } = callee.as_ref() else {
            break;
        };
        // `Color.clear` and `Image.init` are not modifiers: their base is a type.
        if matches!(base.as_ref(), Expr::Ident { name: n, .. } if n.starts_with(|c: char| c.is_ascii_uppercase()))
        {
            break;
        }
        mods.push(Mod {
            name: name.clone(),
            args: args.clone(),
            closures: closures.clone(),
        });
        e = base;
    }
    mods.reverse();
    (e, mods)
}

/// What a closure does, read as Weft: the action it fires, a form it submits, a link it opens.
#[derive(Default)]
struct Handler {
    action: Option<String>,
    submit: Option<String>,
    href: Option<Expr>,
    /// Code that is not a Weft action.
    other: Vec<String>,
}

/// Where a view is placed: decides between kinds that share a SwiftUI form.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
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

struct Loop {
    ident: String,
    index: Option<String>,
    name: String,
}

#[derive(PartialEq, Eq)]
enum Root {
    Object(String),
    Value,
    Theme,
}

struct Reader<'a> {
    catalog: &'a Catalog,
    /// The kinds of the project's own catalog, by the name of the view the app writes for each.
    custom: HashMap<String, String>,
    actions: HashMap<String, String>,
    views: HashMap<String, &'a [Stmt]>,
    roots: HashMap<String, Root>,
    loops: Vec<Loop>,
    aliases: Vec<HashMap<String, Expr>>,
    inlining: Vec<String>,
    ids: IdState,
    losses: Losses,
    elements: usize,
    depth: usize,
    truncated: bool,
}

/// The reserved names of the generated view; a loop variable that took a `_` to avoid one
/// gets its Weft name back.
const VIEW_MEMBERS: &[&str] = &[
    "body", "model", "theme", "perform", "send", "submit", "openLink", "openURL",
];

const LAYOUT_MODIFIERS: &[&str] = &[
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

/// Reads `source` into a document; `None` when it declares no view.
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

fn is_literal(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Str(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Array(_) | Expr::Neg(_)
    )
}

const VALUE_TYPES: &[&str] = &[
    "String", "Bool", "Int", "Double", "Float", "CGFloat", "Date",
];

impl<'a> Reader<'a> {
    fn lose(&mut self, kind: LossKind, path: &str, note: impl Into<String>) {
        self.losses.push(kind, path, note);
    }

    /// Which stored properties hold data: an object's members are data paths, a value property
    /// is one, and a theme's members are token paths.
    fn classify_roots(&mut self, view: &TypeDecl) {
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
    fn take_id(&mut self, explicit: Option<String>, kind: &str, name: &str, path: &str) -> String {
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

    fn string(&mut self, s: &str, path: &str) -> String {
        literal(&mut self.losses, path, s)
    }

    /// The Weft data path an expression reads, if it reads one.
    fn path(&self, e: &Expr) -> Option<String> {
        let p = self.path_inner(e, 0)?;
        // `$` alone and a loop's bare item are not paths a prop can bind to the root.
        (p != "$" && p != "$.").then_some(p)
    }

    fn path_inner(&self, e: &Expr, depth: usize) -> Option<String> {
        self.path_in(e, depth, self.aliases.len())
    }

    /// `scopes` is how many alias scopes `e` sees: an argument of an inlined view is written in
    /// the caller, so it sees only the scopes outside the view (`DeviceRow(device: device)`).
    fn path_in(&self, e: &Expr, depth: usize, scopes: usize) -> Option<String> {
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

    fn token(&self, e: &Expr) -> Option<String> {
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

    fn bind(&self, e: &Expr, not: bool) -> Option<Value> {
        Some(Value::Bind {
            bind: self.path(e)?,
            not,
        })
    }

    /// Reads a value used as text, a flag or a number, undoing the conversions `generate` adds.
    fn value(&mut self, e: &Expr, leaf: Leaf, path: &str) -> Option<Value> {
        self.value_inner(e, leaf, path, 0)
    }

    fn value_inner(&mut self, e: &Expr, leaf: Leaf, path: &str, depth: usize) -> Option<Value> {
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
    fn interpolated(&mut self, parts: &[Part], leaf: Leaf, path: &str) -> Option<Value> {
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

    fn declares_event(&self, kind: &str, event: &str) -> bool {
        self.catalog
            .components
            .get(kind)
            .and_then(|c| c.events.as_ref())
            .is_some_and(|e| e.iter().any(|x| x == event))
    }

    fn declares_prop(&self, kind: &str, prop: &str) -> bool {
        self.catalog
            .components
            .get(kind)
            .is_some_and(|c| c.prop(prop).is_some())
    }

    // ---- Handlers ----

    fn handler(&mut self, statements: &[Stmt]) -> Handler {
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
    fn event(&mut self, node: &mut Node, event: &str, statements: &[Stmt], path: &str) -> Handler {
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

    // ---- Views ----

    fn screen(&mut self, statements: &[Stmt]) -> Node {
        let mut root = Node::new("screen");
        // The generated root, or any single stack, is the screen itself.
        let single = match statements {
            [Stmt::Expr(e)] => Some(e),
            _ => None,
        };
        let mut e = single;
        while let Some(expr) = e {
            let (base, _) = unwind(expr);
            let name = match base {
                Expr::Call(c) => type_name(&c.callee),
                other => type_name(other),
            };
            match name.as_deref() {
                Some("NavigationStack" | "NavigationView" | "ScrollView") => {
                    let Expr::Call(c) = base else { break };
                    let inner = closure(&c.closures, None).map(|c| c.body.as_slice());
                    e = match inner {
                        Some([Stmt::Expr(inner)]) => Some(inner),
                        _ => break,
                    };
                }
                _ => break,
            }
        }
        if let Some(expr) = e {
            let (base, mods) = unwind(expr);
            if let Expr::Call(c) = base
                && ident(&c.callee) == Some("VStack")
            {
                let mut mods = Mods(mods);
                let explicit = self.take_explicit_id(&mut mods);
                let path = "/screen".to_owned();
                root.id = Some(self.take_id(explicit, "screen", "", &path));
                let path = element_path("", &root);
                let generated = matches!(implicit(arg(&c.args, "alignment")), Some("leading"))
                    && c.args.len() == 1;
                if !generated && !c.args.is_empty() {
                    self.lose(
                        LossKind::Layout,
                        &path,
                        "the root stack's alignment and spacing are not kept",
                    );
                }
                let body = closure(&c.closures, None)
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                self.depth = 1;
                root.children = self.views(&body, Place::Nodes, &path);
                self.common(&mut root, mods, &path);
                return root;
            }
        }
        root.id = Some(self.take_id(None, "screen", "", "/screen"));
        let path = element_path("", &root);
        self.lose(
            LossKind::Structure,
            &path,
            "the view's body is not one stack; a screen root is added around it",
        );
        self.depth = 1;
        root.children = self.views(statements, Place::Nodes, &path);
        root
    }

    fn take_explicit_id(&mut self, mods: &mut Mods) -> Option<String> {
        let m = mods.take("accessibilityIdentifier")?;
        match first_arg(&m.args) {
            Some(Expr::Str(p)) => plain_string(p),
            _ => None,
        }
    }

    /// The views of a view builder, in order.
    fn views(&mut self, statements: &[Stmt], place: Place, path: &str) -> Vec<Child> {
        let mut out = vec![];
        for s in statements {
            if self.truncated {
                break;
            }
            match s {
                Stmt::Expr(e) => out.extend(self.view(e, place, path)),
                Stmt::If {
                    condition,
                    then,
                    otherwise,
                } => out.extend(self.conditional(condition, then, otherwise, place, path)),
                Stmt::Other(kind) => {
                    self.lose(
                        LossKind::Structure,
                        path,
                        format!("a `{kind}` in a view builder is not read"),
                    );
                }
            }
        }
        out
    }

    /// `if c { a } else { b }`: both branches, each hidden while its condition is false.
    fn conditional(
        &mut self,
        condition: &Expr,
        then: &[Stmt],
        otherwise: &[Stmt],
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let cond = match self.value(condition, Leaf::Bool, path) {
            Some(Value::Bind { bind, not }) => Some((bind, not)),
            _ => None,
        };
        if cond.is_none() {
            self.lose(LossKind::Hidden, path, "an `if` condition that is not a data path is not kept; its first branch is imported as shown");
        }
        let mut out = vec![];
        for (branch, shown_when) in [(then, false), (otherwise, true)] {
            if branch.is_empty() {
                continue;
            }
            if cond.is_none() && shown_when {
                break;
            }
            for mut child in self.views(branch, place, path) {
                if let (Some((bind, not)), Child::Node(n)) = (&cond, &mut child) {
                    if n.props.contains_key("hidden") {
                        self.lose(
                            LossKind::Hidden,
                            &element_path(path, n),
                            "an element hidden both by `if` and by its own condition keeps its own",
                        );
                    } else {
                        // Hidden when the branch's condition does not hold.
                        n.props.insert(
                            "hidden".to_owned(),
                            Value::Bind {
                                bind: bind.clone(),
                                not: !(*not ^ shown_when),
                            },
                        );
                    }
                }
                out.push(child);
            }
        }
        out
    }

    fn enter(&mut self) -> bool {
        self.elements += 1;
        if self.elements > MAX_NODES || self.depth > MAX_DEPTH {
            self.truncated = true;
            return false;
        }
        true
    }

    fn view(&mut self, expr: &Expr, place: Place, path: &str) -> Vec<Child> {
        let (base, mods) = unwind(expr);
        let mut mods = Mods(mods);
        let (name, call) = match base {
            Expr::Call(c) => (type_name(&c.callee).unwrap_or_default(), Some(c)),
            other => (type_name(other).unwrap_or_default(), None),
        };
        let empty = Call {
            callee: Box::new(Expr::Other(String::new())),
            args: vec![],
            closures: vec![],
        };
        let call = call.unwrap_or(&empty);
        // A sheet presented from the view is a dialog next to it.
        let sheets = mods.take_all("sheet");
        let alerts: Vec<Mod> = ["alert", "confirmationDialog"]
            .iter()
            .flat_map(|n| mods.take_all(n))
            .collect();
        let anchor = name == "Color.clear" && !sheets.is_empty();
        let mut out = if anchor {
            vec![]
        } else {
            self.base_view(&name, call, &mut mods, place, path)
        };
        let anchor_hidden = if anchor {
            mods.take("weftHidden").or_else(|| mods.take("hidden"))
        } else {
            None
        };
        if anchor {
            mods.take("frame");
            self.ignored(mods, path);
        }
        for sheet in sheets {
            if let Some(dialog) = self.sheet(&sheet, anchor_hidden.as_ref(), path) {
                out.push(Child::Node(Box::new(dialog)));
            }
        }
        for alert in alerts {
            if let Some(dialog) = self.alert(&alert, path) {
                out.push(Child::Node(Box::new(dialog)));
            }
        }
        out
    }

    fn base_view(
        &mut self,
        name: &str,
        call: &Call,
        mods: &mut Mods,
        place: Place,
        path: &str,
    ) -> Vec<Child> {
        let mods = Mods(std::mem::take(&mut mods.0));
        let children = |c: &Call| {
            closure(&c.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default()
        };
        match (name, place) {
            ("ForEach", _) => self.each(call, mods, place, path),
            (
                "Group" | "NavigationStack" | "NavigationView" | "ScrollView" | "LazyVStack"
                | "ZStack",
                _,
            ) => {
                if name != "Group" {
                    self.lose(
                        LossKind::Layout,
                        path,
                        format!("`{name}` is not kept; its content is"),
                    );
                }
                self.ignored(mods, path);
                self.views(&children(call), place, path)
            }
            ("Spacer" | "Divider" | "Color.clear", _) => {
                self.lose(LossKind::Layout, path, format!("`{name}` is not kept"));
                vec![]
            }
            ("EmptyView", _) => vec![],
            ("Text" | "Label", Place::Choice(kind)) => self.text_kind(kind, call, mods, path),
            ("Text" | "Label", Place::Header) => self.text_kind("column", call, mods, path),
            ("Button", Place::Menu) => self.button("menu-item", call, mods, path),
            (_, Place::List) if name != "HStack" => self.wrapped("item", name, call, mods, path),
            (_, Place::Row) if name != "HStack" => self.wrapped("cell", name, call, mods, path),
            ("HStack", Place::List) => self.mixed("item", call, mods, path),
            ("HStack", Place::Row) => self.mixed("cell", call, mods, path),
            ("GridRow", Place::Table) => self.grid_row(call, mods, path),
            (_, Place::Table) => self.wrapped("row", name, call, mods, path),
            (_, Place::Tabs) => self.tab(name, call, mods, path),
            (_, Place::Choice(_) | Place::Menu | Place::Header) => {
                self.lose(
                    LossKind::Structure,
                    path,
                    format!("`{name}` cannot go here in Weft and is dropped"),
                );
                vec![]
            }
            _ => self.element_view(name, call, mods, path),
        }
    }

    fn element_view(&mut self, name: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        match name {
            "HStack" if mods.has("weftCombobox") => self.combobox(call, mods, path),
            "VStack" | "HStack" | "LazyHStack" => self.stack(name, call, mods, path),
            "LazyVGrid" | "LazyHGrid" => self.grid(call, mods, path),
            "Grid" => self.container("table", call, mods, Place::Table, path),
            "Section" => self.section(call, mods, path),
            "GroupBox"
                if call.args.is_empty() && closure(&call.closures, Some("label")).is_none() =>
            {
                self.mixed("alert", call, mods, path)
            }
            "GroupBox" => self.section(call, mods, path),
            "Text" | "Label" => {
                let heading = mods.has("accessibilityHeading")
                    || mods.0.iter().any(|m| {
                        m.name == "accessibilityAddTraits"
                            && implicit(first_arg(&m.args)) == Some("isHeader")
                    });
                let font = mods
                    .0
                    .iter()
                    .find(|m| m.name == "font")
                    .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
                let title_font = matches!(
                    font.as_deref(),
                    Some("largeTitle" | "title" | "title2" | "title3")
                );
                if title_font && !heading {
                    self.lose(
                        LossKind::Props,
                        path,
                        "a text with a title font is read as a heading",
                    );
                }
                let kind = if heading || title_font {
                    "heading"
                } else {
                    "text"
                };
                self.text_kind(kind, call, mods, path)
            }
            "Image" | "AsyncImage" => self.image(name, call, mods, path),
            "Button" => {
                let link = mods.0.iter().any(|m| {
                    m.name == "accessibilityAddTraits"
                        && implicit(first_arg(&m.args)) == Some("isLink")
                });
                self.button(if link { "link" } else { "button" }, call, mods, path)
            }
            "Link" => self.button("link", call, mods, path),
            "Form" => self.form(call, mods, path),
            "TextField" | "SecureField" | "TextEditor" => self.field(name, call, mods, path),
            "Toggle" => self.toggle(call, mods, path),
            "Slider" => self.slider(call, mods, path),
            "Stepper" => self.stepper(call, mods, path),
            "DatePicker" => self.date_picker(call, mods, path),
            "ColorPicker" => self.color_picker(call, mods, path),
            "Picker" => self.picker(call, mods, path),
            "List" => self.list(call, mods, path),
            "TabView" => self.tabs(call, mods, path),
            "Menu" => self.menu(call, mods, path),
            _ => {
                if let Some(kind) = self.custom.get(name).cloned() {
                    return self.custom_view(&kind, call, mods, path);
                }
                if let Some(body) = self.views.get(name).copied()
                    && !self.inlining.iter().any(|v| v == name)
                {
                    // A helper view declared in the same file is read in place, its arguments
                    // standing for its properties.
                    let aliases = call
                        .args
                        .iter()
                        .filter_map(|a| Some((a.label.clone()?, a.value.clone())))
                        .collect();
                    self.aliases.push(aliases);
                    self.inlining.push(name.to_owned());
                    let out = self.views(body, Place::Nodes, path);
                    self.inlining.pop();
                    self.aliases.pop();
                    self.ignored_quiet(&mut mods);
                    self.ignored(mods, path);
                    return out;
                }
                let shown = if name.is_empty() {
                    "an expression"
                } else {
                    name
                };
                self.lose(
                    LossKind::Kinds,
                    path,
                    format!("`{shown}` has no Weft kind and is dropped"),
                );
                vec![]
            }
        }
    }

    /// A new element: id from the modifiers, then the shared modifiers once its content is read.
    fn element(
        &mut self,
        kind: &str,
        mods: &mut Mods,
        name: &str,
        path: &str,
    ) -> Option<(Node, String)> {
        if !self.enter() {
            return None;
        }
        let explicit = self.take_explicit_id(mods);
        let mut node = Node::new(kind);
        let here = format!("{path}/{kind}");
        node.id = Some(self.take_id(explicit, kind, name, &here));
        let here = element_path(path, &node);
        Some((node, here))
    }

    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.depth += 1;
        let out = f(self);
        self.depth -= 1;
        out
    }

    /// Modifiers every kind shares: label, hidden, markers and handlers, then losses for the rest.
    fn common(&mut self, node: &mut Node, mut mods: Mods, path: &str) {
        if let Some(m) = mods.take("accessibilityLabel")
            && let Some(e) = first_arg(&m.args)
        {
            match self.value(e, Leaf::Text, path) {
                Some(v) => {
                    node.props.insert("label".to_owned(), v);
                }
                None => self.lose(
                    LossKind::Names,
                    path,
                    "the accessibility label is not text or a data path",
                ),
            }
        }
        for name in ["weftHidden", "hidden"] {
            if let Some(m) = mods.take(name) {
                let v = match first_arg(&m.args) {
                    Some(e) => self.value(e, Leaf::Bool, path),
                    None => Some(Value::Bool(true)),
                };
                if let Some(v) = v {
                    node.props.insert("hidden".to_owned(), v);
                }
            }
        }
        for m in mods.take_all("rotation3DEffect") {
            self.tilt(node, &m.args, path);
        }
        if let Some(m) = mods.take("disabled")
            && let Some(v) = first_arg(&m.args).and_then(|e| self.value(e, Leaf::Bool, path))
        {
            if self.declares_prop(&node.kind, "disabled") {
                node.props.insert("disabled".to_owned(), v);
            } else {
                self.lose(
                    LossKind::Props,
                    path,
                    format!("`{}` cannot be disabled in Weft", node.kind),
                );
            }
        }
        for (modifier, event) in [
            ("onTapGesture", "press"),
            ("onSubmit", "submit"),
            ("onChange", "change"),
        ] {
            for m in mods.take_all(modifier) {
                let body = m
                    .closures
                    .first()
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                self.event(node, event, &body, path);
            }
        }
        for m in mods.take_all("weftProp") {
            let (Some(Expr::Str(name)), Some(e)) = (
                m.args.first().map(|a| &a.value),
                m.args.get(1).map(|a| &a.value),
            ) else {
                continue;
            };
            let Some(name) = plain_string(name) else {
                continue;
            };
            let Some((leaf, _)) = prop_leaf(self.catalog, &node.kind, &name) else {
                self.lose(
                    LossKind::Props,
                    path,
                    format!("`{}` has no `{name}` attribute", node.kind),
                );
                continue;
            };
            match self.value(e, leaf, path) {
                Some(v) => {
                    node.props.insert(name, v);
                }
                None => self.lose(
                    LossKind::Bindings,
                    path,
                    format!("the value of `{name}` is not a literal or a data path"),
                ),
            }
        }
        self.ignored_quiet(&mut mods);
        self.ignored(mods, path);
    }

    /// One `rotation3DEffect` of a tilt: the turn about an axis, and the perspective in pixels.
    fn tilt(&mut self, node: &mut Node, args: &[Arg], path: &str) {
        let number = |e: &Expr| match e {
            Expr::Num(n) => Some(*n),
            Expr::Neg(inner) => match inner.as_ref() {
                Expr::Num(n) => Some(-n),
                _ => None,
            },
            _ => None,
        };
        let degrees = match first_arg(args) {
            Some(Expr::Call(c)) if matches!(c.callee.as_ref(), Expr::Implicit(n) if n == "degrees") => {
                first_arg(&c.args).and_then(number)
            }
            _ => None,
        };
        let axis = match arg(args, "axis") {
            Some(Expr::Tuple(items)) => ["x", "y", "z"].map(|name| {
                items
                    .iter()
                    .find(|a| a.label.as_deref() == Some(name))
                    .and_then(|a| number(&a.value))
            }),
            _ => [None; 3],
        };
        let perspective = match arg(args, "perspective") {
            Some(Expr::Num(n)) if *n == 0.0 => Some(None),
            Some(Expr::Binary { left, op, right })
                if op == "/"
                    && matches!(left.as_ref(), Expr::Num(n) if *n == crate::generate::TILT_NOMINAL_VIEW) =>
            {
                number(right).map(Some)
            }
            _ => None,
        };
        let (Some(degrees), [Some(x), Some(y), Some(z)], Some(perspective)) =
            (degrees, axis, perspective)
        else {
            self.lose(
                LossKind::Props,
                path,
                "a rotation3DEffect is not a tilt Weft can read",
            );
            return;
        };
        if let Some(p) = perspective {
            node.props
                .insert("perspective".to_owned(), Value::Number(p));
        }
        let name = match (x, y, z) {
            (0.0, 0.0, 0.0) => return,
            (1.0, 0.0, 0.0) => "rotate-x",
            (0.0, 1.0, 0.0) => "rotate-y",
            (0.0, 0.0, 1.0) => "rotate-z",
            _ => {
                self.lose(
                    LossKind::Props,
                    path,
                    "a rotation3DEffect turns about an axis Weft has no attribute for",
                );
                return;
            }
        };
        node.props.insert(name.to_owned(), Value::Number(degrees));
    }

    /// Modifiers `generate` adds for SwiftUI's sake, which carry nothing more.
    fn ignored_quiet(&mut self, mods: &mut Mods) {
        mods.0.retain(|m| {
            let trait_arg = implicit(first_arg(&m.args));
            !matches!(
                (m.name.as_str(), trait_arg),
                ("accessibilityElement", _)
                    | (
                        "accessibilityAddTraits",
                        Some("isHeader" | "isModal" | "isLink")
                    )
                    | ("accessibilityRemoveTraits", Some("isButton"))
            )
        });
    }

    fn ignored(&mut self, mods: Mods, path: &str) {
        let mut layout = vec![];
        let mut other = vec![];
        for m in mods.0 {
            if LAYOUT_MODIFIERS.contains(&m.name.as_str()) {
                if !layout.contains(&m.name) {
                    layout.push(m.name);
                }
            } else if !other.contains(&m.name) {
                other.push(m.name);
            }
        }
        if !layout.is_empty() {
            let list: Vec<String> = layout.iter().map(|n| format!("`.{n}`")).collect();
            self.lose(
                LossKind::Layout,
                path,
                format!("{} not kept", list.join(", ")),
            );
        }
        if !other.is_empty() {
            let list: Vec<String> = other.iter().map(|n| format!("`.{n}`")).collect();
            self.lose(
                LossKind::Props,
                path,
                format!("{} has no Weft form", list.join(", ")),
            );
        }
    }

    /// A stack's or grid's `.modifier(theme.<path>)`: the token is its material. Any other use of
    /// `modifier` stays for `ignored` to report.
    fn material(&mut self, node: &mut Node, mods: &mut Mods) {
        for m in mods.take_all("modifier") {
            match first_arg(&m.args).and_then(|e| self.token(e)) {
                Some(t) => {
                    node.props.insert("material".to_owned(), Value::Token(t));
                }
                None => mods.0.push(m),
            }
        }
    }

    fn finish(&mut self, mut node: Node, mods: Mods, path: &str) -> Vec<Child> {
        self.common(&mut node, mods, path);
        vec![Child::Node(Box::new(node))]
    }

    // ---- Kinds ----

    fn container(
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
    fn empty_slot(&mut self, node: &mut Node, mods: &mut Mods, path: &str) {
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
    fn elements_only(&mut self, children: Vec<Child>, path: &str) -> Vec<Child> {
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

    fn stack(&mut self, name: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("stack", &mut mods, "", path) else {
            return vec![];
        };
        let row = name != "VStack";
        if row {
            node.props
                .insert("direction".to_owned(), Value::String("row".to_owned()));
        }
        for a in &call.args {
            match (a.label.as_deref(), &a.value) {
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
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        self.material(&mut node, &mut mods);
        self.finish(node, mods, &here)
    }

    fn grid(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("grid", &mut mods, "", path) else {
            return vec![];
        };
        let columns = arg(&call.args, "columns").or_else(|| arg(&call.args, "rows"));
        let count = match columns {
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
        self.material(&mut node, &mut mods);
        self.finish(node, mods, &here)
    }

    fn section(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
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

    /// The text a text-content kind shows: content runs, or the `text` prop.
    fn text_content(&mut self, node: &mut Node, call: &Call, path: &str) {
        if let Some(e) = arg(&call.args, "verbatim") {
            if let Some(v) = self.value(e, Leaf::Text, path) {
                node.props.insert("text".to_owned(), v);
            }
            return;
        }
        let first = first_arg(&call.args).or_else(|| arg(&call.args, "title"));
        match first {
            Some(Expr::Str(parts)) => match plain_string(parts) {
                Some(s) if s.is_empty() => {}
                Some(s) => {
                    let s = self.string(&s, path);
                    node.children.push(Child::Text(s));
                }
                None => match self.interpolated(parts, Leaf::Text, path) {
                    Some(v @ Value::Bind { .. }) => {
                        node.props.insert("text".to_owned(), v);
                    }
                    Some(Value::String(s)) if !s.is_empty() => node.children.push(Child::Text(s)),
                    _ => {}
                },
            },
            Some(e) => match self.value(e, Leaf::Text, path) {
                Some(v) => {
                    node.props.insert("text".to_owned(), v);
                }
                None => self.lose(
                    LossKind::Text,
                    path,
                    "text that is not a literal or a data path is not kept",
                ),
            },
            None => {
                // `Button { } label: { Text("…") }` and `Label { Text("…") } icon: { }`.
                let label = closure(&call.closures, Some("label"))
                    .or_else(|| {
                        let only = (call.closures.len() == 1)
                            .then(|| call.closures.first())
                            .flatten()?;
                        let action = call
                            .args
                            .iter()
                            .any(|a| a.label.as_deref() == Some("action"));
                        (only.label.is_none() && action).then_some(only)
                    })
                    .or_else(|| closure(&call.closures, Some("title")));
                if let Some(label) = label {
                    let mut texts = vec![];
                    for s in &label.body {
                        let Stmt::Expr(e) = s else { continue };
                        let (base, mods) = unwind(e);
                        if let Expr::Call(c) = base
                            && matches!(ident(&c.callee), Some("Text" | "Label"))
                        {
                            texts.push(c.clone());
                            if !mods.is_empty() {
                                self.lose(
                                    LossKind::Layout,
                                    path,
                                    "modifiers of a label's text are not kept",
                                );
                            }
                        } else {
                            self.lose(
                                LossKind::Text,
                                path,
                                "a label view that is not text is not kept",
                            );
                        }
                    }
                    for c in texts {
                        self.text_content(node, &c, path);
                    }
                }
            }
        }
    }

    fn text_kind(&mut self, kind: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let name = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, &name, path) else {
            return vec![];
        };
        self.text_content(&mut node, call, &here);
        if ident(&call.callee) == Some("Label") && arg(&call.args, "systemImage").is_some() {
            self.lose(LossKind::Props, &here, "the label's icon is not kept");
        }
        match kind {
            "heading" => {
                let mut level = None;
                if let Some(m) = mods.take("accessibilityHeading") {
                    level = match first_arg(&m.args) {
                        Some(Expr::Implicit(h)) => h
                            .strip_prefix('h')
                            .and_then(|n| n.parse::<u8>().ok())
                            .filter(|n| (1..=6).contains(n))
                            .map(|n| Value::Number(f64::from(n))),
                        Some(Expr::Call(c)) if ident(&c.callee) == Some("weftHeadingLevel") => {
                            first_arg(&c.args).and_then(|e| self.value(e, Leaf::Int, &here))
                        }
                        _ => None,
                    };
                }
                if let Some(m) = mods.take("font") {
                    let from_font = match implicit(first_arg(&m.args)) {
                        Some("largeTitle") => Some(1.0),
                        Some("title") => Some(2.0),
                        Some("title2") => Some(3.0),
                        Some("title3") => Some(4.0),
                        Some("headline") => Some(5.0),
                        Some("subheadline") => Some(6.0),
                        _ => None,
                    };
                    if level.is_none() {
                        level = from_font.map(Value::Number);
                    }
                }
                mods.take_flag("accessibilityAddTraits", "isHeader");
                match level {
                    Some(l) => {
                        node.props.insert("level".to_owned(), l);
                    }
                    None => {
                        self.lose(
                            LossKind::Values,
                            &here,
                            "the heading level is not read; level 2 stands in",
                        );
                        node.props.insert("level".to_owned(), Value::Number(2.0));
                    }
                }
            }
            "text" => {
                let style = mods
                    .take("foregroundStyle")
                    .or_else(|| mods.take("foregroundColor"));
                if let Some(m) = style {
                    let tone = match implicit(first_arg(&m.args)) {
                        Some("primary") => Some("default"),
                        Some("secondary") => Some("muted"),
                        Some("green") => Some("success"),
                        Some("orange" | "yellow") => Some("warning"),
                        Some("red") => Some("danger"),
                        _ => None,
                    };
                    match tone {
                        Some(t) => {
                            node.props
                                .insert("tone".to_owned(), Value::String(t.to_owned()));
                        }
                        None => self.lose(
                            LossKind::Layout,
                            &here,
                            "a text color with no Weft tone is not kept",
                        ),
                    }
                }
            }
            "column" => {
                mods.take_flag("font", "headline");
            }
            "radio" | "option" | "segment" => {
                match mods.take("tag").and_then(|m| first_arg(&m.args).cloned()) {
                    Some(e) => {
                        if let Some(v) = self.value(&e, Leaf::Text, &here) {
                            node.props.insert("value".to_owned(), v);
                        }
                    }
                    None => self.lose(
                        LossKind::Values,
                        &here,
                        "a choice without `.tag` gets its text as value",
                    ),
                }
            }
            _ => {}
        }
        self.finish(node, mods, &here)
    }

    fn image(&mut self, name: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("image", &mut mods, "", path) else {
            return vec![];
        };
        let src = match name {
            "AsyncImage" => arg(&call.args, "url").and_then(|e| match e {
                Expr::Call(c) if ident(&c.callee) == Some("URL") => arg(&c.args, "string").cloned(),
                other => Some(other.clone()),
            }),
            _ => first_arg(&call.args)
                .or_else(|| arg(&call.args, "systemName"))
                .cloned(),
        };
        match src.and_then(|e| self.value(&e, Leaf::Text, &here)) {
            Some(v) => {
                if name == "Image" {
                    self.lose(
                        LossKind::Values,
                        &here,
                        "an asset or symbol name is kept as the image source",
                    );
                }
                node.props.insert("src".to_owned(), v);
            }
            None => self.lose(LossKind::Values, &here, "the image source is not read"),
        }
        self.finish(node, mods, &here)
    }

    fn button(&mut self, kind: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let title = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, &title, path) else {
            return vec![];
        };
        self.text_content(&mut node, call, &here);
        // The action: the unlabeled trailing closure, or `action:`, unless that closure is the label.
        let action = match (first_arg(&call.args), arg(&call.args, "action")) {
            (_, Some(Expr::Closure(c))) => Some(c.body.clone()),
            (Some(_), None) => closure(&call.closures, None).map(|c| c.body.clone()),
            (None, None) if closure(&call.closures, Some("label")).is_some() => {
                closure(&call.closures, None).map(|c| c.body.clone())
            }
            _ => None,
        };
        if let Some(body) = action {
            let h = self.event(&mut node, "press", &body, &here);
            if h.submit.is_some() && self.declares_prop(kind, "submit") {
                node.props.insert("submit".to_owned(), Value::Bool(true));
            }
            if let Some(href) = h.href
                && let Some(v) = self.value(&href, Leaf::Text, &here)
            {
                node.props.insert("href".to_owned(), v);
            }
        }
        if name_is(call, "Link")
            && let Some(dest) = arg(&call.args, "destination")
        {
            let dest = match dest {
                Expr::Call(c) if ident(&c.callee) == Some("URL") => arg(&c.args, "string").cloned(),
                other => Some(other.clone()),
            };
            if let Some(v) = dest.and_then(|e| self.value(&e, Leaf::Text, &here)) {
                node.props.insert("href".to_owned(), v);
            }
        }
        if kind == "button" {
            let danger = matches!(implicit(arg(&call.args, "role")), Some("destructive"));
            let style = mods
                .take("buttonStyle")
                .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
            let variant = match (danger, style.as_deref()) {
                (true, _) => Some("danger"),
                (false, Some("borderedProminent")) => Some("primary"),
                (false, Some("bordered")) => Some("secondary"),
                _ => None,
            };
            if let Some(v) = variant {
                node.props
                    .insert("variant".to_owned(), Value::String(v.to_owned()));
            }
        } else if kind == "link" {
            mods.take_flag("buttonStyle", "borderless");
        }
        self.finish(node, mods, &here)
    }

    fn form(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("form", &mut mods, "", path) else {
            return vec![];
        };
        let mut body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let mut footer = None;
        // `Form { Section { … } footer: { … } }` is a form with a footer slot.
        if let [Stmt::Expr(e)] = body.as_slice()
            && let Expr::Call(c) = e
            && ident(&c.callee) == Some("Section")
            && c.args.is_empty()
            && let Some(f) = closure(&c.closures, Some("footer"))
            && closure(&c.closures, Some("header")).is_none()
        {
            footer = Some(f.body.clone());
            body = closure(&c.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default();
        }
        let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        if let Some(f) = footer {
            let views = self.nested(|r| r.views(&f, Place::Nodes, &format!("{here}/slot[footer]")));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert("footer".to_owned(), views);
            }
        }
        self.finish(node, mods, &here)
    }

    /// The caption of a control: its first argument or `label:` closure.
    fn caption(&mut self, node: &mut Node, call: &Call, path: &str) {
        let label = first_arg(&call.args).cloned().or_else(|| {
            let c = closure(&call.closures, Some("label")).or_else(|| {
                (call.closures.len() == 1)
                    .then(|| call.closures.first())
                    .flatten()
            })?;
            match c.body.as_slice() {
                [Stmt::Expr(e)] => {
                    let (base, _) = unwind(e);
                    match base {
                        Expr::Call(t) if matches!(ident(&t.callee), Some("Text" | "Label")) => {
                            first_arg(&t.args).cloned()
                        }
                        _ => None,
                    }
                }
                _ => None,
            }
        });
        if let Some(e) = label
            && let Some(v) = self.value(&e, Leaf::Text, path)
        {
            node.props.insert("label".to_owned(), v);
        }
    }

    /// A control's binding: data it writes, or a constant that reads as a literal.
    fn writes(&mut self, node: &mut Node, prop: &str, e: Option<&Expr>, leaf: Leaf, path: &str) {
        let Some(e) = e else { return };
        let v = match e {
            Expr::Call(c) if matches!(c.callee.as_ref(), Expr::Implicit(n) if n == "constant") => {
                match first_arg(&c.args).and_then(|a| self.value(a, leaf, path)) {
                    // A blank constant is what `generate` prints for an absent prop.
                    Some(Value::String(s)) if s.is_empty() => None,
                    Some(Value::Bool(false)) => None,
                    Some(Value::Number(n)) if n == 0.0 && leaf == Leaf::Double => None,
                    other => other,
                }
            }
            other => match self.bind(other, false) {
                Some(v) => Some(v),
                None => {
                    self.lose(
                        LossKind::Bindings,
                        path,
                        format!("the `{prop}` binding is not a data path"),
                    );
                    None
                }
            },
        };
        if let Some(v) = v {
            node.props.insert(prop.to_owned(), v);
        }
    }

    fn field(&mut self, name: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let title = match first_arg(&call.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let Some((mut node, here)) = self.element("field", &mut mods, &title, path) else {
            return vec![];
        };
        if name != "TextEditor" {
            self.caption(&mut node, call, &here);
        }
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "text"),
            Leaf::Text,
            &here,
        );
        self.placeholder(&mut node, call, &here);
        let mut field_type = match name {
            "SecureField" => Some(Value::String("password".to_owned())),
            "TextEditor" => Some(Value::String("multiline".to_owned())),
            _ if implicit(arg(&call.args, "axis")) == Some("vertical") => {
                Some(Value::String("multiline".to_owned()))
            }
            _ => None,
        };
        if let Some(m) = mods.take("weftFieldType") {
            field_type = first_arg(&m.args).and_then(|e| self.value(e, Leaf::Text, &here));
        }
        for modifier in ["keyboardType", "textContentType"] {
            if let Some(m) = mods.take(modifier) {
                let t = match implicit(first_arg(&m.args)) {
                    Some("emailAddress") => Some("email"),
                    Some("numberPad" | "decimalPad" | "numbersAndPunctuation") => Some("number"),
                    Some("password" | "newPassword") => Some("password"),
                    _ => None,
                };
                if field_type.is_none()
                    && let Some(t) = t
                {
                    field_type = Some(Value::String(t.to_owned()));
                }
            }
        }
        if let Some(t) = field_type {
            node.props.insert("type".to_owned(), t);
        }
        mods.take("textInputAutocapitalization");
        mods.take("autocorrectionDisabled");
        self.finish(node, mods, &here)
    }

    /// `prompt: Text(…)` is the placeholder.
    fn placeholder(&mut self, node: &mut Node, call: &Call, path: &str) {
        if let Some(Expr::Call(p)) = arg(&call.args, "prompt")
            && let Some(e) = first_arg(&p.args)
            && let Some(v) = self.value(e, Leaf::Text, path)
        {
            node.props.insert("placeholder".to_owned(), v);
        }
    }

    /// A number prop read from `e`; `nil` and anything else that is not a number is absent.
    fn number(&mut self, node: &mut Node, prop: &str, e: Option<&Expr>, path: &str) {
        if let Some(v) = e.and_then(|e| self.value(e, Leaf::Double, path)) {
            node.props.insert(prop.to_owned(), v);
        }
    }

    /// The two ends of `lo...hi`, or of the helper that keeps a range from being reversed.
    fn range(e: Option<&Expr>) -> (Option<&Expr>, Option<&Expr>) {
        match e {
            Some(Expr::Binary { left, op, right }) if op == "..." => {
                (Some(left.as_ref()), Some(right.as_ref()))
            }
            Some(Expr::Call(c)) if matches!(ident(&c.callee), Some("weftRange" | "weftBounds")) => {
                let at = |i: usize| c.args.get(i).map(|a| &a.value);
                (at(0), at(1))
            }
            _ => (None, None),
        }
    }

    fn slider(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("slider", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "value"),
            Leaf::Double,
            &here,
        );
        let (lo, hi) = Self::range(arg(&call.args, "in"));
        self.number(&mut node, "min", lo, &here);
        self.number(&mut node, "max", hi, &here);
        self.number(&mut node, "step", arg(&call.args, "step"), &here);
        self.finish(node, mods, &here)
    }

    fn stepper(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("stepper", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "value"),
            Leaf::Double,
            &here,
        );
        let (lo, hi) = Self::range(arg(&call.args, "in"));
        self.number(&mut node, "min", lo, &here);
        self.number(&mut node, "max", hi, &here);
        self.number(&mut node, "step", arg(&call.args, "step"), &here);
        self.finish(node, mods, &here)
    }

    fn date_picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("date-picker", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        mods.take("weftUTC");
        // `weftDate(binding, type)` is what `generate` prints; a plain `Date` binding is not data.
        let selection = arg(&call.args, "selection");
        let (binding, kind) = match selection {
            // The helper leaves an absent `type` out, so the second argument is the `type` prop.
            Some(Expr::Call(c)) if ident(&c.callee) == Some("weftDate") => (
                c.args.first().map(|a| &a.value),
                c.args
                    .get(1)
                    .and_then(|a| self.value(&a.value, Leaf::Text, &here)),
            ),
            other => {
                let components = Self::components(arg(&call.args, "displayedComponents"));
                (
                    other,
                    components.filter(|k| *k != Value::String("date".to_owned())),
                )
            }
        };
        self.writes(&mut node, "value", binding, Leaf::Text, &here);
        if let Some(kind) = kind {
            node.props.insert("type".to_owned(), kind);
        }
        if let Some(Expr::Call(c)) = arg(&call.args, "in")
            && ident(&c.callee) == Some("weftDates")
        {
            for (prop, i) in [("min", 0), ("max", 1)] {
                if let Some(v) = c
                    .args
                    .get(i)
                    .and_then(|a| self.value(&a.value, Leaf::Text, &here))
                    && v != Value::String(String::new())
                {
                    node.props.insert(prop.to_owned(), v);
                }
            }
        }
        self.finish(node, mods, &here)
    }

    /// The `type` a hand-written `displayedComponents: [.date, .hourAndMinute]` stands for.
    fn components(e: Option<&Expr>) -> Option<Value> {
        let names: Vec<&str> = match e? {
            Expr::Array(items) => items.iter().filter_map(|i| implicit(Some(i))).collect(),
            other => implicit(Some(other)).into_iter().collect(),
        };
        let kind = match (names.contains(&"date"), names.contains(&"hourAndMinute")) {
            (true, true) => "datetime",
            (false, true) => "time",
            _ => "date",
        };
        Some(Value::String(kind.to_owned()))
    }

    fn color_picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("color-picker", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        let binding = match arg(&call.args, "selection") {
            Some(Expr::Call(c)) if ident(&c.callee) == Some("weftColor") => {
                c.args.first().map(|a| &a.value)
            }
            other => other,
        };
        self.writes(&mut node, "value", binding, Leaf::Text, &here);
        self.finish(node, mods, &here)
    }

    /// The text field and the menu of options that `generate` prints for a combobox.
    fn combobox(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        mods.take("weftCombobox");
        let Some((mut node, here)) = self.element("combobox", &mut mods, "", path) else {
            return vec![];
        };
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let calls: Vec<&Call> = body
            .iter()
            .filter_map(|s| match s {
                Stmt::Expr(Expr::Call(c)) => Some(c),
                _ => None,
            })
            .collect();
        if let Some(field) = calls.iter().find(|c| name_is(c, "TextField")) {
            self.caption(&mut node, field, &here);
            self.writes(
                &mut node,
                "value",
                arg(&field.args, "text"),
                Leaf::Text,
                &here,
            );
            self.placeholder(&mut node, field, &here);
        }
        let options = calls
            .iter()
            .find(|c| name_is(c, "Menu"))
            .and_then(|m| closure(&m.closures, None))
            .and_then(|c| {
                c.body.iter().find_map(|s| match s {
                    Stmt::Expr(Expr::Call(p)) if name_is(p, "Picker") => {
                        closure(&p.closures, None).map(|c| c.body.clone())
                    }
                    _ => None,
                })
            })
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&options, Place::Choice("option"), &here));
        self.finish(node, mods, &here)
    }

    fn toggle(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let checkbox = mods.0.iter().any(|m| {
            m.name == "toggleStyle"
                && match first_arg(&m.args) {
                    Some(Expr::Implicit(s)) => s == "checkbox",
                    Some(Expr::Call(c)) => ident(&c.callee) == Some("WeftCheckboxStyle"),
                    _ => false,
                }
        });
        mods.take("toggleStyle");
        let kind = if checkbox { "checkbox" } else { "switch" };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "checked",
            arg(&call.args, "isOn"),
            Leaf::Bool,
            &here,
        );
        self.finish(node, mods, &here)
    }

    fn picker(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let style = mods
            .take("pickerStyle")
            .and_then(|m| implicit(first_arg(&m.args)).map(str::to_owned));
        let (kind, child) = match style.as_deref() {
            Some("segmented") => ("segmented-control", "segment"),
            Some("inline" | "radioGroup") => ("radio-group", "radio"),
            _ => ("select", "option"),
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        self.writes(
            &mut node,
            "value",
            arg(&call.args, "selection"),
            Leaf::Text,
            &here,
        );
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Choice(child), &here));
        self.finish(node, mods, &here)
    }

    fn list(&mut self, call: &Call, mods: Mods, path: &str) -> Vec<Child> {
        // `List(items) { item in … }` is a list over an `<each>`.
        if let Some(data) = first_arg(&call.args).cloned() {
            let each = Call {
                callee: Box::new(Expr::Ident {
                    name: "ForEach".to_owned(),
                    raw: false,
                }),
                args: call.args.clone(),
                closures: call.closures.clone(),
            };
            let _ = data;
            let wrapper = Call {
                callee: call.callee.clone(),
                args: vec![],
                closures: vec![Closure {
                    label: None,
                    params: vec![],
                    body: vec![Stmt::Expr(Expr::Call(each))],
                }],
            };
            return self.container("list", &wrapper, mods, Place::List, path);
        }
        self.container("list", call, mods, Place::List, path)
    }

    fn tabs(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("tabs", &mut mods, "", path) else {
            return vec![];
        };
        self.writes(
            &mut node,
            "selected",
            arg(&call.args, "selection"),
            Leaf::Text,
            &here,
        );
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Tabs, &here));
        self.finish(node, mods, &here)
    }

    fn tab(&mut self, name: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some(item) = mods.take("tabItem") else {
            if name == "ForEach" {
                return self.each(call, mods, Place::Tabs, path);
            }
            self.lose(
                LossKind::Structure,
                path,
                format!("`{name}` without `.tabItem` is not a tab and is dropped"),
            );
            return vec![];
        };
        let Some((mut node, here)) = self.element("tab", &mut mods, "", path) else {
            return vec![];
        };
        let label = item.closures.first().and_then(|c| match c.body.as_slice() {
            [Stmt::Expr(e)] => {
                let (base, _) = unwind(e);
                match base {
                    Expr::Call(t) if matches!(ident(&t.callee), Some("Text" | "Label")) => {
                        first_arg(&t.args).cloned()
                    }
                    _ => None,
                }
            }
            _ => None,
        });
        if let Some(e) = label
            && let Some(v) = self.value(&e, Leaf::Text, &here)
        {
            node.props.insert("label".to_owned(), v);
        }
        // `.tag(id)` selects the tab by its id; nothing else to keep.
        mods.take("tag");
        if name == "VStack" {
            if call.args.iter().any(|a| {
                !(a.label.as_deref() == Some("alignment")
                    && implicit(Some(&a.value)) == Some("leading"))
            }) {
                self.lose(
                    LossKind::Layout,
                    &here,
                    "the tab's stack layout is not kept",
                );
            }
            let body = closure(&call.closures, None)
                .map(|c| c.body.clone())
                .unwrap_or_default();
            let children = self.nested(|r| r.views(&body, Place::Nodes, &here));
            node.children = self.elements_only(children, &here);
        } else {
            let children = self.nested(|r| r.element_view(name, call, Mods(vec![]), &here));
            node.children = self.elements_only(children, &here);
        }
        self.finish(node, mods, &here)
    }

    fn menu(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element("menu", &mut mods, "", path) else {
            return vec![];
        };
        self.caption(&mut node, call, &here);
        let items = if first_arg(&call.args).is_some()
            || closure(&call.closures, Some("label")).is_some()
        {
            closure(&call.closures, None)
        } else {
            closure(&call.closures, Some("content"))
        };
        let body = items.map(|c| c.body.clone()).unwrap_or_default();
        node.children = self.nested(|r| r.views(&body, Place::Menu, &here));
        self.finish(node, mods, &here)
    }

    fn mixed(&mut self, kind: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        if !call.args.is_empty() {
            self.lose(
                LossKind::Layout,
                &here,
                "the stack's alignment and spacing are not kept",
            );
        }
        let body = closure(&call.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        // A lone `Text(verbatim:)` or `Text(expr)` without modifiers is the element's own text.
        if let [Stmt::Expr(Expr::Call(c))] = body.as_slice()
            && ident(&c.callee) == Some("Text")
            && (arg(&c.args, "verbatim").is_some()
                || matches!(first_arg(&c.args), Some(e) if !matches!(e, Expr::Str(_))))
        {
            self.text_content(&mut node, c, &here);
        } else {
            node.children = self.nested(|r| r.mixed_children(&body, &here));
        }
        if kind == "alert"
            && let Some(m) = mods.take("backgroundStyle")
        {
            let color = match first_arg(&m.args) {
                Some(Expr::Call(c)) => match c.callee.as_ref() {
                    Expr::Member { base, .. } => type_name(base),
                    _ => None,
                },
                Some(other) => type_name(other),
                None => None,
            };
            let tone = match color.as_deref() {
                Some("Color.blue") => Some("info"),
                Some("Color.green") => Some("success"),
                Some("Color.orange") => Some("warning"),
                Some("Color.red") => Some("danger"),
                _ => None,
            };
            match tone {
                Some(t) => {
                    node.props
                        .insert("tone".to_owned(), Value::String(t.to_owned()));
                }
                None => self.lose(
                    LossKind::Layout,
                    &here,
                    "a background with no Weft tone is not kept",
                ),
            }
        }
        self.finish(node, mods, &here)
    }

    /// The call of a view the app writes for a kind of its own catalog: labelled arguments are
    /// props and `on…` handlers, builders are the content and the slots (README, "Custom
    /// components"). The first trailing builder is the first the view takes.
    fn custom_view(&mut self, kind: &str, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
        let Some(def) = self.catalog.components.get(kind) else {
            return vec![];
        };
        let Some((mut node, here)) = self.element(kind, &mut mods, "", path) else {
            return vec![];
        };
        let mut builders: Vec<String> = vec![];
        if def.content != Content::None {
            builders.push("content".to_owned());
        }
        builders.extend(
            def.slots
                .iter()
                .flat_map(|s| s.keys())
                .map(|n| swift::label(n)),
        );
        let mut bodies: Vec<(String, Vec<Stmt>)> = vec![];
        for a in &call.args {
            let Some(label) = a.label.as_deref() else {
                self.lose(
                    LossKind::Props,
                    &here,
                    format!(
                        "an argument of `{}` without a label is not kept",
                        swift::view_name(kind)
                    ),
                );
                continue;
            };
            let prop = def
                .props
                .iter()
                .flatten()
                .find(|(name, _)| swift::label(name) == label || name.as_str() == label);
            if let Some((name, prop)) = prop {
                let value = match prop.kind {
                    PropType::Token => self.token(&a.value).map(Value::Token),
                    PropType::Number => self.value(&a.value, Leaf::Int, &here),
                    PropType::Boolean => self.value(&a.value, Leaf::Bool, &here),
                    PropType::String | PropType::Enum => self.value(&a.value, Leaf::Text, &here),
                };
                match value {
                    Some(v) => {
                        node.props.insert(name.clone(), v);
                    }
                    None => self.lose(
                        if prop.kind == PropType::Token {
                            LossKind::Tokens
                        } else {
                            LossKind::Bindings
                        },
                        &here,
                        format!("the value of `{name}` is not a literal, a data path or a token"),
                    ),
                }
                continue;
            }
            if label == "state" && def.states.is_some() {
                if let Some(v) = self.value(&a.value, Leaf::Text, &here) {
                    node.props.insert("state".to_owned(), v);
                }
                continue;
            }
            let event = def
                .events
                .iter()
                .flatten()
                .find(|e| swift::event_label(e) == label)
                .cloned();
            match (event, &a.value) {
                (Some(event), Expr::Closure(c)) => {
                    self.event(&mut node, &event, &c.body.clone(), &here);
                }
                (None, Expr::Closure(c)) if builders.iter().any(|b| b == label) => {
                    bodies.push((label.to_owned(), c.body.clone()));
                }
                _ => self.lose(
                    LossKind::Props,
                    &here,
                    format!(
                        "`{}` has no `{label}` argument in Weft",
                        swift::view_name(kind)
                    ),
                ),
            }
        }
        for (i, c) in call.closures.iter().enumerate() {
            let label = match (&c.label, i) {
                (Some(l), _) => Some(l.clone()),
                (None, 0) => builders
                    .iter()
                    .find(|b| !bodies.iter().any(|(l, _)| l == *b))
                    .cloned(),
                (None, _) => None,
            };
            match label.filter(|l| builders.contains(l)) {
                Some(l) => bodies.push((l, c.body.clone())),
                None => self.lose(
                    LossKind::Slots,
                    &here,
                    format!(
                        "a view builder of `{}` has no slot in Weft",
                        swift::view_name(kind)
                    ),
                ),
            }
        }
        let slots: Vec<String> = def.slots.iter().flat_map(|s| s.keys()).cloned().collect();
        for (label, body) in bodies {
            if label == "content" && def.content != Content::None {
                let children = self.nested(|r| r.mixed_children(&body, &here));
                node.children.extend(children);
                continue;
            }
            let Some(name) = slots.iter().find(|n| swift::label(n) == label) else {
                continue;
            };
            let slot_path = format!("{here}/slot[{name}]");
            let views = self.nested(|r| r.views(&body, Place::Nodes, &slot_path));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert(name.clone(), views);
            }
        }
        // The canonical form orders slots and events by name.
        node.slots.sort_keys();
        node.on.sort_keys();
        self.finish(node, mods, &here)
    }

    /// Content of a mixed kind: a bare `Text("…")` is a text run, anything else an element.
    fn mixed_children(&mut self, body: &[Stmt], path: &str) -> Vec<Child> {
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
    fn wrapped(
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

    fn grid_row(&mut self, call: &Call, mut mods: Mods, path: &str) -> Vec<Child> {
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

    fn each(&mut self, call: &Call, mut mods: Mods, place: Place, path: &str) -> Vec<Child> {
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

    fn sheet(&mut self, m: &Mod, hidden: Option<&Mod>, path: &str) -> Option<Node> {
        let content = m
            .closures
            .first()
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let mut inner_mods = Mods(vec![]);
        let mut statements = content.clone();
        // `VStack(alignment: .leading) { … }` with the dialog's modifiers.
        if let [Stmt::Expr(e)] = content.as_slice() {
            let (base, mods) = unwind(e);
            if let Expr::Call(c) = base
                && ident(&c.callee) == Some("VStack")
            {
                statements = closure(&c.closures, None)
                    .map(|c| c.body.clone())
                    .unwrap_or_default();
                inner_mods = Mods(mods);
            }
        }
        let (mut node, here) = self.element("dialog", &mut inner_mods, "", path)?;
        self.writes(
            &mut node,
            "open",
            arg(&m.args, "isPresented"),
            Leaf::Bool,
            &here,
        );
        if let Some(Expr::Closure(c)) = arg(&m.args, "onDismiss") {
            let body = c.body.clone();
            self.event(&mut node, "close", &body, &here);
        }
        let children = self.nested(|r| r.views(&statements, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        if let Some(inset) = inner_mods.take("safeAreaInset") {
            let body = inset
                .closures
                .first()
                .map(|c| c.body.clone())
                .unwrap_or_default();
            let actions = match body.as_slice() {
                [Stmt::Expr(Expr::Call(c))] if ident(&c.callee) == Some("HStack") => {
                    closure(&c.closures, None)
                        .map(|c| c.body.clone())
                        .unwrap_or_default()
                }
                other => other.to_vec(),
            };
            let views =
                self.nested(|r| r.views(&actions, Place::Nodes, &format!("{here}/slot[actions]")));
            let views = self.elements_only(views, &here);
            if !views.is_empty() {
                node.slots.insert("actions".to_owned(), views);
            }
        }
        if let Some(i) = inner_mods.take("presentationBackgroundInteraction") {
            match implicit(first_arg(&i.args)) {
                Some("disabled") => {
                    node.props.insert("modal".to_owned(), Value::Bool(true));
                }
                Some("enabled") => {
                    node.props.insert("modal".to_owned(), Value::Bool(false));
                }
                _ => {}
            }
        }
        if let Some(h) = hidden {
            inner_mods.0.push(h.clone());
        }
        self.common(&mut node, inner_mods, &here);
        Some(node)
    }

    /// `.alert(title, isPresented:) { actions } message: { … }` is a dialog too.
    fn alert(&mut self, m: &Mod, path: &str) -> Option<Node> {
        let title = match first_arg(&m.args) {
            Some(Expr::Str(p)) => plain_string(p).unwrap_or_default(),
            _ => String::new(),
        };
        let (mut node, here) = self.element("dialog", &mut Mods(vec![]), &title, path)?;
        if let Some(t) = first_arg(&m.args).cloned()
            && let Some(v) = self.value(&t, Leaf::Text, &here)
        {
            node.props.insert("label".to_owned(), v);
        }
        self.writes(
            &mut node,
            "open",
            arg(&m.args, "isPresented"),
            Leaf::Bool,
            &here,
        );
        let actions = closure(&m.closures, None)
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let message = closure(&m.closures, Some("message"))
            .map(|c| c.body.clone())
            .unwrap_or_default();
        let children = self.nested(|r| r.views(&message, Place::Nodes, &here));
        node.children = self.elements_only(children, &here);
        let views =
            self.nested(|r| r.views(&actions, Place::Nodes, &format!("{here}/slot[actions]")));
        let views = self.elements_only(views, &here);
        if !views.is_empty() {
            node.slots.insert("actions".to_owned(), views);
        }
        Some(node)
    }
}

fn name_is(call: &Call, name: &str) -> bool {
    ident(&call.callee) == Some(name)
}

/// A literal read where a prop wants another type, converted as Weft reads values (SPEC §2.1).
fn convert(v: Value, leaf: Leaf) -> Value {
    match (leaf, v) {
        (Leaf::Text, Value::Number(n)) => Value::String(swift::number_literal(n)),
        (Leaf::Text, Value::Bool(b)) => Value::String(b.to_string()),
        (_, v) => v,
    }
}

/// The Weft name of a loop variable: `generate` adds `_` to names Swift reserves.
fn loop_name(ident: &str) -> String {
    let stripped = ident.strip_suffix('_').unwrap_or(ident);
    let base =
        if stripped != ident && (swift::is_keyword(stripped) || VIEW_MEMBERS.contains(&stripped)) {
            stripped
        } else {
            ident
        };
    let mut out = String::new();
    for c in base.chars().filter(char::is_ascii_alphanumeric) {
        if out.is_empty() {
            if c.is_ascii_alphabetic() {
                out.push(c.to_ascii_lowercase());
            }
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        "item".to_owned()
    } else {
        out
    }
}

/// A Weft action name from a Swift function name.
fn action_name(name: &str) -> String {
    let n = loop_name(name);
    if n == "item" && !name.starts_with("item") {
        "action".to_owned()
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_come_back() {
        assert_eq!(loop_name("self_"), "self");
        assert_eq!(loop_name("model_"), "model");
        assert_eq!(loop_name("todo"), "todo");
        assert_eq!(loop_name("$0"), "item");
        assert!(is_action("auth.submit"));
        assert!(!is_action("Auth"));
    }
}
