//! The typed data model behind a screen. Weft documents name data paths but never their types,
//! so each path's Swift type is inferred from how the document uses it: what is written by a
//! text field is a `String`, what a toggle writes is a `Bool`, what `<each>` iterates is an array,
//! what has members is a struct.

use indexmap::IndexMap;
use weft_core::{Catalog, Child, Node, PropType, Value};

use crate::swift;
use crate::{Unsupported, kinds};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaf {
    Text,
    Bool,
    Int,
    /// What a slider or stepper reads and writes: a fraction is a legal value.
    Double,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    String,
    Bool,
    Int,
    Double,
    Struct(usize),
    Array(Box<Ty>),
}

#[derive(Debug, Default)]
pub struct Shape {
    pub fields: IndexMap<String, usize>,
    pub item: Option<usize>,
    reads: Vec<Leaf>,
    writes: Vec<Leaf>,
}

/// Every data path of a document as a tree; node 0 is the root `$`.
#[derive(Debug)]
pub struct Shapes {
    pub nodes: Vec<Shape>,
    pub types: Vec<Ty>,
}

/// A loop variable in scope: its name and the shape of one item.
type Scope = Vec<(String, usize)>;

/// The use a prop makes of a bound value, from the catalog's declaration.
pub fn prop_leaf(catalog: &Catalog, kind: &str, prop: &str) -> Option<(Leaf, bool)> {
    match prop {
        "label" | "state" => return Some((Leaf::Text, false)),
        "hidden" => return Some((Leaf::Bool, false)),
        _ => {}
    }
    let def = catalog.components.get(kind)?.prop(prop)?;
    let writable = def.writable == Some(true);
    let leaf = match def.kind {
        PropType::String | PropType::Enum => Leaf::Text,
        PropType::Boolean => Leaf::Bool,
        PropType::Number if matches!(kind, "slider" | "stepper") => Leaf::Double,
        PropType::Number => Leaf::Int,
        PropType::Token => return None,
    };
    Some((leaf, writable))
}

impl Shapes {
    pub fn infer(root: &Node, catalog: &Catalog, problems: &mut Vec<Unsupported>) -> Shapes {
        let mut shapes = Shapes {
            nodes: vec![Shape::default()],
            types: vec![],
        };
        shapes.walk(root, catalog, &mut vec![], problems);
        shapes.resolve(problems);
        shapes
    }

    fn walk(
        &mut self,
        node: &Node,
        catalog: &Catalog,
        scope: &mut Scope,
        problems: &mut Vec<Unsupported>,
    ) {
        if node.kind == kinds::EACH {
            let source = node.props.get("in");
            let item = match source {
                Some(Value::Bind { bind, .. }) => self
                    .locate(bind, scope, problems)
                    .map(|at| self.item_of(at)),
                _ => None,
            };
            let name = match node.props.get("as") {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            };
            if let Some(item) = item {
                scope.push((name, item));
                self.walk_children(node, catalog, scope, problems);
                scope.pop();
            }
            return;
        }
        for (prop, value) in &node.props {
            let Value::Bind { bind, not } = value else {
                continue;
            };
            let Some(at) = self.locate(bind, scope, problems) else {
                continue;
            };
            match prop_leaf(catalog, &node.kind, prop) {
                None => problems.push(Unsupported::new(
                    bind,
                    format!(
                        "`{prop}` of `{}` is bound; only literal token references are generated",
                        node.kind
                    ),
                )),
                Some(_) if *not => self.nodes[at].reads.push(Leaf::Bool),
                Some((leaf, true)) => self.nodes[at].writes.push(leaf),
                Some((leaf, false)) => self.nodes[at].reads.push(leaf),
            }
        }
        self.walk_children(node, catalog, scope, problems);
    }

    fn walk_children(
        &mut self,
        node: &Node,
        catalog: &Catalog,
        scope: &mut Scope,
        problems: &mut Vec<Unsupported>,
    ) {
        let lists = std::iter::once(&node.children).chain(node.slots.values());
        for child in lists.flatten() {
            if let Child::Node(n) = child {
                self.walk(n, catalog, scope, problems);
            }
        }
    }

    fn item_of(&mut self, at: usize) -> usize {
        if let Some(item) = self.nodes[at].item {
            return item;
        }
        self.nodes.push(Shape::default());
        let item = self.nodes.len() - 1;
        self.nodes[at].item = Some(item);
        item
    }

    /// The shape a binding path ends at, created on first use.
    fn locate(
        &mut self,
        bind: &str,
        scope: &Scope,
        problems: &mut Vec<Unsupported>,
    ) -> Option<usize> {
        let rest = bind.strip_prefix('$')?;
        let mut segments = rest.split('.');
        let head = segments.next()?;
        let mut at = if head.is_empty() {
            0
        } else {
            scope.iter().rev().find(|(name, _)| name == head)?.1
        };
        for (i, segment) in segments.enumerate() {
            if segment.chars().all(|c| c.is_ascii_digit()) {
                problems.push(Unsupported::new(
                    bind,
                    "binding paths with an array index are not generated; iterate with <each>",
                ));
                return None;
            }
            if at == 0 && i == 0 && !swift::is_plain_identifier(segment) {
                problems.push(Unsupported::new(
                    bind,
                    format!(
                        "`{segment}` is a Swift keyword and cannot name an @Observable property"
                    ),
                ));
                return None;
            }
            at = match self.nodes[at].fields.get(segment) {
                Some(&next) => next,
                None => {
                    self.nodes.push(Shape::default());
                    let next = self.nodes.len() - 1;
                    self.nodes[at].fields.insert(segment.to_owned(), next);
                    next
                }
            };
        }
        Some(at)
    }

    fn resolve(&mut self, problems: &mut Vec<Unsupported>) {
        self.types = vec![Ty::String; self.nodes.len()];
        // Children are always created after their parent, so a reverse sweep sees them first.
        for at in (0..self.nodes.len()).rev() {
            let shape = &self.nodes[at];
            let ty = if !shape.fields.is_empty() && shape.item.is_some() {
                problems.push(Unsupported::new(
                    "$",
                    "a data path is used both as a list and as an object",
                ));
                Ty::Struct(at)
            } else if !shape.fields.is_empty() || at == 0 {
                Ty::Struct(at)
            } else if let Some(item) = shape.item {
                Ty::Array(Box::new(self.types[item].clone()))
            } else {
                leaf_type(&shape.reads, &shape.writes)
            };
            let collection = matches!(ty, Ty::Struct(_) | Ty::Array(_));
            if collection && !shape.writes.is_empty() {
                problems.push(Unsupported::new(
                    "$",
                    "a data path is written by a control and also holds a list or an object",
                ));
            }
            let written = |leaf| shape.writes.contains(&leaf);
            if written(Leaf::Text) && written(Leaf::Bool) {
                problems.push(Unsupported::new(
                    "$",
                    "a data path is written both as text and as a flag",
                ));
            } else if written(Leaf::Double) && (written(Leaf::Text) || written(Leaf::Bool)) {
                // One Swift type per path: a slider's `Binding<Double>` cannot also be text.
                problems.push(Unsupported::new(
                    "$",
                    "a data path is written both as a number and as text or a flag",
                ));
            }
            self.types[at] = ty;
        }
    }

    /// The type of a path, for the expressions that read it.
    pub fn type_at(&self, at: usize) -> &Ty {
        &self.types[at]
    }

    /// The shape at `bind` without creating anything: used while printing, after inference.
    pub fn find(&self, bind: &str, scope: &[(String, usize)]) -> Option<usize> {
        let rest = bind.strip_prefix('$')?;
        let mut segments = rest.split('.');
        let head = segments.next()?;
        let mut at = if head.is_empty() {
            0
        } else {
            scope.iter().rev().find(|(name, _)| name == head)?.1
        };
        for segment in segments {
            at = *self.nodes[at].fields.get(segment)?;
        }
        Some(at)
    }
}

/// A control's write decides the type; otherwise text wins over numbers (fractions over whole
/// numbers) and numbers over flags,
/// because text can always be tested and converted where the others are read.
fn leaf_type(reads: &[Leaf], writes: &[Leaf]) -> Ty {
    if writes.contains(&Leaf::Text) {
        Ty::String
    } else if writes.contains(&Leaf::Bool) {
        Ty::Bool
    } else if writes.contains(&Leaf::Double) {
        Ty::Double
    } else if reads.contains(&Leaf::Text) {
        Ty::String
    } else if reads.contains(&Leaf::Double) {
        Ty::Double
    } else if reads.contains(&Leaf::Int) {
        Ty::Int
    } else {
        Ty::Bool
    }
}
