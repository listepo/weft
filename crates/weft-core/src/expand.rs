//! Expansion of fragment uses (SPEC §10.7): what a `<use>` means, for renderers and generators.
//! The canonical document keeps its uses; this builds a separate document in which each use is
//! its fragment's body, with parameter reads replaced by the use's values, outlets by its slot
//! content and ids by instance paths. Fragments are as untrusted as screens, so a cycle (`W805`)
//! and an expansion past the element or depth limit (`W806`) stop it.

use std::collections::HashSet;

use crate::diagnostics::{Code, Diagnostic, Position};
use crate::fragment::{OUTLET, PARAM, ParamKind, USE};
use crate::model::{Catalog, Child, Document, Map, Node, Value};
use crate::rules::{EACH, MAX_DEPTH};
use crate::source::path_segment;

/// The most elements an expanded screen may hold.
pub const EXPANDED_LIMIT: usize = 10_000;

/// The expanded document and the expansion's own problems (`W805`, `W806`).
pub struct Expanded {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

/// `document` with every use of a known fragment expanded. A use of an unknown fragment is kept
/// as written, and a use that closes a cycle expands to nothing.
pub fn expand(document: &Document, catalog: &Catalog) -> Expanded {
    let mut x = Expander {
        catalog,
        count: 0,
        stack: vec![],
        site: None,
        diagnostics: vec![],
        stopped: false,
    };
    let path = format!(
        "/{}",
        path_segment(&document.root.kind, document.root.id.as_deref(), None)
    );
    let top = Context::default();
    let mut root = x.node(&document.root, &top, Some(&path), 1);
    if x.stopped {
        // Half an expansion would draw a screen nobody wrote; keep only the root.
        root.children.clear();
        root.slots.clear();
    }
    Expanded {
        document: Document {
            weft: document.weft.clone(),
            root,
        },
        diagnostics: x.diagnostics,
    }
}

/// Where a body is being expanded: the instance-path prefix, the use's arguments, the loop
/// variables in scope and the body's renamed ones.
#[derive(Default)]
struct Context<'a> {
    prefix: String,
    args: Option<&'a Args<'a>>,
    scope: Vec<String>,
    renames: Map<String>,
    local_ids: HashSet<String>,
}

/// A use's arguments. Values and actions are read in the use's context; slot content is expanded
/// where its outlet stands, so that it counts at its real depth, yet in the use's context.
struct Args<'a> {
    params: Map<ParamKind>,
    values: Map<Value>,
    on: Map<String>,
    slots: &'a Map<Vec<Child>>,
    outer: &'a Context<'a>,
    /// The use's path in the input document and how many fragments were open around it.
    path: Option<&'a str>,
    open: usize,
}

struct Expander<'a> {
    catalog: &'a Catalog,
    count: usize,
    /// The fragments being expanded, outermost first.
    stack: Vec<String>,
    /// The outermost use in the document, where expansion problems are reported.
    site: Option<(String, Option<Position>)>,
    diagnostics: Vec<Diagnostic>,
    stopped: bool,
}

impl Expander<'_> {
    fn report(&mut self, code: Code, message: String, expected: &str) {
        let (path, pos) = self.site.clone().unwrap_or_default();
        self.diagnostics
            .push(Diagnostic::new(code, path, message, expected).pos(pos));
    }

    /// `path` is the node's path in the input document; absent inside a fragment body.
    fn list(
        &mut self,
        list: &[Child],
        cx: &Context,
        path: Option<&str>,
        depth: usize,
    ) -> Vec<Child> {
        let mut out = Vec::new();
        for (index, child) in list.iter().enumerate() {
            let node = match child {
                Child::Text(text) => {
                    out.push(Child::Text(text.clone()));
                    continue;
                }
                Child::Node(node) => node,
            };
            if self.stopped {
                break;
            }
            let path = path.map(|p| {
                let segment = path_segment(&node.kind, node.id.as_deref(), Some(index));
                format!("{p}/{segment}")
            });
            if let (OUTLET, Some(args)) = (node.kind.as_str(), cx.args) {
                let name = string(node.props.get("name"));
                if let Some(content) = args.slots.get(&name) {
                    let path = args.path.map(|p| format!("{p}/slot[{name}]"));
                    // Slot content belongs to the use site, so the fragments open inside the use
                    // are not around it: a card in a card's slot is no cycle.
                    let inner = self.stack.split_off(args.open);
                    out.extend(self.list(content, args.outer, path.as_deref(), depth));
                    self.stack.extend(inner);
                }
            } else if node.kind == USE && self.catalog.fragment_of(node).is_some() {
                out.extend(self.use_site(node, cx, path.as_deref(), depth));
            } else {
                let expanded = self.node(node, cx, path.as_deref(), depth + 1);
                out.push(Child::Node(Box::new(expanded)));
            }
        }
        out
    }

    fn node(&mut self, node: &Node, cx: &Context, path: Option<&str>, depth: usize) -> Node {
        self.count += 1;
        // Outside uses the document's own size and depth are its parser's concern.
        if self.site.is_some() && (self.count > EXPANDED_LIMIT || depth > MAX_DEPTH) {
            self.stop();
            return Node::new(node.kind.clone());
        }
        let mut out = Node::new(node.kind.clone());
        out.id = node.id.as_ref().map(|id| format!("{}{id}", cx.prefix));
        out.source = node.source.clone();
        let def = self.catalog.components.get(&node.kind);
        for (name, value) in &node.props {
            let references = def
                .and_then(|d| d.prop(name))
                .and_then(|d| d.references.as_ref());
            let value = match value {
                Value::String(id) if references.is_some() && cx.local_ids.contains(id) => {
                    Some(Value::String(format!("{}{id}", cx.prefix)))
                }
                _ => substitute(value, cx),
            };
            if let Some(value) = value {
                out.props.insert(name.clone(), value);
            }
        }
        for (event, action) in &node.on {
            let read = action.strip_prefix("{$").and_then(|a| a.strip_suffix('}'));
            match (read, cx.args) {
                (Some(name), Some(args)) if args.params.contains_key(name) => {
                    if let Some(action) = args.on.get(name) {
                        out.on.insert(event.clone(), action.clone());
                    }
                }
                _ => {
                    out.on.insert(event.clone(), action.clone());
                }
            }
        }
        // A loop variable of the body that a use-site name would collide with is renamed.
        let mut inner = None;
        if node.kind == EACH && cx.args.is_some() {
            if let Some(Value::String(name)) = node.props.get("as") {
                let mut fresh = name.clone();
                let mut n = 2;
                while cx.scope.contains(&fresh) || cx.renames.values().any(|v| *v == fresh) {
                    fresh = format!("{name}{n}");
                    n += 1;
                }
                out.props.insert("as".into(), Value::String(fresh.clone()));
                let mut renames = cx.renames.clone();
                renames.insert(name.clone(), fresh.clone());
                let mut scope = cx.scope.clone();
                scope.push(fresh);
                inner = Some(Context {
                    prefix: cx.prefix.clone(),
                    args: cx.args,
                    scope,
                    renames,
                    local_ids: cx.local_ids.clone(),
                });
            }
        } else if node.kind == EACH
            && let Some(Value::String(name)) = node.props.get("as")
        {
            let mut scope = cx.scope.clone();
            scope.push(name.clone());
            inner = Some(Context {
                scope,
                ..Context::default()
            });
        }
        let cx = inner.as_ref().unwrap_or(cx);
        for (name, list) in &node.slots {
            let path = path.map(|p| format!("{p}/slot[{name}]"));
            let list = self.list(list, cx, path.as_deref(), depth);
            out.slots.insert(name.clone(), list);
        }
        out.children = self.list(&node.children, cx, path, depth);
        out
    }

    /// The body of the fragment a `<use>` names, expanded with the use's arguments.
    fn use_site(
        &mut self,
        node: &Node,
        cx: &Context,
        path: Option<&str>,
        depth: usize,
    ) -> Vec<Child> {
        let Some(fragment) = self.catalog.fragment_of(node) else {
            return vec![];
        };
        let name = string(node.props.get("fragment"));
        let previous = self.site.clone();
        if let Some(path) = path {
            // A use written in the document, perhaps in another use's slot.
            let pos = node.source.0.as_deref().map(|s| s.pos);
            self.site = Some((path.to_owned(), pos));
        }
        if self.stack.contains(&name) {
            let mut chain = self.stack.clone();
            chain.push(name);
            let message = format!(
                "Fragments use each other in a cycle: {}.",
                chain.join(" → ")
            );
            self.report(Code::W805, message, "fragments that do not use themselves");
            self.site = previous;
            return vec![];
        }
        let root = &fragment.document.root;
        let mut values = Map::new();
        for param in root.children.iter().filter_map(Child::as_node) {
            if param.kind != PARAM {
                break;
            }
            if let (Some(Value::String(p)), Some(default)) =
                (param.props.get("name"), param.props.get("default"))
            {
                values.insert(p.clone(), default.clone());
            }
        }
        for (prop, value) in &node.props {
            if prop != "fragment"
                && let Some(value) = substitute(value, cx)
            {
                values.insert(prop.clone(), value);
            }
        }
        let mut on = Map::new();
        for (event, action) in &node.on {
            let read = action.strip_prefix("{$").and_then(|a| a.strip_suffix('}'));
            let forwarded = match (read, cx.args) {
                (Some(r), Some(args)) => args.on.get(r).cloned(),
                _ => Some(action.clone()),
            };
            if let Some(action) = forwarded {
                on.insert(event.clone(), action);
            }
        }
        let args = Args {
            params: crate::fragment::params(root),
            values,
            on,
            slots: &node.slots,
            outer: cx,
            path,
            open: self.stack.len(),
        };
        let id = node.id.clone().unwrap_or_default();
        let body = Context {
            prefix: format!("{}{id}/", cx.prefix),
            args: Some(&args),
            scope: cx.scope.clone(),
            renames: Map::new(),
            local_ids: ids(&root.children),
        };
        self.stack.push(name);
        // A use counts as a level, so a chain of uses is bounded like nesting.
        let out = self.list(&root.children[params_len(root)..], &body, None, depth + 1);
        self.stack.pop();
        self.site = previous;
        out
    }

    fn stop(&mut self) {
        if !self.stopped {
            self.stopped = true;
            let message = format!(
                "Expanding the fragments gives more than {EXPANDED_LIMIT} elements or more than {MAX_DEPTH} levels."
            );
            self.report(Code::W806, message, "fewer or smaller uses");
        }
    }
}

/// A prop value in a body: a parameter read becomes the use's value (absent when the use gives
/// none), and a read of a renamed loop variable follows the rename.
fn substitute(value: &Value, cx: &Context) -> Option<Value> {
    let Value::Bind { bind, not } = value else {
        return Some(value.clone());
    };
    let Some(rest) = bind.strip_prefix('$').filter(|r| !r.starts_with('.')) else {
        return Some(value.clone());
    };
    let (name, tail) = rest.split_at(rest.find('.').unwrap_or(rest.len()));
    if let Some(args) = cx.args.filter(|a| a.params.contains_key(name)) {
        return match (args.values.get(name)?, not) {
            (Value::Bind { bind, not: inner }, _) => Some(Value::Bind {
                bind: bind.clone(),
                not: inner ^ not,
            }),
            (Value::Bool(b), true) => Some(Value::Bool(!b)),
            (v, _) => Some(v.clone()),
        };
    }
    match cx.renames.get(name) {
        Some(fresh) => Some(Value::Bind {
            bind: format!("${fresh}{tail}"),
            not: *not,
        }),
        None => Some(value.clone()),
    }
}

fn string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn params_len(root: &Node) -> usize {
    let is_param = |c: &Child| c.as_node().is_some_and(|n| n.kind == PARAM);
    root.children.iter().take_while(|c| is_param(c)).count()
}

/// The ids written in a fragment's body, which references to them follow into instance paths.
fn ids(list: &[Child]) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack: Vec<&Node> = list.iter().filter_map(Child::as_node).collect();
    while let Some(node) = stack.pop() {
        out.extend(node.id.clone());
        let lists = node.slots.values().chain(std::iter::once(&node.children));
        stack.extend(lists.flatten().filter_map(Child::as_node));
    }
    out
}
