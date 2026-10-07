//! The component's data: one `in-out property` per root data path that a binding reads, typed
//! from the props that read it, and the Slint expression that reads it as each prop needs it. Kept
//! apart from the generator so the typing rules can be tested on their own.

use indexmap::IndexMap;
use weft_core::{Catalog, Child, Node, PropType, Value, universal_prop};

use crate::Unsupported;
use crate::names::{clash_key, identifier};

/// Names a data property cannot take: the component's `perform` callback and the properties
/// every Slint `Window` already declares, which a component may not declare again.
const TAKEN: &[&str] = &[
    "perform",
    "title",
    "icon",
    "background",
    "always-on-top",
    "full-screen",
    "no-frame",
    "resize-border-width",
    "default-font-family",
    "default-font-size",
    "default-font-weight",
    "width",
    "height",
    "x",
    "y",
    "z",
    "min-width",
    "min-height",
    "max-width",
    "max-height",
    "preferred-width",
    "preferred-height",
    "horizontal-stretch",
    "vertical-stretch",
    "opacity",
    "visible",
    "cache-rendering-hint",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Str,
    Bool,
    Float,
    Int,
}

impl Ty {
    pub fn slint(self) -> &'static str {
        match self {
            Ty::Str => "string",
            Ty::Bool => "bool",
            Ty::Float => "float",
            Ty::Int => "int",
        }
    }
}

/// The Slint type a prop holds, and whether user input writes it back.
pub fn prop_type(catalog: &Catalog, kind: &str, name: &str) -> (Ty, bool) {
    let def = catalog
        .components
        .get(kind)
        .and_then(|c| c.prop(name))
        .or_else(|| universal_prop(name));
    let Some(def) = def else {
        return (Ty::Str, false);
    };
    let ty = match def.kind {
        PropType::Boolean => Ty::Bool,
        // `SpinBox` holds an `int`, `Slider` a `float`.
        PropType::Number if kind == "stepper" => Ty::Int,
        PropType::Number => Ty::Float,
        PropType::String | PropType::Enum | PropType::Token => Ty::Str,
    };
    (ty, def.writable == Some(true))
}

pub struct Property {
    pub name: String,
    pub ty: Ty,
}

/// One repeated array. `fields` is `None` when the array is `[string]`: a `ComboBox` model, which
/// cannot read a field out of a struct.
pub(crate) struct Model {
    pub name: String,
    pub fields: Option<IndexMap<String, Ty>>,
}

impl Model {
    pub(crate) fn slint_type(&self) -> String {
        match &self.fields {
            None => "[string]".to_owned(),
            Some(fields) if fields.is_empty() => "[int]".to_owned(),
            Some(fields) => {
                let body = fields
                    .iter()
                    .map(|(name, ty)| format!("{name}: {}", ty.slint()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{{ {body} }}]")
            }
        }
    }
}

struct Local {
    expr: String,
    ty: Ty,
}

#[derive(Default)]
pub struct Data {
    /// Weft binding path (`$.user.email`) → its property, in document order.
    pub props: IndexMap<String, Property>,
    /// `$.todos` → the model `<each in>` repeats.
    pub(crate) models: IndexMap<String, Model>,
    /// `$todo.done` or `$.players.0.name` → the expression that reads it.
    locals: IndexMap<String, Local>,
}

#[derive(Default)]
struct Readers {
    writable: Vec<Ty>,
    read: Vec<Ty>,
}

struct ModelBuild {
    /// A `select` or `combobox` repeats option text, and `ComboBox.model` is `[string]`.
    strings: bool,
    fields: IndexMap<String, Readers>,
}

struct Use {
    path: String,
    ty: Ty,
    writable: bool,
    /// `(alias, model path)` of enclosing `<each>`, innermost last.
    scope: Vec<(String, String)>,
}

struct Gather<'a> {
    catalog: &'a Catalog,
    uses: Vec<Use>,
    models: IndexMap<String, ModelBuild>,
    problems: &'a mut Vec<Unsupported>,
}

impl Gather<'_> {
    fn walk(&mut self, node: &Node, parent: &str, scope: &[(String, String)]) {
        if node.kind == "each" {
            self.each(node, parent, scope);
            return;
        }
        for (name, value) in &node.props {
            if let Value::Bind { bind, .. } = value {
                let (ty, writable) = prop_type(self.catalog, &node.kind, name);
                self.uses.push(Use {
                    path: bind.clone(),
                    ty,
                    writable,
                    scope: scope.to_vec(),
                });
            }
        }
        let slots = node.slots.values().flatten();
        for child in node.children.iter().chain(slots) {
            if let Child::Node(child) = child {
                self.walk(child, &node.kind, scope);
            }
        }
    }

    fn each(&mut self, node: &Node, parent: &str, scope: &[(String, String)]) {
        let alias = match node.props.get("as") {
            Some(Value::String(name)) if identifier(name).is_some() => name.clone(),
            _ => {
                self.problems.push(Unsupported::new(
                    node_path(node),
                    "the loop variable is reserved in Slint",
                ));
                return;
            }
        };
        let Some(Value::Bind { bind, not: false }) = node.props.get("in") else {
            self.problems.push(Unsupported::new(
                node_path(node),
                "`in` is not a root data path",
            ));
            return;
        };
        if !bind.starts_with("$.") {
            self.problems.push(Unsupported::new(
                bind,
                "the repeated array is not a root data path",
            ));
            return;
        }
        let strings = parent == "select" || parent == "combobox";
        match self.models.entry(bind.clone()) {
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(ModelBuild {
                    strings,
                    fields: IndexMap::new(),
                });
            }
            indexmap::map::Entry::Occupied(entry) => {
                if entry.get().strings != strings {
                    self.problems.push(Unsupported::new(
                        bind,
                        "this array is repeated both as text and as a struct",
                    ));
                }
            }
        }
        let mut inner = scope.to_vec();
        inner.push((alias, bind.clone()));
        let slots = node.slots.values().flatten();
        for child in node.children.iter().chain(slots) {
            if let Child::Node(child) = child {
                self.walk(child, "each", &inner);
            }
        }
    }
}

fn node_path(node: &Node) -> String {
    match &node.id {
        Some(id) => format!("{}#{id}", node.kind),
        None => node.kind.clone(),
    }
}

fn is_index(segment: &str) -> bool {
    segment == "0"
        || (segment.starts_with(|c: char| ('1'..='9').contains(&c))
            && segment.chars().all(|c| c.is_ascii_digit()))
}

/// `$.players.0.name` → (`$.players`, `0`, `name`) when that is one index and one field.
fn model_index(path: &str) -> Option<(String, String, String)> {
    let rest = path.strip_prefix("$.")?;
    let parts: Vec<&str> = rest.split('.').collect();
    let at = parts.iter().position(|part| is_index(part))?;
    if at == 0 || parts.len() != at + 2 {
        return None;
    }
    let field = parts[at + 1];
    if identifier(field).is_none() || parts[..at].iter().any(|part| identifier(part).is_none()) {
        return None;
    }
    Some((
        format!("$.{}", parts[..at].join(".")),
        parts[at].to_owned(),
        field.to_owned(),
    ))
}

fn ty_of(readers: &Readers, path: &str, problems: &mut Vec<Unsupported>) -> Option<Ty> {
    let mut writable = readers.writable.clone();
    writable.dedup();
    if writable.len() > 1 {
        problems.push(Unsupported::new(
            path,
            "inputs of different types write this path",
        ));
        return None;
    }
    Some(writable.first().copied().unwrap_or_else(|| {
        [Ty::Str, Ty::Float, Ty::Int]
            .into_iter()
            .find(|ty| readers.read.contains(ty))
            .unwrap_or(Ty::Bool)
    }))
}

fn property_name(
    path: &str,
    keys: &mut IndexMap<String, String>,
    problems: &mut Vec<Unsupported>,
) -> Option<String> {
    let segments = path.strip_prefix("$.")?;
    let mut name = segments.replace('.', "-");
    let mut key = clash_key(&name);
    // `Window` already declares `title` and the geometry props. A suffix keeps the
    // screen's own data off that property; a name that is still taken is refused.
    if TAKEN.contains(&key.as_str()) || key.starts_with("accessible-") {
        name = format!("{name}-data");
        key = clash_key(&name);
        if TAKEN.contains(&key.as_str()) || key.starts_with("accessible-") {
            problems.push(Unsupported::new(
                path,
                format!("`{name}` is already a property of a Slint Window"),
            ));
            return None;
        }
    }
    if let Some(other) = keys.insert(key, path.to_owned()) {
        problems.push(Unsupported::new(
            path,
            format!("its Slint property `{name}` clashes with the one of {other}"),
        ));
        return None;
    }
    Some(name)
}

/// The Weft path a generated property name stands for. A `-data` suffix is only the
/// `Window` property it was kept off (`title-data` is `$.title`), so it is not another segment.
pub(crate) fn binding_path(name: &str) -> String {
    let key = clash_key(name);
    let stem = if let Some(bare) = key.strip_suffix("-data")
        && (TAKEN.contains(&bare) || bare.starts_with("accessible-"))
        && let Some(kept) = name.strip_suffix("-data")
    {
        kept
    } else {
        name
    };
    format!("$.{}", stem.replace('-', "."))
}

fn note(readers: &mut Readers, ty: Ty, writable: bool) {
    if writable {
        readers.writable.push(ty);
    }
    readers.read.push(ty);
}

enum Place {
    Root,
    Field {
        model: String,
        field: String,
    },
    Index {
        model: String,
        index: String,
        field: String,
    },
    /// Option text of a `[string]` model: the field is the string itself.
    Text,
    Bad,
}

struct Pending {
    path: String,
    model: String,
    field: String,
    alias: Option<String>,
    index: Option<String>,
}

fn classify(
    path: &str,
    scope: &[(String, String)],
    models: &IndexMap<String, ModelBuild>,
) -> Place {
    if let Some((alias, model)) = scope
        .iter()
        .rev()
        .find(|(alias, _)| path == format!("${alias}") || path.starts_with(&format!("${alias}.")))
    {
        if models.get(model).is_some_and(|build| build.strings) {
            return Place::Text;
        }
        let prefix = format!("${alias}.");
        if let Some(field) = path.strip_prefix(&prefix)
            && identifier(field).is_some()
            && !field.contains('.')
        {
            return Place::Field {
                model: model.clone(),
                field: field.to_owned(),
            };
        }
        return Place::Bad;
    }
    if let Some((model, index, field)) = model_index(path) {
        return if models.contains_key(&model) {
            Place::Index {
                model,
                index,
                field,
            }
        } else {
            Place::Bad
        };
    }
    let Some(segments) = path.strip_prefix("$.") else {
        return Place::Bad;
    };
    if segments.split('.').any(|part| identifier(part).is_none()) {
        return Place::Bad;
    }
    Place::Root
}

impl Data {
    pub fn infer(root: &Node, catalog: &Catalog, problems: &mut Vec<Unsupported>) -> Data {
        let mut gather = Gather {
            catalog,
            uses: Vec::new(),
            models: IndexMap::new(),
            problems,
        };
        gather.walk(root, "", &[]);
        let Gather {
            uses,
            mut models,
            problems,
            ..
        } = gather;
        let mut data = Data::default();
        let mut keys: IndexMap<String, String> = IndexMap::new();
        let mut roots: IndexMap<String, Readers> = IndexMap::new();
        let mut pending = Vec::new();
        for use_ in &uses {
            match classify(&use_.path, &use_.scope, &models) {
                Place::Root => {
                    note(
                        roots.entry(use_.path.clone()).or_default(),
                        use_.ty,
                        use_.writable,
                    );
                }
                Place::Field { model, field } => {
                    let alias = use_
                        .scope
                        .iter()
                        .rev()
                        .find(|(_, path)| path == &model)
                        .map(|(alias, _)| alias.clone());
                    if let Some(build) = models.get_mut(&model) {
                        note(
                            build.fields.entry(field.clone()).or_default(),
                            use_.ty,
                            use_.writable,
                        );
                    }
                    pending.push(Pending {
                        path: use_.path.clone(),
                        model,
                        field,
                        alias,
                        index: None,
                    });
                }
                Place::Index { model, index, field } => {
                    if let Some(build) = models.get_mut(&model) {
                        note(
                            build.fields.entry(field.clone()).or_default(),
                            use_.ty,
                            use_.writable,
                        );
                    }
                    pending.push(Pending {
                        path: use_.path.clone(),
                        model,
                        field,
                        alias: None,
                        index: Some(index),
                    });
                }
                Place::Text => {}
                Place::Bad => problems.push(Unsupported::new(
                    &use_.path,
                    "a segment is an array index or no Slint identifier; iterate with <each> instead",
                )),
            }
        }
        for (path, readers) in roots {
            if models.contains_key(&path) {
                problems.push(Unsupported::new(
                    &path,
                    "this path is a repeated array, not a single value",
                ));
                continue;
            }
            let Some(name) = property_name(&path, &mut keys, problems) else {
                continue;
            };
            let Some(ty) = ty_of(&readers, &path, problems) else {
                continue;
            };
            data.props.insert(path, Property { name, ty });
        }
        for (path, build) in models {
            let Some(name) = property_name(&path, &mut keys, problems) else {
                continue;
            };
            let fields = if build.strings {
                None
            } else {
                let mut fields = IndexMap::new();
                for (field, readers) in &build.fields {
                    if let Some(ty) = ty_of(readers, &format!("{path}.{field}"), problems) {
                        fields.insert(field.clone(), ty);
                    }
                }
                Some(fields)
            };
            data.models.insert(path.clone(), Model { name, fields });
        }
        for pending in pending {
            let Some(model) = data.models.get(&pending.model) else {
                continue;
            };
            let Some(ty) = model
                .fields
                .as_ref()
                .and_then(|fields| fields.get(&pending.field))
                .copied()
            else {
                continue;
            };
            let expr = match (&pending.alias, &pending.index) {
                (Some(alias), _) => format!("{alias}.{}", pending.field),
                (_, Some(index)) => format!("root.{}[{index}].{}", model.name, pending.field),
                _ => continue,
            };
            data.locals
                .entry(pending.path)
                .or_insert(Local { expr, ty });
        }
        data
    }

    /// The expression that reads `bind` (negated when `not`) as a value of type `want`; `None`
    /// when Slint has no such reading. A boolean read of text or a number is its truthiness
    /// (SPEC §2.1).
    pub fn read(&self, bind: &str, not: bool, want: Ty) -> Option<String> {
        if let Some(local) = self.locals.get(bind) {
            return adapt(&local.expr, local.ty, not, want);
        }
        let p = self.props.get(bind)?;
        adapt(&format!("root.{}", p.name), p.ty, not, want)
    }

    /// The `<=>` target for a writable prop of type `want`, when the binding can be written.
    pub fn two_way(&self, bind: &str, not: bool, want: Ty) -> Option<String> {
        if not {
            return None;
        }
        if let Some(local) = self.locals.get(bind) {
            return (local.ty == want).then(|| local.expr.clone());
        }
        let p = self.props.get(bind)?;
        (p.ty == want).then(|| format!("root.{}", p.name))
    }
}

fn adapt(expr: &str, ty: Ty, not: bool, want: Ty) -> Option<String> {
    let ne = if not { "==" } else { "!=" };
    Some(match (want, ty, not) {
        (Ty::Bool, Ty::Bool, false) => expr.to_owned(),
        (Ty::Bool, Ty::Bool, true) => format!("!{expr}"),
        (Ty::Bool, Ty::Str, _) => format!("{expr} {ne} \"\""),
        (Ty::Bool, Ty::Float | Ty::Int, _) => format!("{expr} {ne} 0"),
        (_, _, true) => return None,
        // Slint converts numbers to text implicitly, and int and float into each other.
        (Ty::Str, _, false) | (Ty::Float | Ty::Int, Ty::Float | Ty::Int, false) => expr.to_owned(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use weft_core::{Mode, ParseOptions, parse};

    fn data(markup: &str) -> (Data, Vec<Unsupported>) {
        let catalog = weft_catalog::core_catalog().unwrap();
        let options = ParseOptions {
            catalog: Some(&catalog),
            mode: Mode::Strict,
            tokens: None,
            actions: None,
        };
        let doc = parse(markup, &options).document.unwrap();
        let mut problems = vec![];
        (Data::infer(&doc.root, &catalog, &mut problems), problems)
    }

    #[test]
    fn a_path_takes_the_type_of_the_input_that_writes_it() {
        let (d, p) = data(
            r#"<screen id="s" weft="0.1"><field id="e" label="E" value="{$.user.email}"/><button id="b" disabled="{!$.user.email}">Go</button></screen>"#,
        );
        assert!(p.is_empty(), "{p:?}");
        let prop = &d.props["$.user.email"];
        assert_eq!((prop.name.as_str(), prop.ty), ("user-email", Ty::Str));
        assert_eq!(
            d.read("$.user.email", true, Ty::Bool).unwrap(),
            r#"root.user-email == """#
        );
        assert_eq!(
            d.two_way("$.user.email", false, Ty::Str).unwrap(),
            "root.user-email"
        );
    }

    #[test]
    fn a_path_read_only_as_a_boolean_is_a_bool() {
        let (d, _) = data(
            r#"<screen id="s" weft="0.1"><button id="b" disabled="{$.busy}">Go</button></screen>"#,
        );
        assert_eq!(d.props["$.busy"].ty, Ty::Bool);
        assert_eq!(d.read("$.busy", true, Ty::Bool).unwrap(), "!root.busy");
    }

    #[test]
    fn a_loop_field_reads_the_repeated_struct() {
        let (d, p) = data(
            r#"<screen id="s" weft="0.1"><list id="l"><each id="e" as="todo" in="{$.todos}"><item id="i"><checkbox id="c" checked="{$todo.done}" label="{$todo.title}"/></item></each></list><text id="t" text="{$.todos.0.title}"/></screen>"#,
        );
        assert!(p.is_empty(), "{p:?}");
        assert!(!d.props.contains_key("$.todos"));
        let model = &d.models["$.todos"];
        assert_eq!(model.name, "todos");
        assert_eq!(
            model.fields.as_ref().unwrap()["done"],
            Ty::Bool,
            "{:?}",
            model.fields
        );
        assert_eq!(d.read("$todo.done", false, Ty::Bool).unwrap(), "todo.done");
        assert_eq!(
            d.two_way("$todo.done", false, Ty::Bool).unwrap(),
            "todo.done"
        );
        assert_eq!(
            d.read("$.todos.0.title", false, Ty::Str).unwrap(),
            "root.todos[0].title"
        );
    }

    #[test]
    fn paths_that_clash_in_slint_are_refused() {
        let (d, p) = data(
            r#"<screen id="s" weft="0.1"><text id="a" text="{$.a_b}"/><text id="b" text="{$.a.b}"/><text id="c" text="{$.title}"/></screen>"#,
        );
        let paths: Vec<&str> = p.iter().map(|u| u.path.as_str()).collect();
        assert_eq!(paths, ["$.a.b"]);
        assert_eq!(d.props["$.title"].name, "title-data");
    }
}
