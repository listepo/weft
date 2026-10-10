//! A document filled in with sample data, so the static page can show it: every binding becomes
//! the literal it reads, typed by the catalog, and every `<each>` one copy of its content per item
//! with instance ids `id[index]` (SPEC §2.1, §4). This is what the reference renderer computes at
//! run time (`@weft/render-react`, `expand.ts`); paths, truthiness and text come from `weft_core`
//! so the two cannot read data differently.

use serde_json::Value as Json;
use weft_core::{
    Catalog, Child, Document, MAX_DEPTH, Node, PropType, Value, is_loop_variable, resolve_path,
    text, truthy, universal_prop,
};

use crate::js::V;
use crate::jsx::clamp_number;

pub(crate) fn fill(document: &Document, catalog: &Catalog, data: &Json) -> Document {
    let filler = Filler { catalog, data };
    let scope = Scope {
        vars: vec![],
        suffix: String::new(),
    };
    Document {
        weft: document.weft.clone(),
        version: document.version.clone(),
        // The filled document is only ever shown, and end users never see context (SPEC §2.3).
        context: Vec::new(),
        root: filler.node(&document.root, &scope, 0),
    }
}

struct Filler<'a> {
    catalog: &'a Catalog,
    data: &'a Json,
}

struct Scope<'a> {
    vars: Vec<(&'a str, &'a Json)>,
    /// Appended to every id inside `<each>`, e.g. `[2][0]`.
    suffix: String,
}

impl<'a> Filler<'a> {
    fn node(&self, n: &'a Node, scope: &Scope<'a>, depth: usize) -> Node {
        let mut out = Node {
            kind: n.kind.clone(),
            id: n.id.as_ref().map(|id| format!("{id}{}", scope.suffix)),
            props: Default::default(),
            on: n.on.clone(),
            slots: Default::default(),
            children: self.list(&n.children, scope, depth + 1),
            source: Default::default(),
        };
        for (name, value) in &n.props {
            let filled = match value {
                Value::Bind { bind, not } => {
                    let found = resolve_path(self.data, &scope.vars, bind);
                    let negated;
                    let found = if *not {
                        negated = Json::Bool(!truthy(found));
                        Some(&negated)
                    } else {
                        found
                    };
                    self.literal(&n.kind, name, found)
                }
                other => Some(other.clone()),
            };
            if let Some(v) = filled {
                out.props.insert(name.clone(), v);
            }
        }
        for (name, list) in &n.slots {
            out.slots
                .insert(name.clone(), self.list(list, scope, depth + 1));
        }
        out
    }

    fn list(&self, list: &'a [Child], scope: &Scope<'a>, depth: usize) -> Vec<Child> {
        let mut out = Vec::new();
        // Validation bounds the document's depth; this only keeps a hostile one off the stack.
        if depth > MAX_DEPTH {
            return out;
        }
        for c in list {
            match c {
                Child::Text(t) => out.push(Child::Text(t.clone())),
                Child::Node(n) if n.kind == "each" => self.each(n, scope, depth, &mut out),
                Child::Node(n) => out.push(Child::Node(Box::new(self.node(n, scope, depth)))),
            }
        }
        out
    }

    /// The content once per item. Like the renderer, only a plain binding to an array repeats.
    fn each(&self, n: &'a Node, scope: &Scope<'a>, depth: usize, out: &mut Vec<Child>) {
        let (Some(Value::Bind { bind, not: false }), Some(Value::String(name))) =
            (n.props.get("in"), n.props.get("as"))
        else {
            return;
        };
        let Some(Json::Array(items)) = resolve_path(self.data, &scope.vars, bind) else {
            return;
        };
        if !is_loop_variable(name) {
            return;
        }
        for (i, item) in items.iter().enumerate() {
            let mut vars = scope.vars.clone();
            vars.push((name.as_str(), item));
            let inner = Scope {
                vars,
                suffix: format!("{}[{i}]", scope.suffix),
            };
            out.extend(self.list(&n.children, &inner, depth + 1));
        }
    }

    /// The literal a bound prop shows, as the catalog types it; `None` reads as absent.
    fn literal(&self, kind: &str, name: &str, found: Option<&Json>) -> Option<Value> {
        let def = self
            .catalog
            .components
            .get(kind)
            .and_then(|c| c.prop(name))
            .or_else(|| universal_prop(name));
        let scalar = matches!(
            found,
            Some(Json::String(_) | Json::Number(_) | Json::Bool(_))
        );
        match def.map(|d| d.kind) {
            Some(PropType::Boolean) => Some(Value::Bool(truthy(found))),
            Some(PropType::Number) => match found {
                Some(Json::Number(_)) => {
                    let def = def?;
                    let integer = def.integer == Some(true);
                    match clamp_number(&V::of(found), integer, def.min, def.max) {
                        V::Num(n) => Some(Value::Number(n)),
                        _ => None,
                    }
                }
                _ => None,
            },
            Some(PropType::Token) => None,
            _ => scalar.then(|| Value::String(text(found))),
        }
    }
}
